/**
 * The room's HTTP surface.
 *
 * This is the specification for what a presenter view can do over HTTP, and —
 * more to the point — what anybody else cannot. It runs against a plain
 * `Request` and a room backed by a Map, which is the whole reason the routing
 * lives apart from the Durable Object that hosts it.
 *
 * The failure modes it guards:
 *
 * - The moderation queue reachable without the speaker's key.
 * - A wrong key answered differently from an unknown room, which would let
 *   somebody enumerate live rooms.
 * - A live room seized, or its questions read, by the second person to ask.
 * - A typo in frontmatter switching moderation off.
 * - A socket accepted for a room nobody opened, which would collect questions
 *   for a speaker who never agreed to run a Q&A.
 */

import { describe, expect, it } from "vitest";

import { routeRoomRequest, type RouteContext } from "../src/routes";
import { ask, HOST_KEY, open, roomFixture, type Fixture } from "./support";

const ORIGIN = "https://audience.example.workers.dev";

function call(
  fixture: Fixture,
  path: string,
  init: RequestInit = {},
  upgrade?: RouteContext["upgrade"],
): Promise<Response> {
  return routeRoomRequest(new Request(`${ORIGIN}${path}`, init), {
    room: fixture.room,
    ...(upgrade === undefined ? {} : { upgrade }),
  });
}

const bearer = (key: string) => ({ authorization: `Bearer ${key}` });

const openBody = (body: unknown): RequestInit => ({ method: "POST", body: JSON.stringify(body) });

describe("opening a room over HTTP", () => {
  it("returns the room it opened", async () => {
    const fixture = roomFixture();
    const response = await call(fixture, "/rooms/a-talk", openBody({ hostKey: HOST_KEY }));

    expect(response.status).toBe(200);
    expect(await response.json()).toMatchObject({ room: "a-talk", moderation: "held" });
  });

  it("refuses a body with no host key", async () => {
    const fixture = roomFixture();

    expect((await call(fixture, "/rooms/a-talk", openBody({}))).status).toBe(400);
  });

  it("refuses a body that is not JSON at all", async () => {
    const fixture = roomFixture();

    expect((await call(fixture, "/rooms/a-talk", { method: "POST", body: "{" })).status).toBe(400);
  });

  it("refuses a host key too short to be a secret", async () => {
    const fixture = roomFixture();

    expect((await call(fixture, "/rooms/a-talk", openBody({ hostKey: "abc" }))).status).toBe(400);
  });

  it("refuses a second speaker with 409 rather than handing the room over", async () => {
    const fixture = roomFixture();
    await call(fixture, "/rooms/a-talk", openBody({ hostKey: HOST_KEY }));

    const response = await call(
      fixture,
      "/rooms/a-talk",
      openBody({ hostKey: "another-key-here" }),
    );
    expect(response.status).toBe(409);
  });

  it("treats a misspelt moderation mode as the safe one", async () => {
    // Frontmatter is typed by hand. A typo must not be a way to end up
    // unmoderated by accident.
    const fixture = roomFixture();
    const response = await call(
      fixture,
      "/rooms/a-talk",
      openBody({ hostKey: HOST_KEY, moderation: "opne" }),
    );

    expect(await response.json()).toMatchObject({ moderation: "held" });
  });
});

describe("reading a room", () => {
  it("is a 404 before anybody opens it", async () => {
    const fixture = roomFixture();

    expect((await call(fixture, "/rooms/a-talk")).status).toBe(404);
  });

  it("returns the public view, without the queue", async () => {
    const fixture = roomFixture();
    await open(fixture);
    await ask(fixture, "held for the speaker");

    const body = (await (await call(fixture, "/rooms/a-talk")).json()) as Record<string, unknown>;
    expect(body["questions"]).toEqual([]);
    expect(body).not.toHaveProperty("pending");
  });

  it("is never cached, because a stale question queue is worse than none", async () => {
    const fixture = roomFixture();
    await open(fixture);

    expect((await call(fixture, "/rooms/a-talk")).headers.get("cache-control")).toBe("no-store");
  });

  it("answers a cross-origin preflight, because the deck is never on this origin", async () => {
    const fixture = roomFixture();
    const response = await call(fixture, "/rooms/a-talk", { method: "OPTIONS" });

    expect(response.status).toBe(204);
    expect(response.headers.get("access-control-allow-origin")).toBe("*");
  });

  it("refuses a method it has no meaning for", async () => {
    const fixture = roomFixture();

    expect((await call(fixture, "/rooms/a-talk", { method: "PUT" })).status).toBe(405);
  });
});

