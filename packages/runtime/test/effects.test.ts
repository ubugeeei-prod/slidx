/**
 * The stylesheet against the preset list.
 *
 * A preset the compiler can emit but the stylesheet has no rule for fails
 * silently: the element still appears at the right stop, so nothing errors,
 * but the animation the author asked for never plays. Nobody notices until
 * they are on stage. This is the cheapest place to catch it.
 *
 * The list exists twice — as a Rust enum, and as CSS rules — and this file is
 * the seam. The names are read out of the generated declarations rather than
 * restated here, because a third hand-written copy would be one more thing
 * that can be right about the wrong version.
 * `crates/slidx_core/src/steps/preset.rs` owns the source of truth.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

import { loadEffects } from "../src/effects";

// `import.meta.dirname` rather than `new URL(...)`: the DOM environment
// replaces the global URL, and its instances are not accepted by node:url.
const css = readFileSync(join(import.meta.dirname, "../src/effects.css"), "utf8");

const declarations = readFileSync(
  join(import.meta.dirname, "../../../crates/slidx_wasm/deck.d.ts"),
  "utf8",
);

/** The members of one generated union, in the order Rust declares them. */
function union(name: string): string[] {
  const declaration = new RegExp(`export type ${name} =([^;]+);`).exec(declarations);
  expect(declaration, `no generated declaration for ${name}`).not.toBeNull();

  return Array.from(declaration![1]!.matchAll(/"([^"]+)"/g), (match) => match[1]!);
}

const EFFECT_PRESETS = union("EffectPreset");

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
    for (const easing of union("Easing")) {
      expect(css).toContain(`--slidx-easing-${easing}:`);
    }
  });
});

describe("loading", () => {
  it("adds one stylesheet and resolves only after the browser applied it", async () => {
    const page = document.implementation.createHTMLDocument();
    const loaded = loadEffects(page, "data:text/css,[data-slidx-hidden]{visibility:hidden}");
    const link = page.querySelector<HTMLLinkElement>("[data-slidx-effects]");

    expect(link?.rel).toBe("stylesheet");
    expect(link?.getAttribute("href")).toContain("data:text/css");

    link?.dispatchEvent(new Event("load"));
    await expect(loaded).resolves.toBe(true);
  });

  it("shares an in-flight load rather than requesting the stylesheet twice", () => {
    const page = document.implementation.createHTMLDocument();
    const href = "data:text/css,[data-slidx-hidden]{visibility:hidden}";
    const first = loadEffects(page, href);
    const second = loadEffects(page, href);

    expect(second).toBe(first);
    expect(page.querySelectorAll("[data-slidx-effects]")).toHaveLength(1);
  });

  it("keeps the slide visible and permits a retry when loading fails", async () => {
    const page = document.implementation.createHTMLDocument();
    const failed = loadEffects(page, "data:text/css,");
    const link = page.querySelector<HTMLLinkElement>("[data-slidx-effects]");

    link?.dispatchEvent(new Event("error"));

    await expect(failed).resolves.toBe(false);
    expect(page.querySelector("[data-slidx-effects]")).toBeNull();
    expect(loadEffects(page, "data:text/css,body{}")).not.toBe(failed);
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
