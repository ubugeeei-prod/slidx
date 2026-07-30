/**
 * The presenter's clock.
 *
 * This is the specification. A talk timer is a small thing that fails in ways
 * that cost a speaker the room, so the behaviours worth stating are mostly
 * about what it does *wrong* under pressure:
 *
 * - It must not lose time when a laptop sleeps between rooms.
 * - It must keep counting past the budget, because a speaker who is over needs
 *   to know by how much — freezing at zero hides the number that matters.
 * - Pausing must be exact, because it is used mid-talk when a question runs
 *   long.
 *
 * The clock is injected. A timer tested with real time is a timer tested with
 * sleeps, which makes the suite slow and flaky at once.
 */

import { describe, expect, it } from "vite-plus/test";

import { createTimer, formatDuration, type Timer } from "../src/timer";

/** A clock the test moves by hand. */
function clock(start = 0) {
  let now = start;
  return {
    now: () => now,
    advance: (ms: number) => {
      now += ms;
    },
  };
}

function timerAt(budgetMs?: number): { timer: Timer; tick: (ms: number) => void } {
  const time = clock();
  const timer = createTimer({ now: time.now, budgetMs });
  return { timer, tick: time.advance };
}

describe("counting", () => {
  it("starts at zero and stopped", () => {
    const { timer } = timerAt();

    expect(timer.state().elapsedMs).toBe(0);
    expect(timer.state().running).toBe(false);
  });

  it("does not move until started", () => {
    const { timer, tick } = timerAt();
    tick(5_000);

    expect(timer.state().elapsedMs).toBe(0);
  });

  it("counts once started", () => {
    const { timer, tick } = timerAt();
    timer.start();
    tick(1_500);

    expect(timer.state().elapsedMs).toBe(1_500);
  });

  it("keeps counting past the budget", () => {
    // A speaker who is over time needs the number, not a frozen zero.
    const { timer, tick } = timerAt(60_000);
    timer.start();
    tick(75_000);

    expect(timer.state().elapsedMs).toBe(75_000);
    expect(timer.state().remainingMs).toBe(-15_000);
    expect(timer.state().overrun).toBe(true);
  });

  it("survives the machine sleeping", () => {
    // Elapsed time is derived from the clock, not accumulated from ticks, so
    // a lid closed between rooms does not lose the talk's start.
    const { timer, tick } = timerAt();
    timer.start();
    tick(45 * 60_000);

    expect(timer.state().elapsedMs).toBe(45 * 60_000);
  });

  it("continues from elapsed time restored by another presenter page", () => {
    const time = clock();
    const timer = createTimer({ now: time.now, initialElapsedMs: 45_000 });

    expect(timer.state().elapsedMs).toBe(45_000);
    expect(timer.state().running).toBe(false);

    timer.start();
    time.advance(5_000);
    expect(timer.state().elapsedMs).toBe(50_000);
  });

  it("never restores a negative elapsed time", () => {
    expect(createTimer({ initialElapsedMs: -1 }).state().elapsedMs).toBe(0);
  });
});

describe("pausing", () => {
  it("holds the elapsed time", () => {
    const { timer, tick } = timerAt();
    timer.start();
    tick(10_000);
    timer.pause();
    tick(30_000);

    expect(timer.state().elapsedMs).toBe(10_000);
    expect(timer.state().running).toBe(false);
  });

  it("resumes from where it stopped rather than from zero", () => {
    const { timer, tick } = timerAt();
    timer.start();
    tick(10_000);
    timer.pause();
    tick(30_000);
    timer.start();
    tick(5_000);

    expect(timer.state().elapsedMs).toBe(15_000);
  });

  it("ignores a second start while running", () => {
    // The keyboard shortcut is easy to press twice. Restarting the talk clock
    // by accident is unrecoverable.
    const { timer, tick } = timerAt();
    timer.start();
    tick(10_000);
    timer.start();
    tick(1_000);

    expect(timer.state().elapsedMs).toBe(11_000);
  });

  it("ignores a pause while already paused", () => {
    const { timer, tick } = timerAt();
    timer.start();
    tick(10_000);
    timer.pause();
    timer.pause();

    expect(timer.state().elapsedMs).toBe(10_000);
  });

  it("toggles", () => {
    const { timer, tick } = timerAt();
    timer.toggle();
    tick(1_000);
    expect(timer.state().running).toBe(true);

    timer.toggle();
    tick(1_000);
    expect(timer.state().elapsedMs).toBe(1_000);
  });
});

describe("resetting", () => {
  it("returns to zero and stops", () => {
    const { timer, tick } = timerAt();
    timer.start();
    tick(10_000);
    timer.reset();

    expect(timer.state().elapsedMs).toBe(0);
    expect(timer.state().running).toBe(false);
  });

  it("can be started again after a reset", () => {
    const { timer, tick } = timerAt();
    timer.start();
    tick(10_000);
    timer.reset();
    timer.start();
    tick(2_000);

    expect(timer.state().elapsedMs).toBe(2_000);
  });
});

describe("the budget", () => {
  it("reports what is left", () => {
    const { timer, tick } = timerAt(20 * 60_000);
    timer.start();
    tick(5 * 60_000);

    expect(timer.state().remainingMs).toBe(15 * 60_000);
    expect(timer.state().overrun).toBe(false);
  });

  it("warns before the end rather than at it", () => {
    // A warning that arrives as time expires is a warning that arrives too
    // late to change anything.
    const { timer, tick } = timerAt(20 * 60_000);
    timer.start();

    tick(15 * 60_000);
    expect(timer.state().status).toBe("on-time");

    tick(2 * 60_000);
    expect(timer.state().status).toBe("nearly-done");

    tick(4 * 60_000);
    expect(timer.state().status).toBe("over");
  });

  it("has no status to report without a budget", () => {
    const { timer, tick } = timerAt();
    timer.start();
    tick(60 * 60_000);

    expect(timer.state().status).toBe("untimed");
    expect(timer.state().remainingMs).toBeUndefined();
  });
});

describe("formatting", () => {
  it("uses minutes and seconds for a talk-length duration", () => {
    expect(formatDuration(0)).toBe("0:00");
    expect(formatDuration(9_000)).toBe("0:09");
    expect(formatDuration(65_000)).toBe("1:05");
    expect(formatDuration(20 * 60_000)).toBe("20:00");
  });

  it("adds hours only when there are hours", () => {
    // A workshop clock reading 90:00 is harder to read at a glance than 1:30:00.
    expect(formatDuration(60 * 60_000)).toBe("1:00:00");
    expect(formatDuration(90 * 60_000 + 5_000)).toBe("1:30:05");
  });

  it("shows an overrun as a signed value", () => {
    // "-2:00" is immediately two minutes over. "2:00" is ambiguous.
    expect(formatDuration(-120_000)).toBe("-2:00");
  });

  it("rounds towards zero so the display never shows a second that has not passed", () => {
    expect(formatDuration(1_999)).toBe("0:01");
    expect(formatDuration(-1_999)).toBe("-0:01");
  });
});
