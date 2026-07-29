/**
 * The room's rules.
 *
 * This is the specification for the half that cannot be bypassed. Everything a
 * client checks, a room checks again — so these tests deliberately call the
 * room directly, skipping the frame validator the way somebody with a socket
 * and five minutes would.
 *
 * The failure modes it guards are the ones that would make this feature worse
 * than not shipping it:
 *
 * - A question reaching a shared screen without the speaker seeing it first.
 * - The pending queue being readable, or its ids being discoverable, by anyone
 *   who is not the speaker.
 * - A live room being taken over, or its moderation switched off, by whoever
 *   asks second.
 * - One connection flooding the room, or one person voting twenty times.
 * - A caller-supplied lifetime turning a Worker into permanent hosting, or a
 *   room outliving the talk because an alarm did not fire.
 *
 * Storage is a Map and the clock is a counter, so all of it is arithmetic.
 */

import { describe, expect, it } from "vitest";

import { LIMITS } from "../src/protocol";
import { createRoom, ROOM_LIFETIME } from "../src/room";
import { ask, clock, HOST_KEY, memoryStorage, open, roomFixture } from "./support";

describe("opening a room", () => {
  it("holds questions unless told otherwise", async () => {
    // The safe mode is what omission gets you. This is the single most
    // important default in the package.
    const fixture = roomFixture();
    await open(fixture);

    expect((await fixture.room.snapshot())?.moderation).toBe("held");
  });

  it("reaches the unmoderated mode only when it is written down", async () => {
    const fixture = roomFixture();
    await open(fixture, { moderation: "open" });

    expect((await fixture.room.snapshot())?.moderation).toBe("open");
  });

  it("refuses a host key too short to be a secret", async () => {
    const fixture = roomFixture();

    expect(await fixture.room.open({ hostKey: "hunter2" })).toEqual({
      ok: false,
      reason: "weak-host-key",
    });
  });

  it("has no snapshot before anyone opens it", async () => {
    // An unopened slug must not quietly collect questions for a speaker who
    // never agreed to run a Q&A.
    const fixture = roomFixture();

    expect(await fixture.room.snapshot()).toBeNull();
  });

  it("lets the speaker reopen it, so a reloaded presenter view finds its room", async () => {
    const fixture = roomFixture();
    await open(fixture);

    expect((await fixture.room.open({ hostKey: HOST_KEY })).ok).toBe(true);
  });

  it("does not extend its life when it is reopened", async () => {
    // A room renewable by reconnecting is a room that never ends.
    const fixture = roomFixture();
    await open(fixture);
    const first = (await fixture.room.snapshot())?.expiresAt;

    fixture.time.advance(60 * 60_000);
    await fixture.room.open({ hostKey: HOST_KEY });

    expect((await fixture.room.snapshot())?.expiresAt).toBe(first);
  });

  it("refuses a second speaker while it is live", async () => {
    const fixture = roomFixture();
    await open(fixture);

    expect(await fixture.room.open({ hostKey: "someone-elses-key-here" })).toEqual({
      ok: false,
      reason: "taken",
    });
  });

  it("clamps a caller-supplied lifetime to the maximum", async () => {
    // The lifetime is input. An unbounded one is free permanent hosting.
    const fixture = roomFixture();
    const openedAt = fixture.time.now();
    await open(fixture, { lifetimeMs: 400 * 24 * 60 * 60_000 });

    expect((await fixture.room.snapshot())?.expiresAt).toBe(openedAt + ROOM_LIFETIME.maximumMs);
  });

  it("clamps a lifetime nobody could use to the minimum", async () => {
    const fixture = roomFixture();
    const openedAt = fixture.time.now();
    await open(fixture, { lifetimeMs: 1 });

    expect((await fixture.room.snapshot())?.expiresAt).toBe(openedAt + ROOM_LIFETIME.minimumMs);
  });

  it("reports how many connections are attached, and nothing else about them", async () => {
    const fixture = roomFixture();
    await open(fixture);
    fixture.setPresent(37);

    const snapshot = await fixture.room.snapshot();
    expect(snapshot?.present).toBe(37);
    expect(Object.keys(snapshot ?? {})).not.toContain("participants");
  });
});

