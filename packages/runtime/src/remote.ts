/**
 * Driving the deck from a phone.
 *
 * Mirroring already solved the hard half — two windows agreeing on a position,
 * no leader, ordering settled by a counter rather than a clock. A remote is a
 * third window that happens to be in someone's hand, so it is a
 * [`MirrorTransport`](./mirror) rather than a second implementation of the
 * same idea. Everything mirroring knows about echoes and out-of-order
 * delivery is inherited for free.
 *
 * What is new here is trust, and it is the whole reason this module exists.
 *
 * **The remote must not ride on the audience channel.** That channel is
 * anonymous on purpose and would rather forget who anyone is; bolting an
 * authenticated speaker path onto it would compromise the thing that makes it
 * safe to open to a room. This is a separate session with a separate secret.
 *
 * **The secret travels in the URL fragment.** A fragment is not sent with the
 * request, so it lands in no access log, no referrer header, and no proxy
 * record. A pairing link that leaked into a log would let a stranger advance
 * your slides from the back of the room, and a query parameter leaks into
 * logs by design.
 *
 * **A remote can move the deck and can do nothing else.** Not by policy but by
 * construction: the only thing it can express is a `MirrorMessage`, and
 * anything else on the wire has no listener here at all.
 *
 * **It is an enhancement.** A relay needs a network, and the room where the
 * network fails is the room this would have been useful in. When it goes, the
 * deck keeps presenting from the keyboard — same rule as mirroring.
 */

import type { MirrorMessage, MirrorTransport, Position } from "./mirror";

/** A session and the secret that proves the right to drive it. */
export interface Pairing {
  /** Names the room on the relay. Not secret; it appears in relayed frames. */
  session: string;
  /** Proves the right to join. Presented once, never attached to a frame. */
  secret: string;
}

export interface PairingOptions {
  /** Injected so a test can predict a pairing. Defaults to the platform CSPRNG. */
  random?: (bytes: Uint8Array) => Uint8Array;
}

/** The socket, injected so this is testable without a relay. */
export interface RemoteSocket {
  readonly open: boolean;
  send(data: string): void;
  listen(handler: (data: string) => void): () => void;
  close(): void;
}

export interface RemoteOptions {
  pairing: Pairing;
  socket: RemoteSocket;
}

/** Bytes of session identifier. Only has to be unique among live talks. */
const SESSION_BYTES = 8;

/**
 * Bytes of secret.
 *
 * Sixteen, because this is guarded by nothing but its own length: the relay
 * cannot rate-limit a guess it has no way to attribute, and the pairing URL is
 * on screen in a room full of people with cameras. A short numeric code would
 * be worth guessing.
 */
const SECRET_BYTES = 16;

/** The fragment key, so other fragment parameters can coexist with it. */
const FRAGMENT_KEY = "s";

export function createPairing(options: PairingOptions = {}): Pairing {
  const random = options.random ?? platformRandom;

  return {
    session: hex(random(new Uint8Array(SESSION_BYTES))),
    secret: hex(random(new Uint8Array(SECRET_BYTES))),
  };
}

/**
 * The link to point a phone at.
 *
 * The secret goes after the `#` and nowhere else. That single character is
 * what keeps it out of every log between here and the relay.
 */
export function pairingUrl(base: string, pairing: Pairing): string {
  return `${base}#${FRAGMENT_KEY}=${pairing.session}.${pairing.secret}`;
}

/**
 * The pairing a phone was opened with, or nothing.
 *
 * Read from the fragment only. A URL that carried the secret in its query is
 * refused rather than honoured — it is a URL that has already been written to
 * a log somewhere, and accepting it would make replaying that log enough to
 * take over the talk.
 */
export function readPairing(url: string): Pairing | null {
  const hash = url.indexOf("#");
  if (hash < 0) return null;

  const value = new URLSearchParams(url.slice(hash + 1)).get(FRAGMENT_KEY);
  if (value === null) return null;

  const [session, secret, ...rest] = value.split(".");
  if (rest.length > 0 || !isToken(session) || !isToken(secret)) return null;

  return { session, secret };
}

/**
 * A transport over a relay, for `createMirror`.
 *
 * The secret is presented once, in the join frame, and never again. A relay
 * that logs its traffic therefore logs positions and not the credential that
 * lets somebody send them.
 */
export function createRemoteTransport(options: RemoteOptions): MirrorTransport {
  const { pairing, socket } = options;

  send({ type: "join", session: pairing.session, secret: pairing.secret });

  function send(frame: unknown): void {
    // A closed socket is the normal end of a talk, and the failure mode of a
    // conference network. Neither is worth throwing into a slide over.
    if (!socket.open) return;
    socket.send(JSON.stringify(frame));
  }

  return {
    post(message) {
      send({ type: "relay", session: pairing.session, message });
    },

    listen(handler) {
      return socket.listen((data) => {
        const message = readRelay(data, pairing.session);
        if (message !== null) handler(message);
      });
    },

    close: () => socket.close(),
  };
}

/**
 * One inbound frame, if it is one this deck should act on.
 *
 * Returns `null` for everything else and never throws. Two talks will be using
 * the same relay at the same minute, and a frame is bytes from a network on a
 * machine that is on stage.
 */
function readRelay(data: string, session: string): MirrorMessage | null {
  const frame: unknown = parse(data);
  if (!isRecord(frame) || frame["type"] !== "relay" || frame["session"] !== session) return null;

  return asMirrorMessage(frame["message"]);
}

/**
 * The capability boundary, as a function.
 *
 * A remote can move the deck because a position is the only thing it can say.
 * If a relay ever started forwarding something richer, this is where it would
 * stop — there is no branch here that interprets anything else.
 */
function asMirrorMessage(value: unknown): MirrorMessage | null {
  if (!isRecord(value)) return null;

  const type = value["type"];
  const sequence = value["sequence"];

  if ((type !== "position" && type !== "request") || typeof sequence !== "number") return null;

  if (type === "request") return { type, sequence };

  const position = asPosition(value["position"]);

  return position === null ? null : { type, position, sequence };
}

function asPosition(value: unknown): Position | null {
  if (!isRecord(value)) return null;

  const slide = value["slide"];
  const step = value["step"];

  if (typeof slide !== "number" || typeof step !== "number") return null;

  return { slide, step };
}

function parse(data: string): unknown {
  try {
    return JSON.parse(data);
  } catch {
    return null;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Lowercase hex of a fixed length. Anything else did not come from here. */
function isToken(value: string | undefined): value is string {
  return value !== undefined && /^[0-9a-f]+$/.test(value);
}

function hex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * The platform's cryptographic randomness, and nothing else.
 *
 * There is deliberately no `Math.random()` fallback. A secret drawn from a
 * predictable source looks exactly like a real one and protects nothing, and a
 * browser old enough to lack this is not a browser a talk is being given from.
 */
function platformRandom(bytes: Uint8Array): Uint8Array {
  const source = globalThis.crypto;

  if (source?.getRandomValues === undefined) {
    throw new Error("slidx remote: no cryptographic randomness available to pair with");
  }

  return source.getRandomValues(bytes);
}
