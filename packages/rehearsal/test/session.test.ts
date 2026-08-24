/**
 * The presenter is one HTML document per slide. These tests specify the seam
 * between those documents: live dwell is checkpointed before one disappears,
 * restored without billing the page load, and resumed only when the speaker
 * had not explicitly paused.
 */

import { describe, expect, it } from "vite-plus/test";

import { openRehearsalSession, type RehearsalStorage } from "../src/session";

function clock(): { now: () => number; advance: (ms: number) => void } {
  let current = 0;
  return {
    now: () => current,
    advance: (ms) => {
      current += ms;
    },
  };
}

/**
 * A real key-value store, because the session uses two keys.
 *
 * It used to hold one value and ignore the key, which was true of a session
 * that wrote one thing. Once finished runs are filed beside the live one, a
 * store that ignored the key would let the history overwrite the recording —
 * and the test that caught it was the one asserting a finished rehearsal does
 * not reopen on reload, which is precisely the failure a stub like that hides.
 *
 * `value` stays, and stays meaning the live recording: that is what every
 * assertion using it is about.
 */
function storage(initial?: string): RehearsalStorage & { value: string | null } {
  const items = new Map<string, string>();
  if (initial !== undefined) items.set("deck", initial);

  return {
    get value(): string | null {
      return items.get("deck") ?? null;
    },
    getItem(key) {
      return items.get(key) ?? null;
    },
    setItem(key, value) {
      items.set(key, value);
    },
    removeItem(key) {
      items.delete(key);
    },
  };
}

const slides = [
  { id: "intro", budgetMs: 30_000 },
  { id: "details", budgetMs: 60_000 },
];

describe("crossing presenter pages", () => {
  it("checkpoints live dwell and resumes on the next slide without billing the load", () => {
    const time = clock();
    const saved = storage();
    const first = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
      now: time.now,
    });

    first.start();
    time.advance(20_000);
    first.checkpoint();

    // Five seconds spent loading the next document is not speaking time.
    time.advance(5_000);
    const second = openRehearsalSession({
      key: "deck",
      slideId: "details",
      slides,
      storage: saved,
      now: time.now,
    });

    expect(second.state()).toMatchObject({
      status: "recording",
      slideId: "details",
      elapsedMs: 20_000,
    });

    time.advance(10_000);
    const recording = second.recording();
    expect(recording.slides).toMatchObject([
      { id: "intro", actualMs: 20_000, visits: 1 },
      { id: "details", actualMs: 10_000, visits: 1 },
    ]);
  });

  it("keeps an explicit pause paused across navigation", () => {
    const time = clock();
    const saved = storage();
    const first = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
      now: time.now,
    });

    first.start();
    time.advance(12_000);
    first.pause();

    const second = openRehearsalSession({
      key: "deck",
      slideId: "details",
      slides,
      storage: saved,
      now: time.now,
    });
    time.advance(60_000);

    expect(second.state()).toMatchObject({
      status: "paused",
      slideId: "intro",
      elapsedMs: 12_000,
    });
    expect(second.recording().slides[1]).toMatchObject({ visits: 0, actualMs: 0 });

    second.start();
    expect(second.state()).toMatchObject({ status: "recording", slideId: "details" });
  });

  it("resumes on the same slide without inventing a second visit", () => {
    const time = clock();
    const session = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      now: time.now,
    });

    session.start();
    time.advance(5_000);
    session.pause();
    session.start();

    expect(session.recording().slides[0]?.visits).toBe(1);
  });

  it("does not reopen a finished rehearsal on reload", () => {
    const time = clock();
    const saved = storage();
    const first = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
      now: time.now,
    });

    first.start();
    time.advance(30_000);
    first.finish();

    const reloaded = openRehearsalSession({
      key: "deck",
      slideId: "details",
      slides,
      storage: saved,
      now: time.now,
    });
    reloaded.start();

    expect(reloaded.state().status).toBe("finished");
    expect(reloaded.recording().slides[1]?.visits).toBe(0);
  });
});

