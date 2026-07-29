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
 * so this is a prompt to look, in the same spirit as the size guideline.
 */

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

/** Where the workspace's own source lives. */
const SOURCE_ROOTS = ["crates", "packages", "examples", "scripts"];

const SOURCE_PATTERN = /\.(rs|ts|mjs|js)$/;

function trackedFiles() {
  const output = execFileSync("git", ["ls-files", "-z", ...SOURCE_ROOTS], { encoding: "utf8" });
  return output.split("\0").filter((file) => SOURCE_PATTERN.test(file));
}

const files = trackedFiles();
const sources = new Map(files.map((file) => [file, readFileSync(file, "utf8")]));

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
 * True when the name appears somewhere that is not its own declaration or the
 * construction of the struct it belongs to.
 *
 * Assignments are blanked only in the declaring file: building a struct is not
 * reading it, and a field whose only mention is the `Default` impl beside it is
 * the exact shape of the bug this exists to catch. A `.field` read, a
 * destructuring pattern, and any mention from another file all survive.
 */
function isRead({ file, field, line }) {
  const names = new Set([field, camelCase(field)]);

  for (const [candidate, source] of sources) {
    const lines = source.split("\n");

    if (candidate === file) {
      lines[line] = "";
      for (const [index, text] of lines.entries()) {
        if (new RegExp(`^\\s*${field}\\s*:`).test(text)) lines[index] = "";
      }
    }

    const haystack = lines.join("\n");
    for (const name of names) {
      if (new RegExp(`\\b${name}\\b`).test(haystack)) return true;
    }
  }

  return false;
}

const unread = declarations.filter((declaration) => !isRead(declaration));

for (const { file, struct, field } of unread) {
  process.stdout.write(
    `warning: ${file}: ${struct}.${field} is declared and never read — ` +
      "either wire it up or delete it\n",
  );
}

process.stdout.write(
  `dead config: ${declarations.length} public fields checked, ${unread.length} unread\n`,
);
