/**
 * Am I going to make it?
 *
 * The timer answers "how long have I been talking", which is not the question
 * a speaker actually has on stage. The question is whether the time left is
 * enough for the slides left, and that depends on where you are in the deck —
 * something the clock cannot see.
 *
 * The failure modes guarded here are all about being *trusted*, because an
 * indicator a speaker learns to ignore is worse than none: it took up the one
 * piece of screen they can glance at.
 *
 * - **Flicker.** A reading that swings between behind and ahead every few
 *   seconds gets ignored by the second slide. There is a dead zone, and being
 *   partway through a slide is on pace for the whole of that slide's budget.
 * - **False precision.** "Three minutes behind" means something different when
 *   it came from the author's own per-slide budgets than when it was divided
 *   out of a slide count. The basis is reported, never hidden.
 * - **Judging before there is anything to judge.** No slot means no pace, and
 *   a talk that has not started is not behind.
 * - **Suggesting the speaker cut their argument.** Only slides the author
 *   marked optional may ever be offered, and only ones still ahead of them.
 */

import { describe, expect, it } from "vitest";

import { assessPace, describePace, type PaceSlide } from "../src/pace";

const MINUTE = 60_000;

/** Four slides, five minutes each, in a twenty-minute slot. */
function evenDeck(): PaceSlide[] {
  return [0, 1, 2, 3].map((index) => ({
    index,
    title: `Slide ${index + 1}`,
    budgetMs: 5 * MINUTE,
  }));
}

function at(position: number, elapsedMs: number, slides: PaceSlide[] = evenDeck()) {
  return assessPace({ slides, position, elapsedMs, budgetMs: 20 * MINUTE, running: true });
}

describe("reading the pace", () => {
  it("is on pace anywhere inside the current slide's own budget", () => {
    // The whole of slide 2 is minutes 5 to 10. Being at minute 7 is not
    // behind, and a reading that said so would be wrong for most of every
    // slide a speaker is on.
    for (const minute of [5, 7, 9.9]) {
      expect(at(1, minute * MINUTE).pace, `minute ${minute}`).toBe("on-pace");
    }
  });

  it("is behind once the current slide has overrun its budget", () => {
    const state = at(1, 12 * MINUTE);

    expect(state.pace).toBe("behind");
    expect(state.driftMs).toBe(2 * MINUTE);
  });

  it("is ahead when the slide arrived earlier than the deck expected", () => {
    const state = at(3, 9 * MINUTE);

    expect(state.pace).toBe("ahead");
    expect(state.driftMs).toBe(-6 * MINUTE);
  });

  it("reports the window the deck expects, not a single instant", () => {
    const state = at(2, 11 * MINUTE);

    expect(state.expectedFromMs).toBe(10 * MINUTE);
    expect(state.expectedToMs).toBe(15 * MINUTE);
  });

  it("cannot be behind on the first slide before any budget has been spent", () => {
    expect(at(0, 30_000).pace).toBe("on-pace");
  });
});

describe("the dead zone", () => {
  it("ignores a drift too small to act on", () => {
    // Twenty seconds over is noise. A speaker cannot act on it, and a display
    // that flips to a warning for it is a display they stop reading.
    expect(at(1, 10 * MINUTE + 20_000).pace).toBe("on-pace");
  });

  it("scales the tolerance with the slot, because a minute is not always a minute", () => {
    // A minute of slack in a five-minute lightning talk is a fifth of it.
    const lightning: PaceSlide[] = [0, 1].map((index) => ({ index, budgetMs: 2.5 * MINUTE }));
    const state = assessPace({
      slides: lightning,
      position: 1,
      elapsedMs: 5 * MINUTE + 40_000,
      budgetMs: 5 * MINUTE,
      running: true,
    });

    expect(state.pace).toBe("behind");
  });
});

