/**
 * No borrowed palettes, and no palette written as hex literals.
 *
 * # Why this exists
 *
 * slidx shipped a framework's default colour scale for months and nobody
 * noticed. `#18181b`, `#09090b`, `#52525b`, `#e4e4e7` and `#f4f4f5` were one
 * popular CSS framework's `zinc` ramp in order; `#1d4ed8` was its `blue-700`;
 * `#5b21b6` was its `violet-800`, in three of the four built-in themes at once.
 *
 * The problem was never that those colours are ugly. It is that they are what
 * *every* machine reaches for, so they carry no information about what the
 * product is — and a reader can tell, even without being able to name why. A
 * colour a person chose is recognisable even when it is plainer.
 *
 * Taste cannot be relied on to catch this: the values look like decisions once
 * they are in a file, which is precisely how they survived review. So it is a
 * gate.
 *
 * # Two rules, and the first is the one that matters
 *
 * **A palette is mixed, not written.** The files that declare shipped palettes
 * may not contain a hex colour literal at all. Every colour in them comes from
 * `slidx_theme::mix` — a hue, a chroma and a lightness, each with a reason beside
 * it. This is the complete rule: a borrowed scale cannot be pasted into a file
 * that rejects pasted colours, whatever the scale happens to be.
 *
 * **A known borrowed colour is rejected everywhere.** The list below is a list of
 * mistakes actually found in this repository, not an attempt to enumerate every
 * framework in the world — that would be a losing game and a false sense of
 * cover. It earns its place because rule one cannot reach a stylesheet written in
 * CSS, where there is no mixer to call.
 */

import { readFileSync } from "node:fs";

import { shippedFiles } from "./shipped.mjs";

/**
 * Files whose whole job is to declare colours slidx ships.
 *
 * Listed rather than pattern-matched: this is the set somebody has to think about
 * when adding a palette, and a glob would quietly include or exclude one.
 */
export const PALETTE_SOURCES = [
  "crates/slidx_theme/src/builtin.rs",
  "crates/slidx_theme/src/builtin/recipe.rs",
  "crates/slidx_theme/src/palette.rs",
  "crates/slidx_brand/src/palette.rs",
];

/**
 * Colours copied from somewhere else, each one found in this repository.
 *
 * Kept as `{ value, source }` so a failure says where the colour came from. Being
 * told "#5b21b6 is not ours" is useless; being told it is a framework's
 * `violet-800` is the whole explanation.
 */
export const BORROWED = [
  { value: "#18181b", source: "a CSS framework's zinc-900" },
  { value: "#09090b", source: "a CSS framework's zinc-950" },
  { value: "#27272a", source: "a CSS framework's zinc-800" },
  { value: "#3f3f46", source: "a CSS framework's zinc-700" },
  { value: "#52525b", source: "a CSS framework's zinc-600" },
  { value: "#71717a", source: "a CSS framework's zinc-500" },
  { value: "#a1a1aa", source: "a CSS framework's zinc-400" },
  { value: "#d4d4d8", source: "a CSS framework's zinc-300" },
  { value: "#e4e4e7", source: "a CSS framework's zinc-200" },
  { value: "#f4f4f5", source: "a CSS framework's zinc-100" },
  { value: "#fafafa", source: "a CSS framework's zinc-50" },
  { value: "#1d4ed8", source: "a CSS framework's blue-700" },
  { value: "#1e40af", source: "a CSS framework's blue-800" },
  { value: "#1e3a8a", source: "a CSS framework's blue-900" },
  { value: "#bfdbfe", source: "a CSS framework's blue-200" },
  { value: "#5b21b6", source: "a CSS framework's violet-800" },
  { value: "#ddd6fe", source: "a CSS framework's violet-200" },
  { value: "#a7f3d0", source: "a CSS framework's emerald-200" },
  { value: "#86efac", source: "a CSS framework's green-300" },
  { value: "#166534", source: "a CSS framework's green-800" },
  { value: "#fed7aa", source: "a CSS framework's orange-200" },
  { value: "#fdba74", source: "a CSS framework's orange-300" },
  { value: "#9a3412", source: "a CSS framework's orange-800" },
  { value: "#7c2d12", source: "a CSS framework's orange-900" },
  { value: "#1c1917", source: "a CSS framework's stone-900" },
  { value: "#0c0a09", source: "a CSS framework's stone-950" },
  { value: "#292524", source: "a CSS framework's stone-800" },
  { value: "#44403c", source: "a CSS framework's stone-700" },
  { value: "#57534e", source: "a CSS framework's stone-600" },
  { value: "#a8a29e", source: "a CSS framework's stone-400" },
  { value: "#d6d3d1", source: "a CSS framework's stone-300" },
  { value: "#e7e5e4", source: "a CSS framework's stone-200" },
  { value: "#f5f5f4", source: "a CSS framework's stone-100" },
  { value: "#fafaf9", source: "a CSS framework's stone-50" },
  { value: "#7aa2f7", source: "an editor theme's blue" },
  { value: "#2f6feb", source: "a code host's brand blue" },
];