describe("storage that cannot be trusted", () => {
  it("discards malformed JSON and starts a clean rehearsal", () => {
    const saved = storage("{half written");
    const session = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
    });

    expect(session.state().status).toBe("idle");
    expect(saved.value).toBeNull();
    expect(session.persistence()).toBe("available");
  });

  it("discards a valid JSON value with an unsupported recording shape", () => {
    const saved = storage(JSON.stringify({ version: 99, slides: [] }));
    const session = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
    });

    expect(session.state().status).toBe("idle");
    expect(saved.value).toBeNull();
  });

  it("keeps recording in memory when browser storage throws", () => {
    const time = clock();
    const denied: RehearsalStorage = {
      getItem() {
        throw new DOMException("denied", "SecurityError");
      },
      setItem() {
        throw new DOMException("denied", "SecurityError");
      },
      removeItem() {
        throw new DOMException("denied", "SecurityError");
      },
    };
    const session = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: denied,
      now: time.now,
    });

    session.start();
    time.advance(18_000);

    expect(session.checkpoint()).toBe("unavailable");
    expect(session.state().elapsedMs).toBe(18_000);
  });
});

describe("controlling a run", () => {
  it("returns the per-slide report when finishing", () => {
    const time = clock();
    const session = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      now: time.now,
    });

    session.start();
    time.advance(50_000);
    const report = session.finish();

    expect(report.complete).toBe(true);
    expect(report.slides[0]).toMatchObject({
      id: "intro",
      actualMs: 50_000,
      budgetMs: 30_000,
      deltaMs: 20_000,
      verdict: "over",
    });
  });

  it("reset removes the stored run and returns to idle", () => {
    const saved = storage();
    const session = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
    });

    session.start();
    expect(saved.value).not.toBeNull();

    session.reset();
    expect(session.state().status).toBe("idle");
    expect(saved.value).toBeNull();
  });
});

describe("the history a trend is read from", () => {
  it("keeps a finished run so the next one has something to compare against", () => {
    const time = clock();
    const saved = storage();
    const session = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
      now: time.now,
    });

    session.start();
    time.advance(30_000);
    session.finish();

    expect(session.history()).toHaveLength(1);
    expect(session.history()[0]?.status).toBe("finished");
  });

  it("keeps an abandoned run too", () => {
    // A talk the speaker stopped halfway is still where the time went for the
    // slides they reached. Dropping it would make the next comparison silently
    // span two rehearsals rather than one.
    const time = clock();
    const saved = storage();
    const session = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
      now: time.now,
    });

    session.start();
    time.advance(10_000);
    session.abandon();

    expect(session.history().map((run) => run.status)).toEqual(["abandoned"]);
  });

  it("does not file a run that is still going", () => {
    const time = clock();
    const saved = storage();
    const session = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
      now: time.now,
    });

    session.start();
    time.advance(30_000);

    expect(session.history()).toEqual([]);
  });

  it("does not file the same run twice when the page reloads after it ended", () => {
    // `finish` is a user action and a reload is not, which is what makes the
    // filing once per run rather than once per page.
    const time = clock();
    const saved = storage();
    const first = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
      now: time.now,
    });

    first.start();
    time.advance(30_000);
    first.finish();

    const reloaded = openRehearsalSession({
      key: "deck",
      slideId: "details",
      slides,
      storage: saved,
      now: time.now,
    });

    expect(reloaded.history()).toHaveLength(1);
  });

  it("keeps the history when the speaker starts again", () => {
    const time = clock();
    const saved = storage();
    const session = openRehearsalSession({
      key: "deck",
      slideId: "intro",
      slides,
      storage: saved,
      now: time.now,
    });

    session.start();
    time.advance(30_000);
    session.finish();
    session.reset();

    expect(session.history()).toHaveLength(1);
  });

  it("reports no history rather than throwing on a value it cannot parse", () => {
    const saved = storage();
    saved.setItem("deck:history", "{ not an array");

    const session = openRehearsalSession({ key: "deck", slideId: "intro", slides, storage: saved });

    expect(session.history()).toEqual([]);
  });

  it("has no history at all without storage", () => {
    const session = openRehearsalSession({ key: "deck", slideId: "intro", slides });

    expect(session.history()).toEqual([]);
  });
});
