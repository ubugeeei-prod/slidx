/**
 * Which slides are getting worse.
 *
 * One rehearsal tells a speaker where the time went. Three tell them whether
 * the changes they made are working, and that is a different and better
 * question — it is the only one that distinguishes a slide that is over budget
 * because it is hard from a slide that is over budget and *drifting*.
 *
 * The comparison is against the **previous** run rather than the first. A
 * speaker who cut slide 7 from 4m to 2m to 2m30 has a slide that is getting
 * worse again; measured against the first run it would read as a 90-second
 * improvement and the drift would be invisible until the stage.
 *
 * The other half is knowing what not to say. A slide that is over budget but
 * two minutes faster than last time is being fixed, and telling the speaker to
 * cut it again would send them to undo work that is already working.
 */

import { describe, expect, it } from "vite-plus/test";

import { trackRehearsals } from "../src/trend";
import { minutes, recording, skipped, slide } from "./support";

describe("having something to compare", () => {
  it("says so plainly when there are no rehearsals yet", () => {
    const trend = trackRehearsals([]);

    expect(trend.runs).toBe(0);
    expect(trend.slides).toEqual([]);
    expect(trend.note).toContain("No rehearsal");
  });

  it("refuses to call a single rehearsal a trend", () => {
    // A first run is a measurement, not a direction. Claiming otherwise would
    // be the package inventing a comparison it does not have.
    const trend = trackRehearsals([recording([slide("a", minutes(2), minutes(1))])]);

    expect(trend.runs).toBe(1);
    expect(trend.slides[0]?.direction).toBe("new");
    expect(trend.regressions).toEqual([]);
    expect(trend.note).toContain("second");
  });

  it("counts every run it was given, not just the two it compares", () => {
    const runs = [
      recording([slide("a", minutes(4), minutes(1))]),
      recording([slide("a", minutes(3), minutes(1))]),
      recording([slide("a", minutes(2), minutes(1))]),
    ];

    expect(trackRehearsals(runs).runs).toBe(3);
  });
});

describe("comparing the latest run against the one before it", () => {
  it("calls a slide slower when it took materially longer than last time", () => {
    const trend = trackRehearsals([
      recording([slide("a", minutes(1), minutes(1))]),
      recording([slide("a", minutes(3), minutes(1))]),
    ]);

    expect(trend.slides[0]?.direction).toBe("slower");
    expect(trend.slides[0]?.deltaMs).toBe(minutes(2));
    expect(trend.slides[0]?.previousMs).toBe(minutes(1));
  });

  it("calls a slide faster when the speaker got it under control", () => {
    const trend = trackRehearsals([
      recording([slide("a", minutes(4), minutes(1))]),
      recording([slide("a", minutes(1), minutes(1))]),
    ]);

    expect(trend.slides[0]?.direction).toBe("faster");
  });

  it("treats a small difference between runs as steady, not as drift", () => {
    // Nobody gives the same slide twice to the second. A trend that reported
    // four seconds as a regression would report noise every single run.
    const trend = trackRehearsals([
      recording([slide("a", 60_000, minutes(1))]),
      recording([slide("a", 64_000, minutes(1))]),
    ]);

    expect(trend.slides[0]?.direction).toBe("steady");
  });

  it("measures against the previous run rather than the first", () => {
    // 4m → 2m → 2m30 is a slide drifting back, and comparing against the first
    // run would call it a 90-second improvement.
    const trend = trackRehearsals([
      recording([slide("a", minutes(4), minutes(1))]),
      recording([slide("a", minutes(2), minutes(1))]),
      recording([slide("a", minutes(2) + 30_000, minutes(1))]),
    ]);

    expect(trend.slides[0]?.direction).toBe("slower");
  });

  it("calls a slide new when the previous run never reached it", () => {
    const trend = trackRehearsals([
      recording([slide("a", minutes(1), minutes(1)), skipped("b", minutes(1))], "abandoned"),
      recording([slide("a", minutes(1), minutes(1)), slide("b", minutes(3), minutes(1))]),
    ]);

    expect(trend.slides[1]?.direction).toBe("new");
  });

  it("leaves out a slide the latest run never reached, having no dwell to compare", () => {
    const trend = trackRehearsals([
      recording([slide("a", minutes(1), minutes(1)), slide("b", minutes(3), minutes(1))]),
      recording([slide("a", minutes(1), minutes(1)), skipped("b", minutes(1))], "abandoned"),
    ]);

    expect(trend.slides.map((entry) => entry.id)).toEqual(["a"]);
  });
});

describe("naming what is getting worse", () => {
  it("lists the slides that slipped, worst first", () => {
    const trend = trackRehearsals([
      recording([
        slide("a", minutes(1), minutes(1)),
        slide("b", minutes(1), minutes(1)),
        slide("c", minutes(1), minutes(1)),
      ]),
      recording([
        slide("a", minutes(2), minutes(1)),
        slide("b", minutes(4), minutes(1)),
        slide("c", minutes(1), minutes(1)),
      ]),
    ]);

    expect(trend.regressions.map((entry) => entry.index)).toEqual([2, 1]);
  });

  it("names the slipping slide and by how much", () => {
    const trend = trackRehearsals([
      recording([slide("a", minutes(1), minutes(1))]),
      recording([slide("a", minutes(3), minutes(1))]),
    ]);

    expect(trend.note).toContain("Slide 1");
    expect(trend.note).toContain("2m");
  });

  it("says a slide is still over budget as well as slipping", () => {
    // Over budget and drifting is the one combination worth acting on first.
    const trend = trackRehearsals([
      recording([slide("a", minutes(2), minutes(1))]),
      recording([slide("a", minutes(4), minutes(1))]),
    ]);

    expect(trend.note).toContain("over budget");
  });

  it("does not call a slide a regression for slipping while staying inside its budget", () => {
    // Growing from 20s to 50s inside a 2m budget is a slide with room. Naming
    // it would spend the speaker's attention on the one place they have slack.
    const trend = trackRehearsals([
      recording([slide("a", 20_000, minutes(2))]),
      recording([slide("a", 50_000, minutes(2))]),
    ]);

    expect(trend.slides[0]?.direction).toBe("slower");
    expect(trend.regressions).toEqual([]);
  });

  it("says nothing slipped when nothing did", () => {
    const trend = trackRehearsals([
      recording([slide("a", minutes(1), minutes(1))]),
      recording([slide("a", minutes(1), minutes(1))]),
    ]);

    expect(trend.regressions).toEqual([]);
    expect(trend.note).toContain("Nothing");
  });

  it("tells a speaker their cut is working rather than sending them to re-cut it", () => {
    const trend = trackRehearsals([
      recording([slide("a", minutes(5), minutes(1))]),
      recording([slide("a", minutes(2), minutes(1))]),
    ]);

    expect(trend.note).toContain("faster");
  });

  it("names a few slides rather than every slide that moved", () => {
    const previous = Array.from({ length: 8 }, (_, at) => slide(`s${at}`, minutes(1), minutes(1)));
    const latest = Array.from({ length: 8 }, (_, at) => slide(`s${at}`, minutes(3), minutes(1)));

    const trend = trackRehearsals([recording(previous), recording(latest)]);

    expect(trend.regressions.length).toBe(8);
    // The list is the table's job. The sentence names what a speaker can hold.
    expect(trend.note.match(/Slide|slide/g)?.length ?? 0).toBeLessThanOrEqual(2);
  });
});