/** Any six-digit hex colour, as a declaration would write one. */
const HEX = /#[0-9a-fA-F]{6}\b/g;

const BY_VALUE = new Map(BORROWED.map((entry) => [entry.value.toLowerCase(), entry.source]));

/**
 * A file's lines, with comments and everything below `#[cfg(test)]` blanked.
 *
 * Two exclusions, each for a reason the rule would be worse without.
 *
 * **Tests**, on the same grounds `check-conventions.mjs` gives for not counting
 * their lines: a test module is where a claim gets proved, and proving these two
 * claims *requires* writing the rejected thing down. `the_audit_is_not_vacuous`
 * re-runs the borrowed blue to show the contrast audit still has teeth; the
 * palette tests build a fixture palette from literals. Holding those to the rule
 * would mean deleting the tests that make the rule worth having.
 *
 * **Comments**, because a mention is not a declaration — the same distinction
 * `flat.mjs` draws. `builtin/recipe.rs` names the exact borrowed hexes it
 * replaced, which is the most useful paragraph in the file, and a checker that
 * failed on its own explanation would be switched off inside a week.
 *
 * Blanked rather than sliced away, so a reported line number still matches the
 * file a person opens.
 */
function implementation(text) {
  const lines = withoutComments(text).split("\n");
  const testsBegin = lines.findIndex((line) => line.trimStart().startsWith("#[cfg(test)]"));

  return testsBegin === -1 ? lines : lines.map((line, index) => (index < testsBegin ? line : ""));
}

/**
 * Comment bodies replaced by spaces, so every line number and column survives.
 *
 * The three syntaxes this repository's shipped files use. A `//` inside a string
 * literal is blanked too — `https://` is the usual case — which costs nothing
 * here, since a colour is never on the far side of one.
 */
function withoutComments(text) {
  const blank = (match) => match.replace(/[^\n]/g, " ");

  return text
    .replace(/\/\*[\s\S]*?\*\//g, blank)
    .replace(/<!--[\s\S]*?-->/g, blank)
    .replace(/\/\/[^\n]*/g, blank);
}

/**
 * Every borrowed colour in one piece of text.
 *
 * @returns {{value: string, source: string, line: number}[]}
 */
export function findBorrowed(text) {
  return implementation(text).flatMap((line, index) =>
    [...line.matchAll(HEX)]
      .map((match) => ({
        value: match[0],
        source: BY_VALUE.get(match[0].toLowerCase()),
        line: index + 1,
      }))
      .filter((finding) => finding.source !== undefined),
  );
}

/**
 * Every hex literal in a file that is supposed to mix its colours.
 *
 * @returns {{value: string, line: number}[]}
 */
export function findWrittenColours(text) {
  return implementation(text).flatMap((line, index) =>
    [...line.matchAll(HEX)].map((match) => ({ value: match[0], line: index + 1 })),
  );
}

/** Every finding across every shipped file. */
export function scanRepository() {
  const findings = [];

  for (const file of shippedFiles()) {
    const text = readFileSync(file, "utf8");

    for (const finding of findBorrowed(text)) {
      findings.push({ file, ...finding, rule: "borrowed" });
    }

    if (PALETTE_SOURCES.includes(file)) {
      for (const finding of findWrittenColours(text)) {
        findings.push({ file, ...finding, rule: "written" });
      }
    }
  }

  return findings;
}