describe("moderation", () => {
  it("keeps a new question out of the shared view", async () => {
    const fixture = roomFixture();
    await open(fixture);

    const outcome = await fixture.room.submit(
      { type: "question", text: "why not Zig?" },
      fixture.connection(),
    );

    expect(outcome).toEqual({ ok: true, effect: "held" });
    expect((await fixture.room.snapshot())?.questions).toEqual([]);
  });

  it("shows it to the speaker, who is the only one who can see the queue", async () => {
    const fixture = roomFixture();
    await open(fixture);
    await ask(fixture, "why not Zig?");

    const host = await fixture.room.hostSnapshot(HOST_KEY);
    expect(host?.pending?.[0]?.text).toBe("why not Zig?");
  });

  it("does not hand the queue to a wrong key", async () => {
    const fixture = roomFixture();
    await open(fixture);
    await ask(fixture, "why not Zig?");

    expect(await fixture.room.hostSnapshot("a-different-key-here")).toBeNull();
  });

  it("publishes on approval", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const id = await ask(fixture, "why not Zig?");

    expect(await fixture.room.approve(id, HOST_KEY)).toEqual({ ok: true });
    expect((await fixture.room.snapshot())?.questions[0]?.text).toBe("why not Zig?");
  });

  it("cannot be approved by someone without the key", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const id = await ask(fixture, "why not Zig?");

    expect(await fixture.room.approve(id, "not-the-host-key-x")).toEqual({
      ok: false,
      reason: "forbidden",
    });
    expect((await fixture.room.snapshot())?.questions).toEqual([]);
  });

  it("deletes a dismissed question rather than filing it", async () => {
    // A hidden column of everything the speaker rejected is an archive of what
    // people said anonymously, which is the thing this channel promised not to
    // build.
    const fixture = roomFixture();
    await open(fixture);
    const id = await ask(fixture, "an unpleasant thing");

    expect(await fixture.room.dismiss(id, HOST_KEY)).toEqual({ ok: true });
    expect((await fixture.room.hostSnapshot(HOST_KEY))?.pending).toEqual([]);
    expect(await fixture.room.approve(id, HOST_KEY)).toEqual({
      ok: false,
      reason: "unknown-question",
    });
    expect(fixture.storage.keys()).not.toContain(`q:${id}`);
  });

  it("cannot be dismissed by someone without the key", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const id = await ask(fixture, "why not Zig?");

    expect(await fixture.room.dismiss(id, "not-the-host-key-x")).toEqual({
      ok: false,
      reason: "forbidden",
    });
    expect((await fixture.room.hostSnapshot(HOST_KEY))?.pending).toHaveLength(1);
  });

  it("publishes immediately in the unmoderated mode, which is the whole difference", async () => {
    const fixture = roomFixture();
    await open(fixture, { moderation: "open" });

    const outcome = await fixture.room.submit(
      { type: "question", text: "how fast is the parser?" },
      fixture.connection(),
    );

    expect(outcome).toEqual({ ok: true, effect: "published" });
    expect((await fixture.room.snapshot())?.questions).toHaveLength(1);
  });
});

describe("upvotes", () => {
  it("counts one, and reorders the queue", async () => {
    const fixture = roomFixture();
    await open(fixture, { moderation: "open" });
    const first = await ask(fixture, "asked first");
    const second = await ask(fixture, "asked second");

    await fixture.room.submit({ type: "upvote", questionId: second }, fixture.connection());

    const questions = (await fixture.room.snapshot())?.questions ?? [];
    expect(questions.map((entry) => entry.id)).toEqual([second, first]);
  });

  it("refuses a second vote from the same connection", async () => {
    const fixture = roomFixture();
    await open(fixture, { moderation: "open" });
    const id = await ask(fixture, "asked");
    const participant = fixture.connection();

    await fixture.room.submit({ type: "upvote", questionId: id }, participant);

    expect(await fixture.room.submit({ type: "upvote", questionId: id }, participant)).toEqual({
      ok: false,
      reason: "duplicate-vote",
    });
    expect((await fixture.room.snapshot())?.questions[0]?.votes).toBe(1);
  });

  it("counts a different connection separately", async () => {
    const fixture = roomFixture();
    await open(fixture, { moderation: "open" });
    const id = await ask(fixture, "asked");

    await fixture.room.submit({ type: "upvote", questionId: id }, fixture.connection());
    await fixture.room.submit({ type: "upvote", questionId: id }, fixture.connection());

    expect((await fixture.room.snapshot())?.questions[0]?.votes).toBe(2);
  });

  it("answers a held question exactly as it answers one that never existed", async () => {
    // Any other answer turns the upvote path into a way to enumerate what the
    // speaker has not approved.
    const fixture = roomFixture();
    await open(fixture);
    const held = await ask(fixture, "still waiting");

    expect(
      await fixture.room.submit({ type: "upvote", questionId: held }, fixture.connection()),
    ).toEqual({ ok: false, reason: "unknown-question" });
    expect(
      await fixture.room.submit({ type: "upvote", questionId: "999999" }, fixture.connection()),
    ).toEqual({ ok: false, reason: "unknown-question" });
  });
});

