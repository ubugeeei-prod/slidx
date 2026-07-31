/**
 * Nothing is published to a blank page.
 *
 * crates.io and npm render the README from inside the tarball and nothing else,
 * so a workspace with one README at its root publishes as many blank pages as
 * it has packages. A dry run across this one found 26 of them.
 *
 * What this checks is deliberately narrow: that a page exists, and that its
 * first heading names the thing it is a page for. The body is left alone —
 * `@slidxjs/theme-workshop` explains a theme, `@slidxjs/wasm` shows two lines of
 * JavaScript, and a check that owned their text would have to overwrite both.
 * The heading is the part that is wrong in a way nobody notices: a page copied
 * from the package next door reads perfectly and is about something else.
 *
 * `node scripts/write-pages.mjs` composes one for a directory that has none.
 */

import { readFileSync } from "node:fs";

import { needsCommitted, publishedCrates, publishedPackages } from "./licensed.mjs";
import { firstHeading, PAGE_FILE } from "./registry-page.mjs";

/** What a directory publishes under, which is what its page has to be titled. */
function publishedName(directory) {
  if (directory.startsWith("packages/")) {
    return JSON.parse(readFileSync(`${directory}/package.json`, "utf8")).name;
  }

  const manifest = readFileSync(`${directory}/Cargo.toml`, "utf8");
  const found = /^name\s*=\s*"([^"]+)"/m.exec(manifest);

  if (found === null) throw new Error(`no name in ${directory}/Cargo.toml`);
  return found[1];
}

// A page its build writes is skipped for the reason its licence is:
// `packages/wasm` is emptied and refilled, so the tree is not where the answer
// is. `.gitignore` already says which paths those are.
const publishable = [...publishedCrates(), ...publishedPackages()];
const directories = needsCommitted(publishable, PAGE_FILE);

const findings = [];

for (const directory of directories) {
  const name = publishedName(directory);
  let page;

  try {
    page = readFileSync(`${directory}/${PAGE_FILE}`, "utf8");
  } catch {
    findings.push(`${directory} is published as ${name} and has no ${PAGE_FILE}`);
    continue;
  }

  const heading = firstHeading(page);

  if (heading !== name) {
    findings.push(
      `${directory}/${PAGE_FILE} is titled ${heading === undefined ? "nothing" : `"${heading}"`}, ` +
        `but the package is published as ${name}`,
    );
  }
}

for (const failure of findings) {
  process.stderr.write(`error: ${failure}\n`);
}

if (findings.length > 0) {
  process.stderr.write(
    `\nA registry shows the README from inside the tarball and nothing else, so a\n` +
      `package without one is published to a blank page. Run:\n\n` +
      `  node scripts/write-pages.mjs\n\n` +
      `which composes one from the description the manifest already carries.\n`,
  );
  process.exit(1);
}

const generated = publishable.length - directories.length;

process.stdout.write(
  `pages: ${directories.length} publishable directories checked, every one has a page of its own` +
    (generated > 0 ? `, ${generated} more written by its build\n` : `\n`),
);
