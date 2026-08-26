/**
 * A pairing session forwards positions and nothing else.
 *
 * The failure modes it guards:
 *
 * - A secret in a query string being honoured (that is the runtime's job;
 *   this half refuses a session id that is not the pairing's hex).
 * - A second talk's frames reaching this one.
 * - A guess at the secret being answered with a useful distinction.
 * - A frame arriving before a join, which would let a stranger ride a
 *   session they never proved they belong to.
 * - The hub remembering a secret after the last socket has gone — that
 *   would be storage, and this module stores nothing.
 */

import { describe, expect, it } from "vite-plus/test";

import {
  createRelayHub,
  isSessionId,
  readRelayFrame,
  SESSION_HEX_LENGTH,
} from "../src/relay";
import { routeSessionRequest, splitSessionPath } from "../src/relay-routes";
import { handleFetch, type AudienceEnv, type DurableObjectNamespaceLike } from "../src/worker";
import type { Sink } from "../src/worker";

const SESSION = "abababababababab";
const SECRET = "cd".repeat(16);
const OTHER = "0101010101010101";

function sink() {
  const sent: string[] = [];
  const closed: Array<{ code?: number; reason?: string }> = [];
  const target: Sink = {
    send: (data) => sent.push(data),
    close: (code, reason) => closed.push({ code, reason }),
  };
  return { target, sent, closed };
}

describe("isSessionId", () => {
  it("accepts the sixteen hex characters createPairing writes", () => {
    expect(SESSION).toHaveLength(SESSION_HEX_LENGTH);
    expect(isSessionId(SESSION)).toBe(true);
  });

  it("refuses a room slug, a query-shaped secret, and mixed case", () => {
    // A room slug is a different namespace on purpose. Accepting one here
    // would let an audience path and a pairing share an object.
    expect(isSessionId("zero-js")).toBe(false);
    expect(isSessionId("Abababababababab")).toBe(false);
    expect(isSessionId("abababababababa")).toBe(false);
    expect(isSessionId(`${SESSION}ff`)).toBe(false);
  });
});

describe("splitSessionPath", () => {
  it("reads a pairing session and nothing that is not one", () => {
    expect(splitSessionPath(`/sessions/${SESSION}/socket`)).toEqual({
      id: SESSION,
      rest: "/socket",
    });
    expect(splitSessionPath(`/sessions/${SESSION}`)).toEqual({ id: SESSION, rest: "" });
    expect(splitSessionPath("/sessions/Talk/socket")).toBeNull();
    expect(splitSessionPath(`/rooms/${SESSION}/socket`)).toBeNull();
  });
});

describe("routeSessionRequest", () => {
  const origin = "https://audience.example.workers.dev";

  it("upgrades only the socket path", async () => {
    const upgraded = await routeSessionRequest(new Request(`${origin}/sessions/${SESSION}/socket`), {
      upgrade: () => new Response(null, { status: 101 }),
    });
    expect(upgraded.status).toBe(101);
  });

  it("answers 404 for a session that is not a pairing id", async () => {
    const response = await routeSessionRequest(
      new Request(`${origin}/sessions/zero-js/socket`),
      { upgrade: () => new Response(null, { status: 101 }) },
    );
    expect(response.status).toBe(404);
  });

  it("answers 501 when sockets cannot be opened, rather than pretending", async () => {
    const response = await routeSessionRequest(
      new Request(`${origin}/sessions/${SESSION}/socket`),
      {},
    );
    expect(response.status).toBe(501);
  });

  it("does not treat the session itself as a resource that can be read", async () => {
    const response = await routeSessionRequest(
      new Request(`${origin}/sessions/${SESSION}`),
      { upgrade: () => new Response(null, { status: 101 }) },
    );
    expect(response.status).toBe(404);
  });
});