describe("caps the room enforces itself", () => {
  it("refuses an over-long question that never went through the frame validator", async () => {
    // A client-side cap is a suggestion. This is the check that counts.
    const fixture = roomFixture();
    await open(fixture);

    expect(
      await fixture.room.submit(
        { type: "question", text: "a".repeat(LIMITS.questionText + 1) },
        fixture.connection(),
      ),
    ).toEqual({ ok: false, reason: "too-long" });
  });

  it("refuses an over-long display name the same way", async () => {
    const fixture = roomFixture();
    await open(fixture);

    expect(
      await fixture.room.submit(
        { type: "question", text: "fine", name: "n".repeat(LIMITS.displayName + 1) },
        fixture.connection(),
      ),
    ).toEqual({ ok: false, reason: "too-long" });
  });

  it("refuses a question made only of characters that normalise away", async () => {
    const fixture = roomFixture();
    await open(fixture);

    expect(
      await fixture.room.submit(
        { type: "question", text: "\u0009\u200B\u202A" },
        fixture.connection(),
      ),
    ).toEqual({ ok: false, reason: "empty" });
  });

  it("normalises before storing, so the queue holds one line", async () => {
    const fixture = roomFixture();
    await open(fixture);
    await ask(fixture, "line one\n\n\nline two");

    expect((await fixture.room.hostSnapshot(HOST_KEY))?.pending?.[0]?.text).toBe(
      "line one line two",
    );
  });

  it("stops accepting questions once the room is full", async () => {
    const fixture = roomFixture();
    await open(fixture);

    for (let index = 0; index < LIMITS.questionsPerRoom; index += 1) {
      await fixture.room.submit({ type: "question", text: `q${index}` }, fixture.connection());
    }

    expect(
      await fixture.room.submit({ type: "question", text: "one more" }, fixture.connection()),
    ).toEqual({ ok: false, reason: "room-full" });
  });
});

describe("rate limiting", () => {
  it("stops one connection asking without pause", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const participant = fixture.connection();

    const outcomes = [];
    for (let index = 0; index < 5; index += 1) {
      outcomes.push(
        await fixture.room.submit({ type: "question", text: `q${index}` }, participant),
      );
    }

    expect(outcomes.filter((outcome) => outcome.ok)).toHaveLength(3);
    expect(outcomes.at(-1)).toEqual({ ok: false, reason: "rate-limited" });
  });

  it("lets the same connection ask again after waiting", async () => {
    const fixture = roomFixture();
    await open(fixture);
    const participant = fixture.connection();

    for (let index = 0; index < 3; index += 1) {
      await fixture.room.submit({ type: "question", text: `q${index}` }, participant);
    }
    fixture.time.advance(30_000);

    expect((await fixture.room.submit({ type: "question", text: "later" }, participant)).ok).toBe(
      true,
    );
  });

  it("charges for invalid submissions too, so garbage is not free", async () => {
    // Spending the allowance only on well-formed input makes flooding cheaper
    // than participating.
    const fixture = roomFixture();
    await open(fixture);
    const participant = fixture.connection();

    for (let index = 0; index < 3; index += 1) {
      await fixture.room.submit({ type: "question", text: "" }, participant);
    }

    expect(
      await fixture.room.submit({ type: "question", text: "a real one" }, participant),
    ).toEqual({ ok: false, reason: "rate-limited" });
  });

  it("keeps the allowances for different actions apart", async () => {
    // Somebody who used up their questions can still applaud.
    const fixture = roomFixture();
    await open(fixture);
    const participant = fixture.connection();

    for (let index = 0; index < 4; index += 1) {
      await fixture.room.submit({ type: "question", text: `q${index}` }, participant);
    }

    expect(await fixture.room.submit({ type: "reaction", kind: "clap" }, participant)).toEqual({
      ok: true,
      effect: "counted",
    });
  });
});

