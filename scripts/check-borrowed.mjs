/**
 * No borrowed palettes, in CI rather than in a review comment.
 *
 * slidx shipped a framework's default `zinc` ramp, its `blue-700` and its
 * `violet-800` for months, and the reason nobody caught it is that a pasted
 * palette looks exactly like a chosen one once it is in a file. Taste is the
 * wrong instrument for that; a gate is the right one.
 *
 * `scripts/borrowed.mjs` decides what counts. See its header for the two rules
 * and for why the first one — a palette is mixed, not written — is the one that
 * actually closes the hole.
 */

import { scanRepository } from "./borrowed.mjs";
import { shippedFiles } from "./shipped.mjs";

const findings = scanRepository();

for (const finding of findings) {
  const { file, line, rule, value } = finding;

  if (rule === "borrowed") {
    process.stderr.write(`error: ${file}:${line}: ${value} is ${finding.source}\n`);
  } else {
    process.stderr.write(`error: ${file}:${line}: ${value} is written down rather than mixed\n`);
  }
}

if (findings.length > 0) {
  process.stderr.write(
    `\nA colour slidx ships is mixed from a hue, a chroma and a lightness, each with\n` +
      `a reason beside it — see slidx_theme::mix and slidx_theme::builtin::recipe.\n` +
      `A palette pasted from a framework carries no information about what slidx is,\n` +
      `which is what a reader notices even when they cannot name why.\n`,
  );
  process.exit(1);
}

process.stdout.write(
  `borrowed: ${shippedFiles().length} shipped files checked, every palette mixed\n`,
);
