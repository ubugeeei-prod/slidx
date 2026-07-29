/**
 * Turning twenty numbers into one sentence.
 *
 * "You are six minutes over" is the fact a stopwatch already gave the speaker,
 * and it is not a plan. An overrun is almost never spread evenly: three slides
 * ate five of the six minutes and seventeen slides were fine. Naming those
 * three is the difference between a report that gets a talk cut and a report
 * that gets closed.
 *
 * The hard part is knowing when *not* to name slides. An overrun genuinely
 * spread across a whole deck is a deck that is too long, and pointing at its
 * three worst slides would send a speaker to trim thirty seconds off each while
 * the real answer is to drop a section. So there are as many tests here about
 * staying quiet as there are about speaking up.
 */

import { describe, expect, it } from "vite-plus/test";

import { dominantSlides } from "../src/advice";
import { buildReport } from "../src/report";
import { minutes, recording, skipped, slide } from "./support";

/** The advice for a set of slides, as a speaker reads it. */
function adviceFor(...slides: Parameters<typeof recording>[0]): string {
  return buildReport(recording(slides)).advice;
}

describe("finding where the time actually went", () => {
  it("names the one slide that ate the overrun", () => {
    const report = buildReport(
      recording([
        slide("a", minutes(1), minutes(1)),
        slide("b", minutes(5), minutes(1)),
        slide("c", minutes(1), minutes(1)),
      ]),
    );

    expect(report.dominant.map((entry) => entry.index)).toEqual([2]);
  });

  it("orders the slides it names worst first", () => {
    const report = buildReport(
      recording([
        slide("a", minutes(3), minutes(1)),
        slide("b", minutes(6), minutes(1)),
        slide("c", minutes(1), minutes(1)),
      ]),
    );

    expect(report.dominant.map((entry) => entry.index)).toEqual([2, 1]);
  });

  it("names at most three, because that is what a speaker walks away holding", () => {
    const long = Array.from({ length: 10 }, (_, at) => slide(`s${at}`, minutes(3), minutes(1)));

    expect(dominantSlides(buildReport(recording(long)).slides).length).toBeLessThanOrEqual(3);
  });

  it("stays quiet when the overrun is spread across the whole deck", () => {
    // Ten slides each a little long is a deck that is too long. Naming its
    // three worst would send the speaker to trim seconds while the real
    // answer is to drop a section.
    const long = Array.from({ length: 10 }, (_, at) => slide(`s${at}`, minutes(3), minutes(1)));

    expect(dominantSlides(buildReport(recording(long)).slides)).toEqual([]);
  });

  it("ignores slides that are inside tolerance, so noise never becomes a culprit", () => {
    // Otherwise a stack of six-second drifts would accumulate into a slide
    // being blamed for the noise floor.
    const report = buildReport(
      recording([slide("a", 62_000, minutes(1)), slide("b", 63_000, minutes(1))]),
    );

    expect(report.dominant).toEqual([]);
  });

  it("names the same slides in the same order every time, so two runs can be diffed", () => {
    const tied = recording([
      slide("a", minutes(3), minutes(1)),
      slide("b", minutes(3), minutes(1)),
    ]);

    expect(dominantSlides(buildReport(tied).slides).map((entry) => entry.index)).toEqual([1, 2]);
  });

  it("reports each named slide's share of the overrun", () => {
    const report = buildReport(
      recording([slide("a", minutes(4), minutes(1)), slide("b", minutes(2), minutes(1))]),
    );

    expect(report.dominant[0]?.share).toBeCloseTo(0.75);
  });
});

describe("saying it in a sentence", () => {
  it("names the slide and what it cost, rather than restating the total", () => {
    const advice = adviceFor(
      slide("a", minutes(1), minutes(1)),
      slide("b", minutes(5), minutes(1)),
      slide("c", minutes(1), minutes(1)),
    );

    expect(advice).toContain("Slide 2");
    expect(advice).toContain("5m");
    expect(advice).toContain("1m");
  });

  it("names several slides in a voice a person would read aloud", () => {
    const advice = adviceFor(
      slide("a", minutes(4), minutes(1)),
      slide("b", minutes(4), minutes(1)),
      slide("c", minutes(1), minutes(1)),
    );

    expect(advice).toContain("slides 1 and 2");
  });

  it("says the deck is long when no few slides are to blame", () => {
    const long = Array.from({ length: 10 }, (_, at) => slide(`s${at}`, minutes(3), minutes(1)));

    expect(adviceFor(...long)).toContain("spread");
  });

  it("still names a slide that ran long inside a talk that finished early", () => {
    // The slide that will sink the talk on a day the room asks questions.
    const advice = adviceFor(
      slide("a", minutes(4), minutes(1)),
      slide("b", minutes(1), minutes(10)),
    );

    expect(advice).toContain("under");
    expect(advice).toContain("Slide 1");
  });

  it("reports a talk that fit without inventing something to fix", () => {
    const advice = adviceFor(
      slide("a", minutes(1), minutes(1)),
      slide("b", minutes(2), minutes(2)),
    );

    expect(advice).toContain("on budget");
    expect(advice).not.toContain("Slide");
  });

  it("asks for the missing budgets when there is no total to compare against", () => {
    expect(adviceFor(slide("a", minutes(3)))).toContain("budget:");
  });

  it("still names an over-budget slide in a deck with no total", () => {
    const advice = adviceFor(slide("a", minutes(4), minutes(1)), slide("b", minutes(2)));

    // Lower case, because here the slide is named mid-sentence rather than
    // opening one. The advice is prose a speaker reads, not a log line.
    expect(advice).toContain("slide 1 took 4m against a 1m budget");
  });
});

describe("labelling a rehearsal that did not finish", () => {
  it("says how much of the talk the numbers cover", () => {
    // "On budget" for a third of a talk is the most misleading thing this
    // package could say.
    const report = buildReport(
      recording(
        [slide("a", minutes(1), minutes(1)), skipped("b", minutes(1)), skipped("c", minutes(1))],
        "abandoned",
      ),
    );

    expect(report.advice).toContain("1 of 3");
  });

  it("says nothing was recorded when nothing was", () => {
    expect(buildReport(recording([], "abandoned")).advice).toContain("Nothing");
  });

  it("says no slide was reached when the speaker gave up immediately", () => {
    const report = buildReport(recording([skipped("a", minutes(1))], "abandoned"));
    expect(report.advice).toContain("No slide was reached");
  });
});
