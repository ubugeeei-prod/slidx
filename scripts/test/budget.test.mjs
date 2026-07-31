/**
 * Reading a built page for what it weighs and what it fetches.
 *
 * Both readers here got it wrong on their first run, in opposite directions,
 * and each mistake looks exactly like a real finding:
 *
 * - counting `application/ld+json` as JavaScript reported slidx's most-repeated
 *   claim as broken while it was being kept
 * - following only `src` missed the step runtime, which arrives as an `import`
 *   inside an inline module, and reported a deck at a third of its weight
 *
 * A budget that measures the wrong thing is worse than no budget: one of those
 * fails a build nobody broke, and the other passes one nobody is watching.
 */

import { describe, expect, it } from "vite-plus/test";

import {
  BUDGETS,
  executableScripts,
  overBudget,
  referencesIn,
  splitPages,
  unmeasured,
} from "../budget.mjs";

describe("what counts as JavaScript on a page", () => {
  it("counts a module, which is how a staged slide runs", () => {
    const found = executableScripts('<script type="module">import "./runtime.js";</script>');

    expect(found).toHaveLength(1);
    expect(found[0].body).toBe('import "./runtime.js";');
  });

  it("counts a script with no type at all", () => {
    expect(executableScripts("<script>go()</script>")).toHaveLength(1);
  });

  it("does not count the JSON-LD every page carries for a crawler", () => {
    // The mistake that reported the no-JavaScript claim broken on the first run
    // this check ever made.
    expect(
      executableScripts('<script type="application/ld+json">{"@type":"Event"}</script>'),
    ).toEqual([]);
  });

  it("does not count anything else a page parks in a script tag", () => {
    expect(executableScripts('<script type="importmap">{}</script>')).toEqual([]);
    expect(executableScripts('<script type="text/template"><b>hi</b></script>')).toEqual([]);
  });

  it("reads the type whatever case it was written in", () => {
    expect(executableScripts('<script type="Module">go()</script>')).toHaveLength(1);
  });

  it("keeps the attributes, so a src can still be followed", () => {
    const [only] = executableScripts('<script type="module" src="/slides/runtime.js"></script>');

    expect(only.attributes).toContain('src="/slides/runtime.js"');
    expect(only.body).toBe("");
  });
});

describe("what a page asks a browser to fetch", () => {
  it("follows an import inside an inline module, which is how the runtime arrives", () => {
    // The mistake that made a deck look a third of its weight.
    const page = '<script type="module">import { createStage } from "/slides/runtime.js";</script>';

    expect(referencesIn(page)).toEqual(["/slides/runtime.js"]);
  });

  it("follows a dynamic import too", () => {
    expect(referencesIn('<script type="module">await import("./late.js")</script>')).toEqual([
      "./late.js",
    ]);
  });

  it("follows stylesheets, scripts and images", () => {
    const page = [
      '<link rel="stylesheet" href="./effects.css">',
      '<script src="./boot.js"></script>',
      '<img src="./square.png" alt="">',
    ].join("");

    expect(referencesIn(page)).toEqual(["./effects.css", "./boot.js", "./square.png"]);
  });

  it("leaves a page's other links alone", () => {
    // `rel="next"` and `rel="canonical"` are on every page slidx emits, and a
    // browser fetches neither.
    const page = '<link rel="next" href="2/"><link rel="canonical" href="https://x/slides/">';

    expect(referencesIn(page)).toEqual([]);
  });

  it("leaves another origin out, because a deck that reached one is a lint error", () => {
    const page = '<img src="https://cdn.example/logo.png"><img src="data:image/png;base64,AA">';

    expect(referencesIn(page)).toEqual([]);
  });

  it("names a thing once however many pages want it", () => {
    const page = '<img src="./a.png"><img src="./a.png">';

    expect(referencesIn(page)).toEqual(["./a.png"]);
  });
});

describe("holding a measurement to a figure", () => {
  const figures = [
    { name: "small", limit: 100, protects: "nothing in particular" },
    { name: "smaller", limit: 10, protects: "nothing in particular" },
  ];

  it("says nothing when everything is inside", () => {
    expect(overBudget({ small: 100, smaller: 9 }, figures)).toEqual([]);
  });

  it("reports how far over, so the size of the problem is in the message", () => {
    const [over] = overBudget({ small: 140, smaller: 1 }, figures);

    expect(over.name).toBe("small");
    expect(over.over).toBe(40);
  });

  it("treats the limit itself as inside", () => {
    expect(overBudget({ small: 100 }, figures)).toEqual([]);
  });

  it("reports a figure nothing measured, which is how a check stops checking", () => {
    // A budget whose measurement quietly stops being taken passes forever.
    expect(unmeasured({ small: 1 }, figures)).toEqual(["smaller"]);
  });
});

describe("the figures themselves", () => {
  it("gives every one a reason it exists", () => {
    for (const budget of BUDGETS) expect(budget.protects.length).toBeGreaterThan(20);
  });

  it("allows a slide with no steps not one byte", () => {
    // The only figure here that is a claim rather than a measurement, and the
    // one slidx repeats most often.
    const still = BUDGETS.find((budget) => budget.name.includes("no steps"));

    expect(still?.limit).toBe(0);
  });
});

describe("which figure a page belongs to", () => {
  it("keeps a snippet page out of the per-slide average", () => {
    // It is a fraction of the weight of a slide, so averaging it in makes every
    // slide look lighter the more code a deck shares — which is backwards.
    const { slides, snippets } = splitPages([
      "slides/index.html",
      "slides/2/index.html",
      "slides/snippets/retry.html",
    ]);

    expect(slides).toEqual(["slides/index.html", "slides/2/index.html"]);
    expect(snippets).toEqual(["slides/snippets/retry.html"]);
  });

  it("still counts it, because a phone in the room downloads it", () => {
    const { slides, snippets } = splitPages(["slides/snippets/a.html"]);

    expect(slides).toEqual([]);
    expect(snippets).toHaveLength(1);
  });

  it("has a figure for it, which is the tightest one here", () => {
    const page = BUDGETS.find((budget) => budget.name.includes("snippet"));

    expect(page).toBeDefined();
    expect(page.limit).toBeLessThan(5_000);
  });
});
