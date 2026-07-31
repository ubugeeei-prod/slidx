/**
 * The sentences this project has already been caught overstating.
 *
 * `scripts/claims.mjs` holds them, each with the reason it is wrong and the
 * true sentence to write instead. This runs that over everything a reader
 * reads.
 *
 * It exists because the correction did not hold. The first entry on that list
 * was questioned, agreed to be wrong, and rewritten everywhere — and came back
 * in the next rewrite of the README, because it is the shorter sentence and the
 * one that sounds better. A rule kept in somebody's memory is a rule that lasts
 * until the next person edits the paragraph.
 *
 * This file does not quote the phrases for the same reason it would otherwise
 * have to exempt itself: the list is the one place they are written down.
 */

import { readFileSync } from "node:fs";

import { overstatements, readableFiles } from "./claims.mjs";

const files = readableFiles();
const findings = files.flatMap((file) =>
  overstatements(readFileSync(file, "utf8")).map((finding) => ({ file, ...finding })),
);

for (const { file, line, text, claim } of findings) {
  process.stderr.write(`error: ${file}:${line}: "${claim.phrase}" — ${claim.wrong}\n`);
  process.stderr.write(`  ${text}\n`);
  process.stderr.write(`  instead: ${claim.instead}\n`);
}

if (findings.length > 0) {
  process.stderr.write(
    `\nEach of these was written, believed, and found to be wider than the thing\n` +
      `that proves it. If the claim has become true, change the entry in\n` +
      `scripts/claims.mjs in review rather than around it.\n`,
  );
  process.exit(1);
}

process.stdout.write(`claims: ${files.length} readable files checked, none overstated\n`);
