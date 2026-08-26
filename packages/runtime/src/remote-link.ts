/**
 * Connecting a pairing to a live deck.
 *
 * `remote.ts` is the trust model: a secret in a fragment, a transport that
 * can only say a position. This file is the construction site a page actually
 * calls — mint or remember a pairing, open the relay, and keep the
 * same-machine channel so a dead Worker does not take the lectern with it.
 */

import { createMirror, localTransport, type Mirror, type MirrorTransport } from "./mirror";
import {
  createPairing,
  createRemoteTransport,
  type Pairing,
  type PairingOptions,
  type RemoteSocket,
} from "./remote";

const STORAGE_KEY = "slidx:remote-pairing";

export interface JoinRemoteOptions {
  pairing: Pairing;
  socket: RemoteSocket;
  /**
   * Same-machine windows. On by default so the projector still follows the
   * lectern when the relay is down. Off on a phone, which has no second window.
   */
  local?: boolean;
}

/**
 * The pairing this machine is already using, or a new one.
 *
 * One pairing per origin, in `localStorage`, so every presenter page of the
 * same deck joins the same session. A new pairing on every load would leave
 * the phone talking to a session the projector had already abandoned.
 *
 * Storage can be missing — `file://`, a locked-down profile — and then each
 * window mints its own. The QR on the presenter is still the one that works.
 */
export function rememberPairing(
  storage: Storage | null | undefined,
  options: PairingOptions = {},
): Pairing {
  if (storage) {
    try {
      const raw = storage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed: unknown = JSON.parse(raw);
        if (isPairing(parsed)) return parsed;
      }
    } catch {
      // A full or denied store is the normal end of a locked-down profile.
    }
  }

  const pairing = createPairing(options);

  if (storage) {
    try {
      storage.setItem(STORAGE_KEY, JSON.stringify(pairing));
    } catch {
      // Same case: the pairing still works for this window.
    }
  }

  return pairing;
}

/** The WebSocket URL a pairing opens against a Worker origin. */
export function relaySocketUrl(endpoint: string, session: string): string {
  const base = endpoint.trim().replace(/\/+$/, "");
  const ws = base.replace(/^http:/i, "ws:").replace(/^https:/i, "wss:");
  return `${ws}/sessions/${session}/socket`;
}

/**
 * A socket over a WebSocket, with sends queued until it opens.
 *
 * `createRemoteTransport` presents the join frame in the constructor. A
 * WebSocket that is still connecting would drop that frame, and the relay
 * would then see every later position as a stranger's.
 */
export function connectRelay(url: string): RemoteSocket {
  const socket = new WebSocket(url);
  const queue: string[] = [];
  const handlers = new Set<(data: string) => void>();

  socket.addEventListener("open", () => {
    for (const data of queue) socket.send(data);
    queue.length = 0;
  });
  socket.addEventListener("message", (event) => {
    if (typeof event.data === "string") {
      for (const handler of handlers) handler(event.data);
    }
  });

  return {
    get open() {
      return socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING;
    },
    send(data) {
      if (socket.readyState === WebSocket.OPEN) {
        socket.send(data);
        return;
      }
      if (socket.readyState === WebSocket.CONNECTING) queue.push(data);
    },
    listen(handler) {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
    close() {
      queue.length = 0;
      socket.close();
    },
  };
}

/** Posts to every transport and listens to each. Nulls are skipped. */
export function composeTransports(
  ...parts: Array<MirrorTransport | null | undefined>
): MirrorTransport {
  const transports = parts.filter((part): part is MirrorTransport => part != null);

  return {
    post(message) {
      for (const transport of transports) transport.post(message);
    },
    listen(handler) {
      const stops = transports.map((transport) => transport.listen(handler));
      return () => {
        for (const stop of stops) stop();
      };
    },
    close() {
      for (const transport of transports) transport.close();
    },
  };
}

/**
 * A mirror that talks to the phone and, unless asked not to, to this machine.
 *
 * The local channel is the enhancement the rest of mirroring already is: when
 * the relay's plug is pulled, the keyboard still drives the projector.
 */
export function joinRemote(options: JoinRemoteOptions): Mirror {
  const remote = createRemoteTransport({ pairing: options.pairing, socket: options.socket });
  const local = options.local === false ? null : localTransport();

  return createMirror({ transport: composeTransports(local, remote) });
}

function isPairing(value: unknown): value is Pairing {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  return isToken(record["session"]) && isToken(record["secret"]);
}

function isToken(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]+$/.test(value);
}
