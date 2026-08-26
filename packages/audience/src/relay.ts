/**
 * A pairing session: opaque frames, no storage, no audience traffic.
 *
 * The phone remote and the audience channel must not share a room. That
 * channel is anonymous on purpose; this one is a capability. The secret in
 * the pairing URL is the whole of that capability, and the only thing this
 * hub will remember is the secret the first joiner presented — in memory,
 * for as long as someone is still connected.
 *
 * What it forwards is bytes. Interpreting a position is the deck's job, and
 * a relay that started to understand one would be a second implementation of
 * the mirror.
 */

import type { Sink } from "./worker";

/** Eight random bytes, as lowercase hex. Anything else did not come from `createPairing`. */
export const SESSION_HEX_LENGTH = 16;

const SESSION_ID = /^[0-9a-f]+$/;

export function isSessionId(value: unknown): value is string {
  return typeof value === "string" && value.length === SESSION_HEX_LENGTH && SESSION_ID.test(value);
}

export interface JoinFrame {
  type: "join";
  session: string;
  secret: string;
}

export interface RelayFrame {
  type: "relay";
  session: string;
  message: unknown;
}

export type HubOutcome = { ok: true } | { ok: false; reason: "secret" | "session" | "malformed" };

export interface RelayHub {
  /** Present the secret once. The first joiner sets it; everyone else must match. */
  join(sink: Sink, session: string, secret: string): HubOutcome;
  leave(sink: Sink): void;
  /** Forwards a frame to everyone else in the session. The sender is not echoed. */
  relay(sink: Sink, session: string, message: unknown): HubOutcome;
  size(): number;
}

/**
 * One session's connected sockets.
 *
 * A Durable Object is already one session — its name is the pairing's
 * session id — so this hub does not key members by session. The id is
 * checked anyway: a join frame naming a different session is a frame that
 * has already been written down somewhere it should not have been.
 */
export function createRelayHub(session: string): RelayHub {
  const members = new Set<Sink>();
  let secret: string | null = null;

  const drop = (sink: Sink) => {
    members.delete(sink);
    if (members.size === 0) secret = null;
  };

  return {
    join(sink, offeredSession, offeredSecret) {
      if (offeredSession !== session || !isToken(offeredSecret)) {
        return { ok: false, reason: "session" };
      }

      if (secret === null) secret = offeredSecret;
      else if (secret !== offeredSecret) return { ok: false, reason: "secret" };

      members.add(sink);
      return { ok: true };
    },

    leave(sink) {
      drop(sink);
    },

    relay(from, offeredSession, message) {
      if (offeredSession !== session) return { ok: false, reason: "session" };
      if (!members.has(from)) return { ok: false, reason: "malformed" };

      const frame = JSON.stringify({ type: "relay", session, message });
      for (const sink of members) {
        if (sink === from) continue;
        try {
          sink.send(frame);
        } catch {
          drop(sink);
        }
      }

      return { ok: true };
    },

    size: () => members.size,
  };
}

/** Lowercase hex of a fixed length. Anything else did not come from the pairing. */
function isToken(value: string): boolean {
  return value.length > 0 && SESSION_ID.test(value);
}

/**
 * One inbound frame, or nothing.
 *
 * Returns `null` for everything else and never throws. Two talks will be
 * using the same Worker at the same minute, and a frame is bytes from a
 * network on a machine that is on stage.
 */
export function readRelayFrame(raw: unknown): JoinFrame | RelayFrame | null {
  if (typeof raw !== "string") return null;

  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return null;
  }

  if (!isRecord(value) || typeof value["session"] !== "string") return null;

  if (value["type"] === "join" && typeof value["secret"] === "string") {
    return { type: "join", session: value["session"], secret: value["secret"] };
  }

  if (value["type"] === "relay") {
    return { type: "relay", session: value["session"], message: value["message"] };
  }

  return null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
