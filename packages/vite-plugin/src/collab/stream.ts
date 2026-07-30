/**
 * How the dev server tells a browser something changed.
 *
 * Server-sent events, over the connection the browser already opened. Two things
 * ruled out the alternatives.
 *
 * **Not Vite's own websocket.** That channel belongs to HMR, and a deck reloads
 * whole documents rather than swapping modules — putting editing traffic on it
 * would mean slidx frames arriving in a channel Vite is entitled to reshape
 * between minor versions.
 *
 * **Not a websocket of our own.** `node:http` has no websocket, so one would
 * mean a dependency in a plugin whose only third-party dependency is the CRDT it
 * cannot responsibly write itself. Everything here goes one way — the server
 * tells, the browser asks over ordinary POSTs — and one-way is exactly what SSE
 * is. It also reconnects on its own, which is what a co-presenter's phone
 * leaving a lift needs.
 *
 * # The heartbeat is not decoration
 *
 * A proxy or a phone radio will drop an idle connection without telling either
 * end, and the browser only notices when it next tries to read. A comment frame
 * every few seconds keeps the connection warm *and* is what the roster upstream
 * uses to decide somebody is still here.
 */

import type { ServerResponse } from "node:http";

/** How often a keep-alive comment goes out. Well inside a proxy's idle timeout. */
export const HEARTBEAT_MS = 5_000;

/** One connected browser. */
export interface Listener {
  id: string;
  send(event: string, payload: unknown): void;
  close(): void;
}

export interface Stream {
  /** Takes over a response and holds it open. */
  join(id: string, response: ServerResponse): Listener;
  /** Sends to everyone still connected. */
  broadcast(event: string, payload: unknown): void;
  readonly size: number;
  /** Ends every connection. For when the dev server stops. */
  closeAll(): void;
}

export function createStream(heartbeat = HEARTBEAT_MS): Stream {
  const listeners = new Map<string, { response: ServerResponse; timer: NodeJS.Timeout }>();

  function write(response: ServerResponse, frame: string): boolean {
    // A browser that has gone away makes this throw rather than return false,
    // and a dev server must not fall over because a phone went into a tunnel.
    try {
      response.write(frame);
      return true;
    } catch {
      return false;
    }
  }

  function drop(id: string): void {
    const held = listeners.get(id);
    if (!held) return;

    clearInterval(held.timer);
    listeners.delete(id);
    held.response.end();
  }

  return {
    get size() {
      return listeners.size;
    },

    join(id, response) {
      response.statusCode = 200;
      response.setHeader("content-type", "text/event-stream; charset=utf-8");
      response.setHeader("cache-control", "no-store");
      // Vite sits behind whatever the author has in front of it. A proxy that
      // buffers this never delivers a single event.
      response.setHeader("x-accel-buffering", "no");
      response.setHeader("connection", "keep-alive");
      response.flushHeaders?.();

      const timer = setInterval(() => {
        if (!write(response, ": keep-alive\n\n")) drop(id);
      }, heartbeat);
      // Never the reason a process stays alive: the dev server decides when it
      // is finished, not a timer nobody can see.
      timer.unref?.();

      listeners.set(id, { response, timer });
      response.on("close", () => drop(id));

      return {
        id,
        send: (event, payload) => void write(response, frame(event, payload)),
        close: () => drop(id),
      };
    },

    broadcast(event, payload) {
      const text = frame(event, payload);

      for (const [id, held] of [...listeners.entries()]) {
        if (!write(held.response, text)) drop(id);
      }
    },

    closeAll() {
      for (const id of [...listeners.keys()]) drop(id);
    },
  };
}

/**
 * One event, in the wire format.
 *
 * A blank line ends a frame, so a newline inside the data would truncate it —
 * and the payload here carries a deck source, which is nothing but newlines.
 * JSON escaping already makes that impossible; splitting anyway is what keeps it
 * impossible if the payload ever stops being JSON.
 */
export function frame(event: string, payload: unknown): string {
  const body = JSON.stringify(payload) ?? "null";
  const lines = body.split("\n").map((line) => `data: ${line}`);

  return `event: ${event}\n${lines.join("\n")}\n\n`;
}
