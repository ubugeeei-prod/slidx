/**
 * The details that separate a tool from a mock-up.
 *
 * None of these are visible in a screenshot and every one of them is the
 * difference between an interface somebody uses all day and one that was drawn
 * once. They are asserted here because a stylesheet has no other way to keep a
 * promise: a focus ring that quietly disappeared, or a font stack that quietly
 * lost its CJK fallback, would look completely fine to whoever removed it.
 */

import { describe, expect, it } from "vite-plus/test";

import { STYLESHEET } from "../src/styles";

/** One rule's body, by selector. */
function rule(selector: string): string {
  const at = STYLESHEET.indexOf(`${selector} {`);
  expect(at, `no rule for ${selector}`).toBeGreaterThan(-1);

  return STYLESHEET.slice(at, STYLESHEET.indexOf("}", at));
}

/** The stylesheet with comments removed, so a note about a rule is not a rule. */
function declarations(): string {
  return STYLESHEET.replaceAll(/\/\*[\s\S]*?\*\//g, "");
}

describe("a Japanese deck is written in this, not only shown in it", () => {
  it("names a CJK family in both stacks", () => {
    // The deck themes have a test for exactly this and the chrome had none.
    // Without a CJK family a Japanese slide title renders as tofu in the outline
    // — the panel whose whole job is telling an author which slide they are on.
    for (const token of ["--slidx-e-font-sans", "--slidx-e-font-mono"]) {
      const stack = rule(":root").split(`${token}:`)[1]?.split(";")[0] ?? "";

      expect(stack, token).toMatch(/Hiragino|Noto Sans/);
    }
  });

  it("ends both stacks in a generic family", () => {
    const root = rule(":root");

    expect(root).toContain("sans-serif;");
    expect(root).toContain("monospace;");
  });

  it("sets a line height that suits full-width glyphs", () => {
    // CJK fills its em box where Latin lowercase fills about half of it, so 1.5
    // reads airy in English and cramped in Japanese.
    const body = rule("body");
    const height = Number(body.split("line-height:")[1]?.split(";")[0]);

    expect(height).toBeGreaterThanOrEqual(1.6);
  });
});

describe("the keyboard is a first-class way to drive this", () => {
  it("draws a focus ring thick enough to see against a hairline border", () => {
    const ring = rule("[tabindex]:focus-visible");

    expect(ring).toContain("outline: 2px solid var(--slidx-e-accent)");
    expect(ring).toContain("outline-offset: 2px");
  });

  it("uses :focus-visible so a mouse click leaves no ring behind", () => {
    expect(declarations()).not.toMatch(/[^-]:focus\b(?!-visible)/);
  });

  it("reveals the row's remove button on focus and not only on hover", () => {
    // It used to be revealed by hover alone, which means tabbing to it moved
    // focus to something invisible and drew the ring on a control nobody could
    // see. That is not a small bug: it is a control a keyboard user cannot use.
    expect(declarations()).toContain(".slidx-outline-remove:focus-visible");
  });

  it("lifts a focused control above a neighbouring row's background", () => {
    expect(rule("[tabindex]:focus-visible")).toContain("z-index: 1");
  });
});

describe("a target is bigger than its ink", () => {
  it("gives every control a minimum height", () => {
    for (const selector of ["button", "input, textarea", ".slidx-outline-row"]) {
      expect(rule(selector), selector).toContain("min-height: var(--slidx-e-hit)");
    }
  });

  it("gives the remove button a square target rather than a sliver", () => {
    const remove = rule(".slidx-outline-remove");

    expect(remove).toContain("min-width: var(--slidx-e-hit)");
    expect(remove).toContain("min-height: var(--slidx-e-hit)");
  });
});

describe("one rhythm rather than ad-hoc pixels", () => {
  it("spends every distance from the four stated stops", () => {
    // The tokens themselves, the type sizes, the panel head, the hairline ring
    // and the two fixed panel widths are the exceptions, and each is a decision
    // about something other than spacing.
    const allowed = new Set([
      "0px",
      "1px",
      "2px",
      "4px",
      "6px",
      "8px",
      "11px",
      "12px",
      "13px",
      "16px",
      "24px",
      "28px",
      "34px",
      "80px",
      "232px",
      "296px",
    ]);

    for (const [value] of declarations().matchAll(/\b\d+px\b/g)) {
      expect(allowed.has(value), `${value} is off the rhythm`).toBe(true);
    }
  });

  it("declares the stops once", () => {
    for (const token of ["--slidx-e-tight", "--slidx-e-snug", "--slidx-e-gap", "--slidx-e-loose"]) {
      expect(rule(":root"), token).toContain(token);
    }
  });
});

describe("states are designed rather than left to the browser", () => {
  it("keeps the diagnostics panel present when there is nothing wrong", () => {
    // It used to be `display: none`, which moves every other panel by its height
    // the moment an author fixes the last diagnostic. The layout jumping is how
    // they find out it worked.
    expect(declarations()).not.toContain(
      '.slidx-diagnostics[data-empty="true"] { display: none; }',
    );
    expect(declarations()).toContain('.slidx-diagnostics[data-empty="true"]::after');
  });

  it("chooses the selection colour rather than inheriting the operating system's", () => {
    expect(rule("::selection")).toContain("--slidx-e-accent");
  });

  it("chooses the scrollbar colour", () => {
    expect(declarations()).toContain("scrollbar-color:");
  });

  it("sets placeholder text apart from a value", () => {
    expect(rule("::placeholder")).toContain("var(--slidx-e-muted)");
  });

  it("says which slide is current with more than a background", () => {
    // Hover and selection used to be the same fill, so moving the mouse through
    // the outline hid which slide the author was actually on.
    const current = rule('.slidx-outline-row[aria-current="true"]');

    expect(current).toContain("border-left: 2px solid var(--slidx-e-accent)");
  });
});

describe("motion is purposeful or absent", () => {
  it("only animates behind a reduced-motion query", () => {
    for (const [, body = ""] of declarations().matchAll(/transition:([^;]+);/g)) {
      const at = declarations().indexOf(`transition:${body}`);
      const query = declarations().lastIndexOf("@media (prefers-reduced-motion", at);

      expect(query, `an unguarded transition: ${body.trim()}`).toBeGreaterThan(-1);
    }
  });

  it("never animates the focus ring", () => {
    // It is the answer to "where am I", and an answer that fades in arrives
    // after the question.
    for (const [, body = ""] of declarations().matchAll(/transition:([^;]+);/g)) {
      expect(body).not.toContain("outline");
    }
  });
});

describe("the chrome stays flat", () => {
  it("declares no shadow and no gradient", () => {
    // `scripts/check-flat.mjs` is the repository-wide gate; this is the same
    // claim at the point the stylesheet is written, and it caught a defensive
    // `box-shadow: none` that had no reason to be here.
    for (const construct of ["box-shadow", "text-shadow", "gradient(", "drop-shadow("]) {
      expect(declarations(), construct).not.toContain(construct);
    }
  });
});
