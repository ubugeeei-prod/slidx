/**
 * Version consistency across the workspace.
 *
 * A release is one version number written in several places: the Cargo
 * workspace, every publishable package.json, and the git tag. They drift
 * silently — a bumped Cargo.toml and a forgotten package.json publishes two
 * different things under one tag, and the mistake is only visible after it is
 * permanent on a registry.
 *
 * Run with no argument to check the tree agrees with itself. Run with a tag
 * (`v0.0.1`) to also check the tag agrees with the tree, which is what the
 * release workflow does before it publishes anything.
 */

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

const [tag] = process.argv.slice(2);

const cargoVersion = readCargoWorkspaceVersion();
const packages = readPublishablePackages();
const problems = [];

for (const { path, name, version } of packages) {
  if (version !== cargoVersion) {
    problems.push(`${path}: ${name} is ${version}, but the Cargo workspace is ${cargoVersion}`);
  }
}

if (tag !== undefined) {
  const expected = tag.replace(/^v/, "");
  if (expected !== cargoVersion) {
    problems.push(`tag ${tag} does not match the workspace version ${cargoVersion}`);
  }
}

for (const problem of problems) {
  process.stderr.write(`error: ${problem}\n`);
}

if (problems.length > 0) process.exit(1);

process.stdout.write(
  `version ${cargoVersion} is consistent across Cargo and ${packages.length} package(s)` +
    (tag ? ` and tag ${tag}` : "") +
    "\n",
);

function readCargoWorkspaceVersion() {
  const manifest = readFileSync("Cargo.toml", "utf8");
  const section = manifest.slice(manifest.indexOf("[workspace.package]"));
  const match = /^version\s*=\s*"([^"]+)"/m.exec(section);

  if (!match) throw new Error("no version in [workspace.package]");
  return match[1];
}

/** Every package.json git tracks that is not marked private. */
function readPublishablePackages() {
  const listed = execFileSync("git", ["ls-files", "-z", "packages"], { encoding: "utf8" })
    .split("\0")
    .filter((path) => path.endsWith("package.json"));

  return listed
    .map((path) => ({ path, manifest: JSON.parse(readFileSync(path, "utf8")) }))
    .filter(({ manifest }) => manifest.private !== true)
    .map(({ path, manifest }) => ({ path, name: manifest.name, version: manifest.version }));
}