describe("the moderation queue", () => {
  it("needs the key", async () => {
    const fixture = roomFixture();
    await open(fixture);
    await ask(fixture, "held for the speaker");

    expect((await call(fixture, "/rooms/a-talk/pending")).status).toBe(403);
  });

  it("answers a wrong key exactly as it answers an unknown room", async () => {
    // Anything else tells a guesser which slugs are live.
    const fixture = roomFixture();
    await open(fixture);

    const wrong = await call(fixture, "/rooms/a-talk/pending", {
      headers: bearer("wrong-key-here"),
    });
    const unknown = await call(roomFixture(), "/rooms/a-talk/pending", {
      headers: bearer("wrong-key-here"),
    });

    expect(wrong.status).toBe(unknown.status);
    expect(await wrong.json()).toEqual(await unknown.json());
  });

  it("is handed over with the key", async () => {
    const fixture = roomFixture();
    await open(fixture);
    await ask(fixture, "held for the speaker");

    const response = await call(fixture, "/rooms/a-talk/pending", { headers: bearer(HOST_KEY) });
    const body = (await response.json()) as { pending: { text: string }[] };

    expect(body.pending.map((entry) => entry.text)).toEqual(["held for the speaker"]);
  });
});

describe("moderating", () => {
  it("publishes an approved question", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const id = await ask(fixture, "held for the speaker");

    const response = await call(fixture, `/rooms/a-talk/questions/${id}/approve`, {
      method: "POST",
      headers: bearer(HOST_KEY),
    });

    expect(response.status).toBe(200);
    expect((await fixture.room.snapshot())?.questions).toHaveLength(1);
  });

  it("refuses to approve without the key", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const id = await ask(fixture, "held for the speaker");

    const response = await call(fixture, `/rooms/a-talk/questions/${id}/approve`, {
      method: "POST",
    });

    expect(response.status).toBe(403);
    expect((await fixture.room.snapshot())?.questions).toEqual([]);
  });

  it("discards a dismissed question", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const id = await ask(fixture, "an unpleasant thing");

    await call(fixture, `/rooms/a-talk/questions/${id}/dismiss`, {
      method: "POST",
      headers: bearer(HOST_KEY),
    });

    expect((await fixture.room.hostSnapshot(HOST_KEY))?.pending).toEqual([]);
  });

  it("is a 404 for a question that is not there", async () => {
    const fixture = roomFixture();
    await open(fixture);

    const response = await call(fixture, "/rooms/a-talk/questions/999999/approve", {
      method: "POST",
      headers: bearer(HOST_KEY),
    });

    expect(response.status).toBe(404);
  });

  it("ends the room on the speaker's word", async () => {
    const fixture = roomFixture();
    await open(fixture);

    const response = await call(fixture, "/rooms/a-talk", {
      method: "DELETE",
      headers: bearer(HOST_KEY),
    });

    expect(response.status).toBe(200);
    expect(await fixture.room.snapshot()).toBeNull();
  });

  it("does not end it on anybody else's", async () => {
    const fixture = roomFixture();
    await open(fixture);

    const response = await call(fixture, "/rooms/a-talk", { method: "DELETE" });

    expect(response.status).toBe(403);
    expect(await fixture.room.snapshot()).not.toBeNull();
  });
});

describe("the socket route", () => {
  it("refuses a room nobody opened, without reaching the socket layer", async () => {
    const fixture = roomFixture();
    let upgraded = false;

    const response = await call(fixture, "/rooms/a-talk/socket", {}, () => {
      upgraded = true;
      return new Response(null, { status: 101 });
    });

    expect(response.status).toBe(404);
    expect(upgraded).toBe(false);
  });

  it("hands the socket layer a host key only after checking it", async () => {
    const fixture = roomFixture();
    await open(fixture);
    let seen: string | null | undefined;

    await call(fixture, `/rooms/a-talk/socket?key=${HOST_KEY}`, {}, (_request, hostKey) => {
      seen = hostKey;
      return new Response(null, { status: 101 });
    });

    expect(seen).toBe(HOST_KEY);
  });

  it("lets a stale key join as an ordinary participant", async () => {
    // Somebody watching on the presenter link should still be able to ask a
    // question when the key in it has gone out of date.
    const fixture = roomFixture();
    await open(fixture);
    let seen: string | null | undefined = "unset";

    await call(fixture, "/rooms/a-talk/socket?key=a-stale-key-here", {}, (_request, hostKey) => {
      seen = hostKey;
      return new Response(null, { status: 101 });
    });

    expect(seen).toBeNull();
  });

  it("says so plainly where sockets are not available", async () => {
    const fixture = roomFixture();
    await open(fixture);

    expect((await call(fixture, "/rooms/a-talk/socket")).status).toBe(501);
  });
});

describe("paths", () => {
  it("refuses a slug that is really a path", async () => {
    const fixture = roomFixture();

    expect((await call(fixture, "/rooms/..%2Fsecrets")).status).toBe(404);
  });

  it("refuses a slug that is a case variant, which would be a second room", async () => {
    const fixture = roomFixture();

    expect((await call(fixture, "/rooms/A-Talk")).status).toBe(404);
  });

  it("refuses a path it does not serve", async () => {
    const fixture = roomFixture();
    await open(fixture);

    expect((await call(fixture, "/rooms/a-talk/everything")).status).toBe(404);
  });
});
