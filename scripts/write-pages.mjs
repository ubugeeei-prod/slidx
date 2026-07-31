/**
 * Composes a registry page for every publishable directory that has none.
 *
 * Only for the ones with none. A page already in the tree is somebody's writing
 * about their own package, and a generator that overwrote it would make the
 * general page the ceiling rather than the floor.
 */

import { existsSync, readFileSync, writeFileSync } from "node:fs";

import { needsCommitted, publishedCrates, publishedPackages } from "./licensed.mjs";
import { PAGE_FILE, registryPage } from "./registry-page.mjs";

/** The name and description a manifest already carries. */
function describes(directory) {
  if (directory.startsWith("packages/")) {
    const { name, description } = JSON.parse(readFileSync(`${directory}/package.json`, "utf8"));
    return { name, description };
  }

  const manifest = readFileSync(`${directory}/Cargo.toml`, "utf8");
  const name = /^name\s*=\s*"([^"]+)"/m.exec(manifest);
  const description = /^description\s*=\s*"((?:[^"\\]|\\.)*)"/m.exec(manifest);

  if (name === null || description === null) {
    throw new Error(`${directory}/Cargo.toml needs a name and a description to have a page`);
  }

  return { name: name[1], description: description[1].replaceAll('\\"', '"') };
}

const written = [];
const publishable = [...publishedCrates(), ...publishedPackages()];

for (const directory of needsCommitted(publishable, PAGE_FILE)) {
  const page = `${directory}/${PAGE_FILE}`;
  if (existsSync(page)) continue;

  writeFileSync(page, registryPage(describes(directory)));
  written.push(page);
}

process.stdout.write(
  written.length === 0
    ? "pages: every publishable directory already has one\n"
    : `pages: wrote ${written.length}\n${written.map((page) => `  ${page}\n`).join("")}`,
);