describe("what the number is worth", () => {
  it("uses the author's own budgets when every slide has one", () => {
    expect(at(1, 6 * MINUTE).basis).toBe("budgets");
  });

  it("says so when it divided the slot by the slide count instead", () => {
    // Still useful, and it must not be presented as if the author said it.
    const bare: PaceSlide[] = [0, 1, 2, 3].map((index) => ({ index }));

    expect(at(1, 6 * MINUTE, bare).basis).toBe("uniform");
  });

  it("uses the budgets it has and shares the rest out, when only some are declared", () => {
    // The common case: budgets on the slides the author worried about.
    // Throwing away real numbers to fall back to a slide count would be worse.
    const mixed: PaceSlide[] = [
      { index: 0, budgetMs: 2 * MINUTE },
      { index: 1 },
      { index: 2 },
      { index: 3, budgetMs: 6 * MINUTE },
    ];
    const state = at(1, 3 * MINUTE, mixed);

    expect(state.basis).toBe("mixed");
    // Twelve minutes left over two undeclared slides: six each.
    expect(state.expectedFromMs).toBe(2 * MINUTE);
    expect(state.expectedToMs).toBe(8 * MINUTE);
  });

  it("refuses to judge a talk with no slot at all", () => {
    const state = assessPace({ slides: evenDeck(), position: 1, elapsedMs: MINUTE, running: true });

    expect(state.pace).toBe("unknown");
    expect(state.basis).toBe("none");
    expect(state.driftMs).toBeUndefined();
  });

  it("refuses to judge a talk that has not started", () => {
    const state = assessPace({
      slides: evenDeck(),
      position: 0,
      elapsedMs: 0,
      budgetMs: 20 * MINUTE,
      running: false,
    });

    expect(state.pace).toBe("unknown");
  });
});

describe("what can be dropped", () => {
  function withOptional(): PaceSlide[] {
    return [
      { index: 0, budgetMs: 5 * MINUTE },
      { index: 1, budgetMs: 5 * MINUTE, optional: true, title: "A tangent" },
      { index: 2, budgetMs: 5 * MINUTE },
      { index: 3, budgetMs: 5 * MINUTE, optional: true, title: "Bonus demo" },
    ];
  }

  it("offers only slides the author marked optional", () => {
    // The author decided what the argument is. A tool that suggested cutting
    // the core of it is a tool nobody takes advice from twice.
    const state = at(0, 12 * MINUTE, withOptional());

    expect(state.skippable.map((slide) => slide.title)).toEqual(["A tangent", "Bonus demo"]);
  });

  it("offers only slides still ahead of the speaker", () => {
    // Slide 2 is already behind them, and cannot be skipped now.
    const state = at(2, 20 * MINUTE, withOptional());

    expect(state.skippable.map((slide) => slide.title)).toEqual(["Bonus demo"]);
  });

  it("adds up what dropping them would recover", () => {
    expect(at(0, 12 * MINUTE, withOptional()).recoverableMs).toBe(10 * MINUTE);
  });

  it("has nothing to offer on the last slide", () => {
    const state = at(3, 30 * MINUTE, withOptional());

    expect(state.skippable).toEqual([]);
    expect(state.recoverableMs).toBe(0);
  });

  it("has nothing to offer in a deck that marked nothing optional", () => {
    expect(at(0, 12 * MINUTE).skippable).toEqual([]);
  });
});

describe("saying it in one line", () => {
  it("names the slides worth dropping, because that is the action", () => {
    const slides: PaceSlide[] = [
      { index: 0, budgetMs: 5 * MINUTE },
      { index: 1, budgetMs: 5 * MINUTE, optional: true, title: "A tangent" },
    ];
    const line = describePace(at(0, 12 * MINUTE, slides));

    expect(line).toContain("behind");
    expect(line).toContain("A tangent");
  });

  it("says to wrap up when there is nothing left to drop", () => {
    // Being behind on the last slide is not a cutting problem any more.
    expect(describePace(at(3, 30 * MINUTE))).toMatch(/wrap up/i);
  });

  it("stays quiet when the pace is fine", () => {
    expect(describePace(at(1, 7 * MINUTE))).toBe("on pace");
  });

  it("says nothing rather than guessing when it cannot judge", () => {
    const state = assessPace({ slides: evenDeck(), position: 0, elapsedMs: 0, running: false });

    expect(describePace(state)).toBe("");
  });
});
