/**
 * The licence travels with every copy, checked rather than remembered.
 *
 * `scripts/licensed.mjs` decides which directories become a tarball and why the
 * notice has to be in each of them. This runs that over the tree and fails when
 * one is missing or has drifted from the notice at the root.
 *
 * Deliberately a check rather than a generator. A step that copied the file at
 * release time would keep the tree tidy and would also mean nobody ever sees
 * the licence in the repository they are reading — and a release step that
 * writes files is a release step that can be skipped by the one hand-run
 * publish that matters most, which is the first one.
 */

import { readFileSync } from "node:fs";

import { LICENCE_FILE, publishedCrates, publishedPackages, unlicensed } from "./licensed.mjs";

const licence = readFileSync(LICENCE_FILE, "utf8");
const directories = [...publishedCrates(), ...publishedPackages()];
const findings = unlicensed(directories, licence);

for (const { directory, problem } of findings) {
  process.stderr.write(
    problem === "missing"
      ? `error: ${directory} is published and has no ${LICENCE_FILE}\n`
      : `error: ${directory}/${LICENCE_FILE} is not the notice at the root of this repository\n`,
  );
}

if (findings.length > 0) {
  process.stderr.write(
    `\nNeither cargo nor npm looks outside a package directory for a licence, so a\n` +
      `copy has to sit beside each manifest. Run:\n\n` +
      findings.map(({ directory }) => `  cp ${LICENCE_FILE} ${directory}/\n`).join("") +
      `\nMIT asks for the notice to be included in the copies, and "license": "MIT"\n` +
      `is metadata about that rather than the notice itself.\n`,
  );
  process.exit(1);
}

process.stdout.write(
  `licensed: ${directories.length} publishable directories checked, every one carries the notice\n`,
);
