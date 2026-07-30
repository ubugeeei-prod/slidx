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

function storage(initial?: string): RehearsalStorage & { value: string | null } {
  return {
    value: initial ?? null,
    getItem() {
      return this.value;
    },
    setItem(_key, value) {
      this.value = value;
    },
    removeItem() {
      this.value = null;
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
