/**
 * The borrowed-palette rule, checked on the checker.
 *
 * slidx shipped a framework's `zinc` ramp, its `blue-700` and its `violet-800`
 * for months without anyone noticing, because a pasted palette looks exactly like
 * a chosen one once it is in a file. This gate is the replacement for noticing.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

import {
  BORROWED,
  findBorrowed,
  findWrittenColours,
  PALETTE_SOURCES,
  scanRepository,
} from "../borrowed.mjs";
import { shippedFiles } from "../shipped.mjs";

const ROOT = join(import.meta.dirname, "../..");
const read = (path) => readFileSync(join(ROOT, path), "utf8");

describe("a borrowed colour is named, not just rejected", () => {
  it("reports the framework a colour came from", () => {
    const [finding] = findBorrowed("const accent = '#5b21b6';");

    expect(finding.value).toBe("#5b21b6");
    expect(finding.source).toContain("violet-800");
  });

  it("finds a borrowed colour whatever case it is written in", () => {
    expect(findBorrowed("#18181B")).toHaveLength(1);
  });

  it("says nothing about a colour that was mixed here", () => {
    // Every value the recipe produces. If one of these were ever on the list the
    // build would fail on its own output.
    for (const mixed of ["#01489f", "#a5c9ff", "#1755aa", "#6d3369", "#fafcff"]) {
      expect(findBorrowed(`--c: ${mixed};`), mixed).toHaveLength(0);
    }
  });

  it("does not fire on a fixture below #[cfg(test)]", () => {
    // Proving the contrast audit has teeth means re-running the borrowed blue
    // through it. A rule that deleted its own evidence would be a bad trade.
    const source = [
      'fn accent() -> &\'static str { "#01489f" }',
      "#[cfg(test)]",
      "mod tests {",
      '  let old = "#1d4ed8";',
      "}",
    ].join("\n");

    expect(findBorrowed(source)).toHaveLength(0);
  });

  it("still fires above #[cfg(test)]", () => {
    const source = ['const ACCENT: &str = "#1d4ed8";', "#[cfg(test)]", "mod tests {}"].join("\n");

    expect(findBorrowed(source)).toHaveLength(1);
    expect(findBorrowed(source)[0].line).toBe(1);
  });
});

describe("a palette is mixed, not written", () => {
  it("reports any hex literal in a file that declares a palette", () => {
    // The complete rule: a borrowed scale cannot be pasted into a file that
    // rejects pasted colours, whatever the scale happens to be.
    const found = findWrittenColours('let accent = Rgba::from("#123456");');

    expect(found).toHaveLength(1);
    expect(found[0].value).toBe("#123456");
  });

  it("says nothing about a mixed colour", () => {
    expect(findWrittenColours("Oklch::new(0.42, 0.154, 258.0).to_rgba()")).toHaveLength(0);
  });

  it("names files that exist and actually declare palettes", () => {
    // A path that stopped existing would make this rule silently cover nothing.
    for (const file of PALETTE_SOURCES) {
      expect(shippedFiles(), file).toContain(file);
    }
  });

  it("covers the file the borrowed scale was actually in", () => {
    expect(PALETTE_SOURCES).toContain("crates/slidx_theme/src/builtin.rs");
  });
});

describe("the repository is clean", () => {
  it("has no borrowed colour and no written palette left", () => {
    expect(scanRepository()).toEqual([]);
  });

  it("no longer ships the values that started this", () => {
    // The specific mistakes, asserted gone rather than assumed gone.
    const themes = read("crates/slidx_theme/src/builtin.rs");

    for (const value of ["#5b21b6", "#18181b", "#1d4ed8", "#f4f4f5", "#ddd6fe"]) {
      expect(themes, value).not.toContain(value);
    }
  });

  it("lists every borrowed colour with a source a reader can act on", () => {
    for (const { value, source } of BORROWED) {
      expect(value).toMatch(/^#[0-9a-f]{6}$/);
      expect(source.length, value).toBeGreaterThan(8);
    }
  });
});

describe("the editor chrome uses the brand's own signal", () => {
  it("takes its accent from the committed brand tokens", () => {
    // The editor is TypeScript and cannot call the mixer, so its accent is
    // copied. Copied and checked is a different thing from copied and hoped.
    const tokens = JSON.parse(read("assets/brand/tokens.json"));
    const styles = read("packages/editor/src/styles.ts");

    expect(styles).toContain(`--slidx-e-accent: ${tokens.color.light.signal};`);
    expect(styles).toContain(`--slidx-e-accent: ${tokens.color.dark.signal};`);
  });
});