describe("reactions", () => {
  it("are tallied", async () => {
    const fixture = roomFixture();
    await open(fixture);

    await fixture.room.submit({ type: "reaction", kind: "clap" }, fixture.connection());
    await fixture.room.submit({ type: "reaction", kind: "clap" }, fixture.connection());
    await fixture.room.submit({ type: "reaction", kind: "confused" }, fixture.connection());

    expect((await fixture.room.snapshot())?.reactions).toEqual({
      clap: 2,
      agree: 0,
      confused: 1,
      love: 0,
    });
  });

  it("are never written down, because applause is a moment and not a record", async () => {
    const storage = memoryStorage();
    const fixture = roomFixture({ storage });
    await open(fixture);
    await ask(fixture, "a question that does persist");
    await fixture.room.submit({ type: "reaction", kind: "clap" }, fixture.connection());

    // A second room over the same storage is what a recycled Durable Object
    // sees when it comes back.
    const restarted = createRoom({ slug: "a-talk", storage, now: fixture.time.now });

    expect((await restarted.hostSnapshot(HOST_KEY))?.pending).toHaveLength(1);
    expect((await restarted.snapshot())?.reactions.clap).toBe(0);
  });
});

describe("the room ending", () => {
  it("is gone once its time is up", async () => {
    const fixture = roomFixture();
    await open(fixture);
    await ask(fixture, "asked during the talk");

    fixture.time.advance(ROOM_LIFETIME.defaultMs);

    expect(await fixture.room.snapshot()).toBeNull();
  });

  it("takes its questions with it", async () => {
    // Expiry is a promise made to the people who asked the questions, so it
    // has to delete rather than hide.
    const fixture = roomFixture();
    await open(fixture);
    await ask(fixture, "asked during the talk");

    fixture.time.advance(ROOM_LIFETIME.defaultMs);
    await fixture.room.sweep();

    expect(fixture.storage.keys()).toEqual([]);
  });

  it("expires when asked rather than waiting for an alarm to fire", async () => {
    // A redeploy or a cold object can lose an alarm. Checking on the way in
    // means a missed alarm cannot leave a room answering after its day.
    const fixture = roomFixture();
    await open(fixture);
    fixture.time.advance(ROOM_LIFETIME.defaultMs);

    expect(
      await fixture.room.submit({ type: "question", text: "too late" }, fixture.connection()),
    ).toEqual({ ok: false, reason: "room-closed" });
  });

  it("reports why it ended, once", async () => {
    const fixture = roomFixture();
    await open(fixture);
    fixture.time.advance(ROOM_LIFETIME.defaultMs);

    expect(await fixture.room.sweep()).toBe("expired");
    expect(await fixture.room.sweep()).toBeNull();
  });

  it("can be ended early by the speaker", async () => {
    const fixture = roomFixture();
    await open(fixture);
    await ask(fixture, "asked during the talk");

    expect(await fixture.room.end(HOST_KEY)).toEqual({ ok: true });
    expect(await fixture.room.snapshot()).toBeNull();
    expect(fixture.storage.keys()).toEqual([]);
  });

  it("cannot be ended by anybody else", async () => {
    const fixture = roomFixture();
    await open(fixture);

    expect(await fixture.room.end("not-the-host-key-x")).toEqual({
      ok: false,
      reason: "forbidden",
    });
    expect(await fixture.room.snapshot()).not.toBeNull();
  });

  it("frees the slug, so the next talk in that slot gets a fresh room", async () => {
    const fixture = roomFixture();
    await open(fixture);
    fixture.time.advance(ROOM_LIFETIME.defaultMs);

    expect((await fixture.room.open({ hostKey: "a-totally-new-key-1" })).ok).toBe(true);
  });

  it("tells the platform when to wake it up", async () => {
    const time = clock();
    const room = createRoom({ slug: "a-talk", storage: memoryStorage(), now: time.now });
    expect(await room.endsAt()).toBeNull();

    await room.open({ hostKey: HOST_KEY });
    expect(await room.endsAt()).toBe(time.now() + ROOM_LIFETIME.defaultMs);
  });
});
