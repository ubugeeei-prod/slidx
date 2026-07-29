/**
 * How a report says a number out loud.
 *
 * A rehearsal report is read once, tired, immediately after giving the talk.
 * Every format here is chosen so that a sentence containing it parses on the
 * first read: `9s` rather than `0:09`, which inside prose reads as a time of
 * day, and a signed delta rather than a bare span, because "2m over" and
 * "2m under" are opposite pieces of news that a bare `2m` cannot distinguish.
 */

import { describe, expect, it } from "vitest";

import { formatDelta, formatList, formatSpan } from "../src/duration";

describe("saying how long something took", () => {
  it("reads a short span in seconds, which is how a speaker thinks about one", () => {
    expect(formatSpan(9_000)).toBe("9s");
  });

  it("reads a long span in minutes and seconds", () => {
    expect(formatSpan(150_000)).toBe("2m 30s");
  });

  it("drops the seconds when there are none, rather than saying 2m 0s", () => {
    expect(formatSpan(120_000)).toBe("2m");
  });

  it("never claims a precision a speaker could act on", () => {
    // Nobody paces a talk in milliseconds, and a report that printed them
    // would be claiming a measurement it does not have.
    expect(formatSpan(9_400)).toBe("9s");
    expect(formatSpan(9_600)).toBe("10s");
  });

  it("says a span without a sign, because the direction is the delta's job", () => {
    expect(formatSpan(-9_000)).toBe("9s");
  });
});

describe("saying how far off a budget something was", () => {
  it("names the direction, so the number is never ambiguous", () => {
    expect(formatDelta(120_000)).toBe("2m over");
    expect(formatDelta(-120_000)).toBe("2m under");
  });

  it("says a slide landed on its budget rather than printing a zero", () => {
    expect(formatDelta(0)).toBe("on budget");
  });

  it("treats a sub-second difference as landing on budget", () => {
    // Reporting "0s over" would be arithmetically true and would read as a
    // failure to hit a target the speaker in fact hit.
    expect(formatDelta(400)).toBe("on budget");
  });
});

describe("naming several slides in one sentence", () => {
  it("joins the last pair with `and`, because the advice is read aloud", () => {
    expect(formatList(["4", "9", "12"])).toBe("4, 9 and 12");
  });

  it("leaves a single item alone", () => {
    expect(formatList(["7"])).toBe("7");
  });

  it("joins a pair without a comma", () => {
    expect(formatList(["4", "9"])).toBe("4 and 9");
  });

  it("says nothing when there is nothing to name", () => {
    expect(formatList([])).toBe("");
  });
});
