/**
 * The flatness rule, checked on the checker.
 *
 * `scripts/check-flat.mjs` is a gate in CI, so the thing that decides what
 * counts as a shadow has to be right about two opposite cases: it must catch a
 * declaration, and it must not catch a sentence saying declarations like that
 * are forbidden. This repository is full of the second kind — the built-in
 * themes' module docs, the editor stylesheet's header, the mark's own SVG
 * comment — and a checker that fired on its own documentation would be turned
 * off within a week.
 */

import { describe, expect, it } from "vite-plus/test";

import { EXEMPT, findFlatness, shippedFiles } from "../flat.mjs";

describe("what counts as a shadow", () => {
  it("reports a box-shadow declaration", () => {
    const found = findFlatness(".panel {\n  box-shadow: 0 1px 2px #0003;\n}\n");

    expect(found).toHaveLength(1);
    expect(found[0].construct).toBe("box-shadow");
  });

  it("reports a text-shadow declaration", () => {
    expect(findFlatness("h1 { text-shadow: 0 1px 0 white; }")).toHaveLength(1);
  });

  it("reports a drop-shadow filter", () => {
    expect(findFlatness("img { filter: drop-shadow(0 2px 4px black); }")).toHaveLength(1);
  });

  it("reports a shadow written as a JavaScript style property", () => {
    // An island rendering inline styles is a stylesheet with different
    // punctuation, and it reaches the same page.
    expect(findFlatness('<div style={{ boxShadow: "0 1px 2px" }} />')).toHaveLength(1);
    expect(findFlatness("element.style.textShadow = '0 1px 0 red';")).toHaveLength(1);
  });

  it("reports an SVG drop-shadow primitive", () => {
    expect(findFlatness('<filter id="s"><feDropShadow dy="2"/></filter>')).not.toHaveLength(0);
  });
});

describe("what counts as a gradient", () => {
  it("reports every kind of CSS gradient", () => {
    for (const value of [
      "linear-gradient(#fff, #000)",
      "radial-gradient(circle, #fff, #000)",
      "conic-gradient(from 0deg, #fff, #000)",
      "repeating-linear-gradient(45deg, #fff 0 2px, #000 2px 4px)",
    ]) {
      const found = findFlatness(`.hero { background: ${value}; }`);
      expect(found, value).toHaveLength(1);
      expect(found[0].construct, value).toBe("gradient()");
    }
  });

  it("reports an SVG gradient element", () => {
    expect(
      findFlatness('<linearGradient id="g"><stop offset="0"/></linearGradient>'),
    ).not.toHaveLength(0);
    expect(findFlatness('<radialGradient id="g"/>')).not.toHaveLength(0);
  });
});

describe("prose is not a declaration", () => {
  it("does not report a sentence that states the rule", () => {
    // Every one of these is real text from this repository. A checker that
    // failed on them would be a checker nobody keeps.
    const prose = [
      "All four are flat: no gradients, no shadows, no decorative radius.",
      "flat surfaces, no gradient, and no shadow that is decoration rather than depth",
      "No radius, no shadow, no gradient -- the built-in themes are flat",
      "Gradients and shadows are the first thing a projector turns to mud.",
      "the box-shadow property is not available to a theme",
    ];

    for (const line of prose) {
      expect(findFlatness(line), line).toHaveLength(0);
    }
  });

  it("does not report a word that merely contains one of the names", () => {
    expect(findFlatness("const shadowed = true;\nlet gradientless = 1;\n")).toHaveLength(0);
  });
});

describe("a finding is actionable", () => {
  it("names the line it is on", () => {
    const [finding] = findFlatness("a {}\nb {}\n.c { box-shadow: none; }\n");

    expect(finding.line).toBe(3);
  });

  it("quotes the text so the construct is recognisable in the report", () => {
    const [finding] = findFlatness(".c {\n  background: linear-gradient(#fff, #000);\n}\n");

    expect(finding.text).toContain("linear-gradient");
  });

  it("reports every construct in a file rather than stopping at the first", () => {
    const found = findFlatness(
      ".a { box-shadow: 0 0 1px red; }\n.b { background: conic-gradient(red, blue); }\n",
    );

    expect(found).toHaveLength(2);
  });
});

describe("a flat stylesheet passes", () => {
  it("reports nothing for the shell stylesheet's own declarations", () => {
    const flat = [
      ".slidx-slide { background: var(--slidx-color-surface); }",
      "pre { border-radius: var(--slidx-radius); }",
      "[data-slidx-mark] { transition: color 200ms ease-out; }",
      '<rect x="0" y="0" width="9" height="24" fill="#101014"/>',
    ].join("\n");

    expect(findFlatness(flat)).toHaveLength(0);
  });
});

describe("where the rule applies", () => {
  it("looks at files that actually exist", () => {
    // A glob that matched nothing would be a gate that passes because it stopped
    // asking. The shell stylesheet and the brand's mark are the two ends of what
    // this check has to cover.
    const files = shippedFiles();

    expect(files).toContain("crates/slidx_render/src/layout.rs");
    expect(files).toContain("crates/slidx_theme/src/builtin.rs");
    expect(files).toContain("packages/runtime/src/effects.css");
    expect(files).toContain("packages/editor/src/styles.ts");
    expect(files).toContain("assets/brand/mark-light.svg");
  });

  it("exempts only the checker and its own test", () => {
    // These two files have to contain what they reject. Naming them rather than
    // loosening the patterns keeps the next real shadow caught.
    expect(EXEMPT).toEqual(["scripts/flat.mjs", "scripts/test/flat.test.mjs"]);

    for (const exempt of EXEMPT) {
      expect(shippedFiles()).not.toContain(exempt);
    }
  });

  it("does not look at an author's own deck", () => {
    // slidx does not forbid a shadow in someone else's slide. The rule is about
    // what slidx itself ships, and confusing the two would make the framework
    // opinionated about content rather than about its own output.
    expect(shippedFiles().every((file) => !file.startsWith("examples/"))).toBe(true);
  });
});
