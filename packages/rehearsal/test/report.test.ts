/**
 * The rehearsal, diffed against the plan.
 *
 * The point of the report is that the answer is **per slide**. "You ran six
 * minutes over" is what a stopwatch already said, and it is the one fact that
 * does not tell a speaker what to cut. "Slide 7 took four minutes and was
 * budgeted one" does.
 *
 * Two judgements dominate these tests, because getting either wrong produces a
 * number that reads authoritative and is wrong:
 *
 * - A **partial budget total is not reported**. If any slide in the comparison
 *   declares no budget, there is no total — a partial sum looks like a whole
 *   one, and `slidx_core` makes the same call for the same reason.
 * - An **unfinished rehearsal is diffed against the slides it reached**. A
 *   speaker who gave up on slide 5 of 20 is not "fifteen minutes under"; that
 *   number describes the fifteen slides they did not give.
 */

import { describe, expect, it } from "vite-plus/test";

import { buildReport } from "../src/report";
import { minutes, recording, skipped, slide } from "./support";

describe("judging one slide against its budget", () => {
  it("says a slide that ran long is over, by how much", () => {
    const report = buildReport(recording([slide("deep", minutes(4), minutes(1))]));

    expect(report.slides[0]?.verdict).toBe("over");
    expect(report.slides[0]?.deltaMs).toBe(minutes(3));
  });

  it("says a slide that ran short is under", () => {
    const report = buildReport(recording([slide("quick", minutes(1), minutes(3))]));
    expect(report.slides[0]?.verdict).toBe("under");
  });

  it("treats a small drift as delivery rather than a plan to fix", () => {
    // A speaker cannot pace a slide to the second. Flagging a four-second
    // difference would be reporting their breathing.
    const report = buildReport(recording([slide("fine", 64_000, 60_000)]));
    expect(report.slides[0]?.verdict).toBe("on-budget");
  });

  it("reports a slide that was never opened as skipped, not as under", () => {
    // Telling a speaker they were two minutes under on a slide they never
    // showed is noise dressed as advice.
    const report = buildReport(recording([skipped("cut", minutes(2))]));
    expect(report.slides[0]?.verdict).toBe("skipped");
  });

  it("reports a slide with no declared budget as unbudgeted rather than guessing", () => {
    const report = buildReport(recording([slide("extra", minutes(2))]));

    expect(report.slides[0]?.verdict).toBe("unbudgeted");
    expect(report.slides[0]?.deltaMs).toBeUndefined();
  });

  it("numbers slides the way a speaker refers to them", () => {
    const report = buildReport(recording([slide("a", 1_000, 1_000), slide("b", 1_000, 1_000)]));

    expect(report.slides.map((entry) => entry.index)).toEqual([1, 2]);
  });

  it("keeps the visit count, because coming back to a slide is why it ran long", () => {
    const report = buildReport(recording([slide("diagram", minutes(4), minutes(1), 3)]));
    expect(report.slides[0]?.visits).toBe(3);
  });
});

describe("judging the whole talk against the slot", () => {
  it("sums the budgets when every slide declares one", () => {
    const report = buildReport(
      recording([slide("a", minutes(2), minutes(1)), slide("b", minutes(2), minutes(1))]),
    );

    expect(report.totals.budgetMs).toBe(minutes(2));
    expect(report.totals.deltaMs).toBe(minutes(2));
    expect(report.totals.verdict).toBe("over");
  });

  it("refuses to total a deck where some slides declare no budget", () => {
    // A partial sum looks like a whole one, and a speaker would read it as
    // their talk fitting a slot it does not fit.
    const report = buildReport(
      recording([slide("a", minutes(2), minutes(1)), slide("b", minutes(2))]),
    );

    expect(report.totals.budgetMs).toBeUndefined();
    expect(report.totals.verdict).toBe("unbudgeted");
  });

  it("lets the whole talk drift further than any one slide before saying so", () => {
    // The last minute of a slot is buffer every speaker already leaves.
    const report = buildReport(
      recording([slide("a", minutes(1) + 20_000, minutes(1)), slide("b", minutes(1), minutes(1))]),
    );

    expect(report.slides[0]?.verdict).toBe("over");
    expect(report.totals.verdict).toBe("on-budget");
  });
});

describe("a rehearsal that was abandoned", () => {
  it("compares only the slides that were reached", () => {
    // Fifteen unreached slides are not fifteen minutes of running short.
    const report = buildReport(
      recording([slide("a", minutes(3), minutes(1)), skipped("b", minutes(10))], "abandoned"),
    );

    expect(report.totals.basis).toBe("reached");
    expect(report.totals.budgetMs).toBe(minutes(1));
    // Counted against the whole deck's 11m this would read as 8m under, which
    // describes the slide the speaker never gave.
    expect(report.totals.verdict).toBe("over");
  });

  it("compares against the whole deck once the speaker says the talk is over", () => {
    // A slide skipped on the day is still in the talk they intend to give.
    const report = buildReport(
      recording([slide("a", minutes(2), minutes(1)), skipped("b", minutes(10))], "finished"),
    );

    expect(report.totals.basis).toBe("deck");
    expect(report.totals.budgetMs).toBe(minutes(11));
  });

  it("marks itself as partial so its numbers are never read as a whole talk", () => {
    const report = buildReport(recording([slide("a", minutes(2), minutes(1))], "abandoned"));

    expect(report.complete).toBe(false);
    expect(report.status).toBe("abandoned");
  });

  it("says how far it got", () => {
    const report = buildReport(
      recording([slide("a", minutes(1), minutes(1)), skipped("b", minutes(1))], "abandoned"),
    );

    expect(report.totals.slidesVisited).toBe(1);
    expect(report.totals.slidesTotal).toBe(2);
  });

  it("survives being abandoned before a single slide was reached", () => {
    // `every` over an empty list is true, so without saying so explicitly the
    // report would claim a budget of zero and a verdict of on-budget.
    const report = buildReport(recording([skipped("a", minutes(1))], "abandoned"));

    expect(report.totals.verdict).toBe("unbudgeted");
    expect(report.totals.actualMs).toBe(0);
  });

  it("survives a recording with no slides at all", () => {
    expect(() => buildReport(recording([], "abandoned"))).not.toThrow();
  });
});

describe("being recomputable", () => {
  it("produces the same report from the same recording", () => {
    // No clock and no storage: this is what lets a report be rebuilt after a
    // reload and compared with the one from last week.
    const stored = recording([slide("a", minutes(4), minutes(1))]);
    expect(buildReport(stored)).toEqual(buildReport(stored));
  });
});
