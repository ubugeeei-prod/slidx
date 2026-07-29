/**
 * Recording where the time actually went.
 *
 * The specification is dominated by one fact about rehearsals: **most of them
 * are abandoned**. A speaker starts, gets interrupted, closes the laptop. So
 * the recorder is built so that stopping at any point still leaves a usable
 * measurement, and so that the measurement outlives the tab that made it.
 *
 * The second fact is that a talk is not a straight line. A speaker goes back to
 * re-explain a diagram, and that second visit is *more time on that slide*, not
 * a correction to the first. Keeping only the latest visit would report the
 * slide that ate four minutes as having taken thirty seconds.
 *
 * The clock is injected throughout. A rehearsal is forty minutes long, and a
 * suite that measured one with real time would be a suite nobody runs.
 */

import { describe, expect, it } from "vitest";

import { createRehearsal, restoreRehearsal, type RehearsalRecording } from "../src/rehearsal";

/** A clock a test drives by hand. */
function clock(): { now: () => number; advance: (ms: number) => void } {
  let current = 0;
  return {
    now: () => current,
    advance: (ms: number) => {
      current += ms;
    },
  };
}

const DECK = [
  { id: "intro", budgetMs: 60_000 },
  { id: "middle", budgetMs: 120_000 },
  { id: "end", budgetMs: 60_000 },
];

describe("recording dwell time", () => {
  it("charges the time between two visits to the slide that was on screen", () => {
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.visit("intro");
    time.advance(30_000);
    rehearsal.visit("middle");

    expect(rehearsal.toJSON().slides[0]?.actualMs).toBe(30_000);
  });

  it("adds a second visit to the first rather than replacing it", () => {
    // Going back to re-explain a diagram is more time on that slide. Keeping
    // only the last visit would report the slide that ate the talk as cheap.
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.visit("intro");
    time.advance(30_000);
    rehearsal.visit("middle");
    time.advance(10_000);
    rehearsal.visit("intro");
    time.advance(20_000);
    rehearsal.visit("end");

    const intro = rehearsal.toJSON().slides.find((slide) => slide.id === "intro");
    expect(intro?.actualMs).toBe(50_000);
    expect(intro?.visits).toBe(2);
  });

  it("counts the time being spent right now, not only what has been banked", () => {
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.visit("intro");
    time.advance(45_000);

    expect(rehearsal.state().slideMs).toBe(45_000);
    expect(rehearsal.state().elapsedMs).toBe(45_000);
  });

  it("records a slide the deck never declared rather than refusing the visit", () => {
    // A speaker jumping to a backup slide is spending real time. Refusing it
    // would make that time vanish from the only record of where time went.
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.visit("backup-appendix");
    time.advance(20_000);
    rehearsal.finish();

    const extra = rehearsal.toJSON().slides.find((slide) => slide.id === "backup-appendix");
    expect(extra?.actualMs).toBe(20_000);
    expect(extra?.budgetMs).toBeUndefined();
  });
});

describe("stopping the clock", () => {
  it("excludes paused time, so a long question does not become slide time", () => {
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.visit("intro");
    time.advance(10_000);
    rehearsal.pause();
    time.advance(600_000);
    rehearsal.resume();
    time.advance(10_000);

    expect(rehearsal.state().slideMs).toBe(20_000);
  });

  it("resumes on a navigation, because navigating is presenting", () => {
    // A speaker who answered a question and moved on should not have to press
    // two keys, one of which they will forget.
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.visit("intro");
    rehearsal.pause();
    rehearsal.visit("middle");
    time.advance(15_000);

    expect(rehearsal.state().status).toBe("recording");
    expect(rehearsal.state().slideMs).toBe(15_000);
  });

  it("has nothing to resume onto before the first slide was reached", () => {
    // Attributing that time to whichever slide is opened later would be an
    // invention, and an invention is worse than a gap.
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.resume();
    time.advance(30_000);

    expect(rehearsal.state().elapsedMs).toBe(0);
    expect(rehearsal.state().status).toBe("idle");
  });
});

describe("ending a rehearsal", () => {
  it("banks the slide still on screen when the speaker finishes", () => {
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.visit("end");
    time.advance(40_000);
    rehearsal.finish();

    expect(rehearsal.toJSON().slides.find((slide) => slide.id === "end")?.actualMs).toBe(40_000);
  });

  it("keeps the slide the speaker gave up on, which is the most useful single fact", () => {
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.visit("middle");
    time.advance(30_000);
    rehearsal.abandon();

    expect(rehearsal.toJSON().status).toBe("abandoned");
    expect(rehearsal.toJSON().currentSlide).toBe("middle");
  });

  it("stops the clock once it has ended, so a stray key cannot corrupt the numbers", () => {
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.visit("intro");
    rehearsal.finish();
    time.advance(60_000);
    rehearsal.visit("middle");

    expect(rehearsal.state().elapsedMs).toBe(0);
    expect(rehearsal.state().status).toBe("finished");
  });
});

describe("surviving the tab", () => {
  it("emits a snapshot on every change, so nothing is lost if the tab closes", () => {
    // Most rehearsals end by being abandoned rather than by anyone pressing
    // stop. Waiting to be asked for a recording means never being asked.
    const time = clock();
    const written: RehearsalRecording[] = [];
    const rehearsal = createRehearsal({
      slides: DECK,
      now: time.now,
      onChange: (recording) => written.push(recording),
    });

    rehearsal.visit("intro");
    time.advance(30_000);
    rehearsal.visit("middle");

    expect(written.length).toBe(2);
    expect(written.at(-1)?.slides[0]?.actualMs).toBe(30_000);
  });

  it("folds the slide currently being spoken into its snapshot", () => {
    const time = clock();
    const rehearsal = createRehearsal({ slides: DECK, now: time.now });

    rehearsal.visit("intro");
    time.advance(25_000);

    expect(rehearsal.toJSON().slides[0]?.actualMs).toBe(25_000);
  });

  it("picks a stored rehearsal back up with its totals intact", () => {
    const time = clock();
    const first = createRehearsal({ slides: DECK, now: time.now });

    first.visit("intro");
    time.advance(30_000);
    first.visit("middle");
    time.advance(20_000);

    const resumed = restoreRehearsal(first.toJSON(), { now: time.now });
    expect(resumed.state().elapsedMs).toBe(50_000);
  });

  it("comes back paused rather than running, so the reload is not charged to the talk", () => {
    // The gap between the snapshot and the restore is a page load, not
    // talking. Backdating it would make every recovered rehearsal look worse
    // than the talk that produced it.
    const time = clock();
    const first = createRehearsal({ slides: DECK, now: time.now });

    first.visit("intro");
    time.advance(30_000);

    const resumed = restoreRehearsal(first.toJSON(), { now: time.now });
    time.advance(600_000);

    expect(resumed.state().status).toBe("paused");
    expect(resumed.state().elapsedMs).toBe(30_000);
  });

  it("refuses a recording written by a version it does not understand", () => {
    // Loudly, because the alternative is a report full of zeroes that reads
    // like a rehearsal nobody gave.
    const recording = {
      ...createRehearsal().toJSON(),
      version: 99,
    } as unknown as RehearsalRecording;

    expect(() => restoreRehearsal(recording)).toThrow(/version/);
  });

  it("carries the tolerances it was recorded with, so a reloaded report is the same report", () => {
    const rehearsal = createRehearsal({ slides: DECK, toleranceMs: 5_000 });
    expect(restoreRehearsal(rehearsal.toJSON()).toJSON().toleranceMs).toBe(5_000);
  });
});
