/**
 * Copies files into a package's dist directory.
 *
 * `cp` is not a command on Windows and `copyfiles` is a dependency for
 * something Node already does, so this is the smallest thing that keeps
 * `pack:lib` identical on every platform — which is the same reason CI runs
 * the Windows matrix at all.
 */

import { copyFileSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";

const pairs = process.argv.slice(2);
if (pairs.length === 0 || pairs.length % 2 !== 0) {
  process.stderr.write("usage: copy-assets.mjs <from> <to> [<from> <to>...]\n");
  process.exit(1);
}

for (let i = 0; i < pairs.length; i += 2) {
  const from = pairs[i];
  const to = pairs[i + 1];
  mkdirSync(dirname(to), { recursive: true });
  copyFileSync(from, to);
  process.stdout.write(`copied ${from} -> ${to}\n`);
}
