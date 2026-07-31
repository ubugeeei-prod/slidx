/**
 * Version consistency across the workspace.
 *
 * A release is one version number written in several places: the Cargo
 * workspace, the version each crate is required at by its siblings, every
 * publishable package.json, and the git tag. They drift silently — a bumped
 * Cargo.toml and a forgotten package.json publishes two different things under
 * one tag, and the mistake is only visible after it is permanent on a
 * registry.
 *
 * Run with no argument to check the tree agrees with itself. Run with a tag
 * (`v0.0.1`) to also check the tag agrees with the tree, which is what the
 * release workflow does before it publishes anything.
 */

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

const [tag] = process.argv.slice(2);

const cargoVersion = readCargoWorkspaceVersion();
const internalRequirements = readInternalCrateRequirements();
const packages = readPublishablePackages();
const npmRequirements = readInternalPackageRequirements(packages);
const problems = [];

// `[workspace.package] version` is inherited, but the version each crate is
// *required* at by its siblings is written out once per crate, because cargo
// will not publish a path dependency without one. So a bump of the workspace
// alone leaves twelve requirements behind, and the resolver then finds a
// `slidx_core` on disk that no longer satisfies what `slidx_cli` asked for.
for (const { name, requirement } of internalRequirements) {
  if (requirement !== cargoVersion) {
    problems.push(
      `Cargo.toml: [workspace.dependencies] requires ${name} at ${requirement}, ` +
        `but the Cargo workspace is ${cargoVersion}`,
    );
  }
}

// And the lockfile records a version per workspace member too. Nothing in `vp
// check` passes `--locked`, so a bump that forgets `cargo update --workspace`
// is invisible here and surfaces in the release's binary builds, which do.
for (const { name, version } of readLockedWorkspaceMembers(internalRequirements)) {
  if (version !== cargoVersion) {
    problems.push(
      `Cargo.lock: ${name} is locked at ${version}, but the Cargo workspace is ${cargoVersion}` +
        " (run `cargo update --workspace`)",
    );
  }
}

for (const { path, name, version } of packages) {
  const directory = path.split("/").at(-2);
  const expectedName = directory === "cli" ? "slidx" : `@slidxjs/${directory}`;

  if (name !== expectedName) {
    problems.push(`${path}: package name must be ${expectedName}, but found ${name}`);
  }

  if (version !== cargoVersion) {
    problems.push(`${path}: ${name} is ${version}, but the Cargo workspace is ${cargoVersion}`);
  }
}

for (const { path, name, requirement } of npmRequirements) {
  if (!requirement.startsWith("workspace:") && requirement !== cargoVersion) {
    problems.push(
      `${path}: requires ${name} at ${requirement}, but the package release is ${cargoVersion}`,
    );
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
  `version ${cargoVersion} is consistent across Cargo, ` +
    `${internalRequirements.length} internal crate requirement(s), ` +
    `${packages.length} package(s) and ${npmRequirements.length} internal package requirement(s)` +
    (tag ? ` and tag ${tag}` : "") +
    "\n",
);

/**
 * What `[workspace.dependencies]` requires each crate in this workspace at.
 *
 * Only the entries that carry a `path` into `crates/`, so a third-party
 * dependency that happens to be named after one of ours is not held to our
 * version.
 */
function readInternalCrateRequirements() {
  const manifest = readFileSync("Cargo.toml", "utf8");
  const start = manifest.indexOf("[workspace.dependencies]");

  if (start === -1) throw new Error("no [workspace.dependencies] in Cargo.toml");

  const rest = manifest.slice(start + "[workspace.dependencies]".length);
  const next = rest.search(/^\[/m);
  const section = next === -1 ? rest : rest.slice(0, next);

  return [...section.matchAll(/^(\S+)\s*=\s*\{([^}]*)\}/gm)]
    .map(([, name, body]) => ({
      name,
      requirement: /version\s*=\s*"([^"]+)"/.exec(body)?.[1],
      path: /path\s*=\s*"([^"]+)"/.exec(body)?.[1],
    }))
    .filter(({ path }) => path !== undefined)
    .map(({ name, requirement }) => ({ name, requirement }));
}

/** What `Cargo.lock` has each crate of this workspace pinned at. */
function readLockedWorkspaceMembers(requirements) {
  const lock = readFileSync("Cargo.lock", "utf8");
  const ours = new Set(requirements.map(({ name }) => name));

  return [...lock.matchAll(/^name = "([^"]+)"\nversion = "([^"]+)"/gm)]
    .map(([, name, version]) => ({ name, version }))
    .filter(({ name }) => ours.has(name));
}

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
    .map(({ path, manifest }) => ({
      path,
      name: manifest.name,
      version: manifest.version,
      manifest,
    }));
}

/** Exact or workspace-relative requirements on packages this project owns. */
function readInternalPackageRequirements(packages) {
  const fields = ["dependencies", "optionalDependencies", "peerDependencies"];

  return packages.flatMap(({ path, manifest }) =>
    fields.flatMap((field) =>
      Object.entries(manifest[field] ?? {})
        .filter(([name]) => name === "slidx" || name.startsWith("@slidxjs/"))
        .map(([name, requirement]) => ({ path, name, requirement })),
    ),
  );
}
