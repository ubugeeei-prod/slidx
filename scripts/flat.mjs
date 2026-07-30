/**
 * What "flat" means, and where the rule applies.
 *
 * slidx ships nothing with a shadow and nothing with a gradient. That is a
 * legibility decision before it is a taste one — both are the first thing a
 * projector turns to mud, which is why all four built-in themes are flat and why
 * the brand's corner radius is zero — and a legibility decision that can be
 * quietly broken is not a decision, it is a preference someone wrote down.
 *
 * So it is a gate. `scripts/check-flat.mjs` runs this over every shipped
 * stylesheet, theme, logo and generated image and exits non-zero on a single
 * finding. Unlike the size guideline, there is no warn-only mode: a shadow is
 * not a judgement call about how much design a file is holding.
 *
 * # Why the patterns need a colon or a parenthesis
 *
 * This repository is full of sentences that *say* shadows are forbidden — the
 * built-in themes' module docs, the editor stylesheet's header, the comment
 * inside the mark's own SVG. A checker that fired on its own documentation would
 * be switched off inside a week, so what is matched is a **declaration or a
 * call**: a property followed by `:` or `=`, a function followed by `(`, an
 * element followed by a word boundary. Prose is none of those, and the
 * distinction costs nothing to state.
 *
 * # What this cannot see
 *
 * A value assembled at runtime from pieces — `"linear-" + "gradient(...)"` —
 * would pass. Nothing in the repository does that and a checker built to survive
 * deliberate evasion would be a parser, so the bound is stated rather than
 * papered over. Rasters are covered at their source: every PNG under `docs/` and
 * `assets/` is generated from a file scanned here, and one that disagreed with
 * its source would already fail to reproduce.
 */

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

/**
 * The two files allowed to contain what the rule rejects.
 *
 * A checker and its test cannot be held to it. Naming them rather than
 * loosening a pattern is the same trade `og.rs` makes when it exempts `xmlns`
 * by name: the next real shadow is still caught.
 */
export const EXEMPT = ["scripts/flat.mjs", "scripts/test/flat.test.mjs"];

/** Where slidx's own output and assets come from. */
const SOURCE_ROOTS = ["crates", "packages", "scripts", "assets", "docs"];

/**
 * Text that can carry a declaration.
 *
 * Markdown is deliberately absent. Prose describing the rule is not a violation
 * of it, and a document is neither a stylesheet nor an image.
 *
 * `examples/` is absent for a different reason: slidx does not forbid a shadow
 * in somebody else's slide. This rule is about what the framework ships, and
 * confusing the two would make it opinionated about content.
 */
const SCANNABLE = /\.(rs|ts|tsx|mjs|js|css|svg|html|json)$/;

/**
 * Every construct the rule rejects, with the punctuation that makes it a
 * construct rather than a mention.
 */
const CONSTRUCTS = [
  { construct: "box-shadow", pattern: /\bbox-shadow\s*:/g },
  { construct: "box-shadow", pattern: /\bboxShadow\s*[:=]/g },
  { construct: "text-shadow", pattern: /\btext-shadow\s*:/g },
  { construct: "text-shadow", pattern: /\btextShadow\s*[:=]/g },
  { construct: "drop-shadow()", pattern: /\bdrop-shadow\s*\(/g },
  { construct: "feDropShadow", pattern: /<\s*feDropShadow\b/g },
  // Any gradient at all, however it is spelled: linear, radial, conic, and the
  // repeating forms all end in the same function name.
  { construct: "gradient()", pattern: /[\w-]*gradient\s*\(/g },
  { construct: "svgGradient", pattern: /<\s*(?:linear|radial)Gradient\b/g },
];

/**
 * Every shadow and gradient in one piece of text.
 *
 * @param {string} text
 * @returns {{construct: string, line: number, text: string}[]} in line order
 */
export function findFlatness(text) {
  const found = [];

  for (const { construct, pattern } of CONSTRUCTS) {
    for (const match of text.matchAll(pattern)) {
      const before = text.slice(0, match.index);
      found.push({
        construct,
        line: before.split("\n").length,
        // The whole line, trimmed. A report that quoted only the matched
        // fragment would name the property without the value that explains it.
        text: lineAt(text, match.index),
      });
    }
  }

  return found.sort((one, other) => one.line - other.line);
}

function lineAt(text, index) {
  const start = text.lastIndexOf("\n", index) + 1;
  const end = text.indexOf("\n", index);

  return text.slice(start, end === -1 ? undefined : end).trim();
}

/** Files git knows about, so build output and dependencies are excluded for free. */
export function shippedFiles() {
  const output = execFileSync("git", ["ls-files", "-z", ...SOURCE_ROOTS], { encoding: "utf8" });

  return output
    .split("\0")
    .filter((file) => SCANNABLE.test(file))
    .filter((file) => !EXEMPT.includes(file));
}

/** Every finding across every shipped file, each tagged with where it came from. */
export function scanRepository() {
  return shippedFiles().flatMap((file) =>
    findFlatness(readFileSync(file, "utf8")).map((finding) => ({ file, ...finding })),
  );
}
