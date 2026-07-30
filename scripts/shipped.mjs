/**
 * Which files count as something slidx ships.
 *
 * One list, because two design gates read it — `check-flat.mjs` and
 * `check-borrowed.mjs` — and a file that was in one list and not the other would
 * be a file held to half the rules for no stated reason.
 */

import { execFileSync } from "node:child_process";

/** Where slidx's own output and assets come from. */
const SOURCE_ROOTS = ["crates", "packages", "scripts", "assets", "docs"];

/**
 * Text that can carry a declaration.
 *
 * Markdown is deliberately absent. Prose describing a rule is not a violation of
 * it, and a document is neither a stylesheet nor an image.
 *
 * `examples/` is absent for a different reason: slidx does not forbid a shadow —
 * or a borrowed colour — in somebody else's slide. These rules are about what the
 * framework ships, and confusing the two would make it opinionated about content.
 */
const SCANNABLE = /\.(rs|ts|tsx|mjs|js|css|svg|html|json)$/;

/**
 * The files allowed to contain what the rules reject.
 *
 * A checker and its test cannot be held to the rule they implement. Naming them
 * rather than loosening a pattern is the same trade `og.rs` makes when it exempts
 * `xmlns` by name: the next real violation is still caught.
 */
export const EXEMPT = [
  "scripts/flat.mjs",
  "scripts/test/flat.test.mjs",
  "scripts/borrowed.mjs",
  "scripts/test/borrowed.test.mjs",
];

/** Files git knows about, so build output and dependencies are excluded for free. */
export function shippedFiles() {
  const output = execFileSync("git", ["ls-files", "-z", ...SOURCE_ROOTS], { encoding: "utf8" });

  return output
    .split("\0")
    .filter((file) => SCANNABLE.test(file))
    .filter((file) => !EXEMPT.includes(file));
}
