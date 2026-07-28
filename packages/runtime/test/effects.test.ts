/**
 * The stylesheet against the preset list.
 *
 * A preset the compiler can emit but the stylesheet has no rule for fails
 * silently: the element still appears at the right stop, so nothing errors,
 * but the animation the author asked for never plays. Nobody notices until
 * they are on stage. This is the cheapest place to catch it.
 *
 * The same list exists three times — as a Rust enum, as a TypeScript union,
 * and as CSS rules — so the seams between them are checked rather than
 * trusted. `crates/slidx_core/src/steps/preset.rs` owns the source of truth.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { EFFECT_PRESETS } from "../src/types";

// `import.meta.dirname` rather than `new URL(...)`: the DOM environment
// replaces the global URL, and its instances are not accepted by node:url.
const css = readFileSync(join(import.meta.dirname, "../src/effects.css"), "utf8");

/** Presets that deliberately animate more than transform and opacity. */
const PAINT_HEAVY = new Set(["typewriter", "draw", "color-pulse", "underline"]);

describe("preset coverage", () => {
  it.each(EFFECT_PRESETS)("`%s` has a rule", (preset) => {
    expect(css).toContain(`[data-slidx-effect="${preset}"]`);
  });

  it.each(EFFECT_PRESETS.filter((preset) => preset !== "none"))("`%s` has keyframes", (preset) => {
    expect(css).toContain(`@keyframes slidx-${preset} {`);
  });

  it("declares no keyframes nothing references", () => {
    const declared = Array.from(css.matchAll(/@keyframes slidx-([\w-]+)/g), (match) => match[1]!);
    const referenced = new Set(
      Array.from(css.matchAll(/--slidx-effect-name: slidx-([\w-]+)/g), (match) => match[1]!),
    );

    expect(declared.filter((name) => !referenced.has(name))).toEqual([]);
  });
});

describe("compositor safety", () => {
  it.each(EFFECT_PRESETS.filter((preset) => preset !== "none" && !PAINT_HEAVY.has(preset)))(
    "`%s` animates only transform and opacity",
    (preset) => {
      const block = keyframeBlock(preset);
      const properties = Array.from(block.matchAll(/^\s*([a-z-]+):/gm), (match) => match[1]!);

      for (const property of properties) {
        expect(
          ["transform", "opacity", "clip-path"],
          `${preset} animates ${property}, which cannot stay on the compositor`,
        ).toContain(property);
      }
    },
  );

  it("marks every paint-heavy preset as such in Rust's terms", () => {
    // If this list and `EffectPreset::is_compositor_only` disagree, the linter
    // is warning about the wrong presets.
    for (const preset of PAINT_HEAVY) {
      expect(EFFECT_PRESETS).toContain(preset);
    }
  });
});

describe("degradation", () => {
  it("shows staged content when the runtime never loads", () => {
    // A venue with no network, or a blocked bundle, must not leave a slide
    // mostly invisible.
    expect(css).toContain("html:not([data-slidx-js]) [data-slidx-hidden]");
    expect(css).toMatch(
      /html:not\(\[data-slidx-js\]\) \[data-slidx-hidden\] \{\s*visibility: visible/,
    );
  });

  it("reserves layout for hidden elements rather than collapsing them", () => {
    // `display: none` would reflow the slide on every reveal, which is the
    // most common way staged bullet lists go wrong.
    expect(css).toMatch(/\[data-slidx-hidden\] \{\s*visibility: hidden/);
    expect(css).not.toMatch(/\[data-slidx-hidden\] \{\s*display: none/);
  });

  it("honours a reduced-motion preference", () => {
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
  });

  it("defines every easing the model can name", () => {
    for (const easing of ["linear", "ease", "ease-in", "ease-out", "ease-in-out", "spring"]) {
      expect(css).toContain(`--slidx-easing-${easing}:`);
    }
  });
});

function keyframeBlock(preset: string): string {
  const start = css.indexOf(`@keyframes slidx-${preset} {`);
  expect(start, `no keyframes for ${preset}`).toBeGreaterThan(-1);

  let depth = 0;
  for (let i = css.indexOf("{", start); i < css.length; i += 1) {
    if (css[i] === "{") depth += 1;
    if (css[i] === "}") {
      depth -= 1;
      if (depth === 0) return css.slice(start, i + 1);
    }
  }

  throw new Error(`unterminated keyframes for ${preset}`);
}