describe("the hub", () => {
  it("forwards a relay frame to the other member and not back to the sender", () => {
    const hub = createRelayHub(SESSION);
    const speaker = sink();
    const phone = sink();

    expect(hub.join(speaker.target, SESSION, SECRET)).toEqual({ ok: true });
    expect(hub.join(phone.target, SESSION, SECRET)).toEqual({ ok: true });

    const message = { type: "position", slide: 2, step: 0, sequence: 1 };
    expect(hub.relay(phone.target, SESSION, message)).toEqual({ ok: true });

    expect(phone.sent).toEqual([]);
    expect(speaker.sent).toEqual([JSON.stringify({ type: "relay", session: SESSION, message })]);
  });

  it("refuses a join that names a different session", () => {
    const hub = createRelayHub(SESSION);
    const phone = sink();

    expect(hub.join(phone.target, OTHER, SECRET)).toEqual({ ok: false, reason: "session" });
    expect(hub.size()).toBe(0);
  });

  it("refuses a second secret rather than handing the session over", () => {
    const hub = createRelayHub(SESSION);
    const speaker = sink();
    const stranger = sink();

    expect(hub.join(speaker.target, SESSION, SECRET)).toEqual({ ok: true });
    expect(hub.join(stranger.target, SESSION, "ff".repeat(16))).toEqual({
      ok: false,
      reason: "secret",
    });
    expect(hub.size()).toBe(1);
  });

  it("drops a frame that arrives before a join", () => {
    const hub = createRelayHub(SESSION);
    const stranger = sink();
    const speaker = sink();

    hub.join(speaker.target, SESSION, SECRET);
    expect(hub.relay(stranger.target, SESSION, { type: "position" })).toEqual({
      ok: false,
      reason: "malformed",
    });
    expect(speaker.sent).toEqual([]);
  });

  it("forgets the secret when the last socket leaves", () => {
    const hub = createRelayHub(SESSION);
    const first = sink();
    const later = sink();

    hub.join(first.target, SESSION, SECRET);
    hub.leave(first.target);
    expect(hub.size()).toBe(0);

    // A new talk can reuse the Durable Object name. The previous secret
    // must not still be the one that gets in.
    expect(hub.join(later.target, SESSION, "ee".repeat(16))).toEqual({ ok: true });
  });
});

describe("handleFetch", () => {
  it("sends a pairing socket to the session object, never the room", async () => {
    const rooms = fakeNamespace();
    const sessions = fakeNamespace();
    const env: AudienceEnv = { AUDIENCE_ROOM: rooms.namespace, REMOTE_SESSION: sessions.namespace };

    await handleFetch(new Request(`https://w.dev/sessions/${SESSION}/socket`), env);

    expect(sessions.names).toEqual([SESSION]);
    expect(rooms.names).toEqual([]);
  });

  it("still sends a room slug to the audience object", async () => {
    const rooms = fakeNamespace();
    const sessions = fakeNamespace();
    const env: AudienceEnv = { AUDIENCE_ROOM: rooms.namespace, REMOTE_SESSION: sessions.namespace };

    await handleFetch(new Request("https://w.dev/rooms/zero-js/socket"), env);

    expect(rooms.names).toEqual(["zero-js"]);
    expect(sessions.names).toEqual([]);
  });
});

function fakeNamespace(): { names: string[]; namespace: DurableObjectNamespaceLike } {
  const names: string[] = [];
  return {
    names,
    namespace: {
      idFromName: (name: string) => name,
      get: (id: unknown) => ({
        fetch: async () => {
          names.push(String(id));
          return new Response("ok");
        },
      }),
    },
  };
}

describe("readRelayFrame", () => {
  it("reads a join and a relay and nothing else", () => {
    expect(readRelayFrame(JSON.stringify({ type: "join", session: SESSION, secret: SECRET }))).toEqual(
      { type: "join", session: SESSION, secret: SECRET },
    );
    expect(readRelayFrame(JSON.stringify({ type: "relay", session: SESSION, message: { a: 1 } }))).toEqual(
      { type: "relay", session: SESSION, message: { a: 1 } },
    );
    expect(readRelayFrame("not json")).toBeNull();
    expect(readRelayFrame(JSON.stringify({ type: "state", session: SESSION }))).toBeNull();
    expect(readRelayFrame(new Uint8Array([1, 2]))).toBeNull();
  });
});
