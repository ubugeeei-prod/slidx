/**
 * Layout conventions, checked locally and in CI.
 *
 * These are the two rules that are cheap to state, expensive to undo, and
 * invisible to a compiler. Keeping them here rather than in a CI workflow means
 * `vp check` catches them before a push, which is the only time a layout rule
 * is cheap to act on.
 *
 * See CONTRIBUTING.md for the reasoning.
 */

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

/**
 * Past this, a module is usually holding two ideas.
 *
 * A guideline, not a rule: it warns and never fails. The number is a prompt to
 * look, not a budget to spend — a 300-line file doing two things should still
 * be split, and a 420-line file doing one thing well should be left alone.
 */
const SOFT_LINE_LIMIT = 400;

/** Directories that hold source we wrote. */
const SOURCE_ROOTS = ["crates", "packages", "scripts"];

const failures = [];
const warnings = [];

/** Files git knows about, so build output and node_modules are excluded for free. */
function trackedFiles() {
  const output = execFileSync("git", ["ls-files", "-z", ...SOURCE_ROOTS], { encoding: "utf8" });
  return output.split("\0").filter(Boolean);
}

const files = trackedFiles();

// A tree full of files all named mod.rs is unnavigable in an editor's fuzzy
// file switcher: every result looks identical. Use the 2018 path style.
for (const file of files.filter((file) => file.endsWith("/mod.rs"))) {
  failures.push(
    `${file}: mod.rs is not allowed — use ${file.replace(/\/mod\.rs$/, ".rs")} instead`,
  );
}

/**
 * Lines of implementation, excluding tests.
 *
 * Tests are allowed to be long: a test module is a list, not an abstraction,
 * and splitting one to hit a line count makes it harder to read rather than
 * easier. The guideline is about how much *design* one file is holding, so
 * only the part above `#[cfg(test)]` counts.
 */
function implementationLines(file) {
  const lines = readFileSync(file, "utf8").split("\n");
  const testModule = lines.findIndex((line) => line.trimStart().startsWith("#[cfg(test)]"));

  return testModule === -1 ? lines.length : testModule;
}

for (const file of files.filter((file) => /\.(rs|ts|mjs)$/.test(file))) {
  if (/(^|\/)tests?\//.test(file)) continue;
  // A `.d.ts` describes a boundary rather than implementing one, and the ones
  // in this repository are generated from Rust. There is no design in them to
  // split, so a warning about their length is one nobody could ever act on.
  if (file.endsWith(".d.ts")) continue;

  const lines = implementationLines(file);
  if (lines > SOFT_LINE_LIMIT) {
    warnings.push(
      `${file}: ${lines} lines of implementation, over the ${SOFT_LINE_LIMIT} guideline`,
    );
  }
}

for (const warning of warnings) {
  process.stdout.write(`warning: ${warning}\n`);
}

for (const failure of failures) {
  process.stderr.write(`error: ${failure}\n`);
}

if (failures.length > 0) {
  process.exit(1);
}

process.stdout.write(
  `conventions: ${files.length} tracked files checked, ${warnings.length} over the size guideline\n`,
);
