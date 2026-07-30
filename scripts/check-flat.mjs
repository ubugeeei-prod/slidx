/**
 * No shadows, no gradients — in CI, not in a style guide.
 *
 * Everything slidx draws is flat: the four built-in themes, the shell
 * stylesheets, the presenter and print views, the editor chrome, the mark, and
 * every generated asset. The reason is the one this whole project is organised
 * around — a projector loses a soft edge and a colour ramp before it loses
 * anything else, and a speaker finds out in the room.
 *
 * A note in a document would have been the usual way to keep that. This is the
 * other way: `vp run workspace:ci` fails on a single shadow, so the rule cannot
 * be broken quietly, only deliberately and in review.
 *
 * `scripts/flat.mjs` decides what counts. See its header for why a mention is
 * not a declaration, and for the one thing this cannot see.
 */

import { scanRepository, shippedFiles } from "./flat.mjs";

const findings = scanRepository();

for (const { file, line, construct, text } of findings) {
  process.stderr.write(`error: ${file}:${line}: ${construct} — ${text}\n`);
}

if (findings.length > 0) {
  process.stderr.write(
    `\nslidx ships nothing with a shadow and nothing with a gradient: both are the\n` +
      `first thing a projector turns to mud. Use a hairline in --slidx-color-border,\n` +
      `or a flat fill. If the design genuinely needs one, change this rule in review\n` +
      `rather than around it.\n`,
  );
  process.exit(1);
}

process.stdout.write(
  `flat: ${shippedFiles().length} shipped files checked, no shadows or gradients\n`,
);
