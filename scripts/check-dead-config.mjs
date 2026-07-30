/**
 * Public fields that nothing reads.
 *
 * Written after `ShellOptions::include_runtime` was found declared, defaulted,
 * and documented as "emitted into the page so the runtime can resolve steps" —
 * and read nowhere. The step pipeline therefore never ran on the one screen an
 * audience looks at, while the PDF, the print shell, and the presenter view
 * all walked the stops correctly and every test passed.
 *
 * The compiler cannot see this. Rust's dead-code lint does not fire for a
 * `pub` field in a library, because from the crate's point of view a caller
 * might read it. In a workspace where the only callers are in the same
 * workspace, that assumption is exactly wrong.
 *
 * A warning rather than a failure. A field can be genuinely write-only — a
 * payload that crosses into JavaScript, a struct built only to be serialised —
 * so this is a prompt to look, in the same spirit as the size guideline. The
 * ones that have been looked at are recorded with their reason in
 * `write-only.mjs`, which is also what keeps this output short enough to read.
 */

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

import { implementation } from "./rust-source.mjs";
import { classify, keyOf, WRITE_ONLY } from "./write-only.mjs";

/** Where the workspace's own source lives. */
const SOURCE_ROOTS = ["crates", "packages", "examples", "scripts"];

const SOURCE_PATTERN = /\.(rs|ts|mjs|js)$/;

/**
 * Test code, which does not count as a caller.
 *
 * A field read only by a test is read by nobody the product depends on, and
 * that is not a hypothetical: `Slide::layout` was parsed, asserted on in two
 * tests, and rendered by nothing — a documented frontmatter field that changed
 * nothing about a deck. Counting its tests as readers is what let it sit there.
 */
const TEST_FILE = /(^|\/)tests?\/|[._](test|spec)\./;

function trackedFiles() {
  const output = execFileSync("git", ["ls-files", "-z", ...SOURCE_ROOTS], { encoding: "utf8" });
  return output.split("\0").filter((file) => SOURCE_PATTERN.test(file) && !TEST_FILE.test(file));
}

const files = trackedFiles();
const sources = new Map(
  files.map((file) => [
    file,
    file.endsWith(".rs")
      ? implementation(readFileSync(file, "utf8")).text
      : readFileSync(file, "utf8"),
  ]),
);

/** A field, and the file that declares it. */
const declarations = [];

for (const [file, source] of sources) {
  if (!file.endsWith(".rs")) continue;

  const lines = source.split("\n");
  let struct = null;

  for (const [index, line] of lines.entries()) {
    const opened = /^pub struct (\w+)/.exec(line);
    if (opened) {
      struct = opened[1];
      continue;
    }

    if (struct && line.startsWith("}")) {
      struct = null;
      continue;
    }

    const field = struct === null ? null : /^\s{4}pub (\w+):/.exec(line);
    if (field) declarations.push({ file, struct, field: field[1], line: index });
  }
}

/**
 * The same name as JavaScript sees it.
 *
 * A payload type carries `#[serde(rename_all = "camelCase")]`, so a field read
 * only across the WebAssembly boundary is spelled differently on the far side
 * and would otherwise look unread.
 */
function camelCase(name) {
  return name.replace(/_(\w)/g, (_, letter) => letter.toUpperCase());
}

/**
 * True when something reads the field.
 *
 * Three things are deliberately not reads, and each was a hole that let a real
 * dead field through:
 *
 * **Building the struct.** `Slide` is declared in `model.rs` and constructed in
 * `parser.rs`, so blanking assignments only in the declaring file exempted
 * every field of every model type — the ones that matter most. The key is
 * blanked wherever it appears; the value is kept, because `a: other.a` does
 * read one.
 *
 * **A method of the same name on some other type.** `\blayout\b` matched
 * `version.layout(ecc)` in `slidx_qr`, a method on an unrelated struct, so
 * `Slide::layout` looked read from a crate that has never seen a slide. A read
 * is `.field` *not* followed by a call.
 *
 * **A test.** Handled by leaving test code out of the haystack entirely.
 *
 * What survives is a field access and a mention from JavaScript, which is what
 * a read actually looks like. The cost is that a field only ever destructured
 * — `let Slide { layout, .. }` — reads as unread; that is a warning worth
 * looking at rather than a rule worth loosening, and this check only warns.
 */
function isRead({ file, field, line }) {
  const names = new Set([field, camelCase(field)]);

  for (const [candidate, source] of sources) {
    const lines = source.split("\n");

    if (candidate === file && line < lines.length) lines[line] = "";
    for (const [index, text] of lines.entries()) {
      for (const name of names) {
        lines[index] = lines[index].replace(new RegExp(`^(\\s*)${name}\\s*:`), "$1");
      }
    }

    const haystack = lines.join("\n");
    for (const name of names) {
      if (new RegExp(`\\.${name}\\b(?!\\s*\\()`).test(haystack)) return true;
    }
  }

  return false;
}

const unread = declarations.filter((declaration) => !isRead(declaration));
const { unexplained, stale } = classify(unread.map(keyOf), new Set(WRITE_ONLY.keys()));

for (const entry of unexplained) {
  process.stdout.write(
    `warning: ${entry} is declared and never read — either wire it up, delete it, ` +
      "or record in write-only.mjs why it is write-only\n",
  );
}

for (const entry of stale) {
  process.stdout.write(
    `warning: ${entry} is listed as write-only and is now read — drop the exemption\n`,
  );
}

process.stdout.write(
  `dead config: ${declarations.length} public fields checked, ${unexplained.length} unread, ` +
    `${WRITE_ONLY.size - stale.length} write-only by design\n`,
);
