/**
 * Cut a release: write the version everywhere, tag it, push the tag.
 *
 * `check-version.mjs` already knows every place a version lives — the Cargo
 * workspace, the version each crate is required at by its siblings, the
 * lockfile, and every publishable package.json. It exists because those drift
 * silently, and a bumped `Cargo.toml` with a forgotten `package.json` publishes
 * two different things under one tag, visible only once it is permanent.
 *
 * So this does not re-derive that list: it writes the same places and then runs
 * the check, which is the same authority the release workflow runs before it
 * publishes anything. If the two ever disagree, the check wins and this refuses.
 *
 * Pushing the tag is the last thing it does, and it is what starts a publish
 * that cannot be taken back. Everything that could fail is made to fail before
 * that point — a dirty tree, the wrong branch, a tag that already exists, a
 * version the check rejects.
 */

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const LEVELS = ["major", "minor", "patch"];

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const level = args.find((argument) => !argument.startsWith("-"));

if (!LEVELS.includes(level)) {
  process.stderr.write(
    `usage: vp run release <${LEVELS.join("|")}> [--dry-run]\n\n` +
      "Writes the version across the Cargo workspace and every publishable\n" +
      "package, commits it, and pushes the tag that starts the release.\n",
  );
  process.exit(2);
}

function git(...argv) {
  return execFileSync("git", argv, { encoding: "utf8" }).trim();
}

function run(command, argv) {
  execFileSync(command, argv, { stdio: "inherit" });
}

/** Reads `[workspace.package] version`, the one number everything else follows. */
function currentVersion() {
  const manifest = readFileSync("Cargo.toml", "utf8");
  const section = manifest.slice(manifest.indexOf("[workspace.package]"));
  const match = /^version\s*=\s*"([^"]+)"/m.exec(section);

  if (!match) throw new Error("no version in [workspace.package]");
  return match[1];
}

function bump(version, by) {
  const [major, minor, patch] = version.split(".").map(Number);

  if ([major, minor, patch].some(Number.isNaN)) {
    throw new Error(`${version} is not a version this can bump`);
  }

  if (by === "major") return `${major + 1}.0.0`;
  if (by === "minor") return `${major}.${minor + 1}.0`;
  return `${major}.${minor}.${patch + 1}`;
}

/**
 * Refuses for the four reasons a release goes wrong before it starts.
 *
 * All of them are cheap to check and expensive to discover afterwards, and
 * three of them cannot be undone once the tag is pushed.
 */
function refuseUnlessReady(tag) {
  const problems = [];

  if (git("status", "--porcelain") !== "") {
    problems.push("the tree has uncommitted changes — a release must be a commit that exists");
  }

  const branch = git("rev-parse", "--abbrev-ref", "HEAD");
  if (branch !== "main") {
    problems.push(`on ${branch}, not main — a tag off a branch publishes what was never reviewed`);
  }

  if (git("tag", "--list", tag) !== "") {
    problems.push(`${tag} already exists — a version is published once`);
  }

  git("fetch", "--quiet", "origin", "main");
  if (git("rev-parse", "HEAD") !== git("rev-parse", "origin/main")) {
    problems.push("HEAD is not origin/main — pull, or push, before tagging");
  }

  if (problems.length > 0) {
    for (const problem of problems) process.stderr.write(`error: ${problem}\n`);
    process.exit(1);
  }
}

/** The Cargo workspace version, and what each crate is required at by its siblings. */
function writeCargo(from, to) {
  const manifest = readFileSync("Cargo.toml", "utf8");
  const start = manifest.indexOf("[workspace.package]");
  const head = manifest.slice(0, start);
  const tail = manifest
    .slice(start)
    .replace(/^version\s*=\s*"[^"]+"/m, `version = "${to}"`)
    // Every `{ version = "…", path = "crates/…" }` in [workspace.dependencies].
    // Cargo will not publish a path dependency without one, so a bump of the
    // workspace alone leaves each of these behind.
    .replaceAll(
      new RegExp(`version\\s*=\\s*"${from}"(\\s*,\\s*path\\s*=\\s*"crates/)`, "g"),
      `version = "${to}"$1`,
    );

  writeFileSync("Cargo.toml", head + tail);
}

/**
 * Every publishable package and each exact internal package requirement.
 *
 * pnpm resolves `workspace:*` while packing, so those stay declarative. The
 * native CLI packages do not exist in the workspace, however: their exact
 * versions are generated during the release and the wrapper must request the
 * same version or a first install receives no executable.
 */
function writePackages(from, to) {
  const listed = execFileSync("git", ["ls-files", "-z", "packages"], { encoding: "utf8" })
    .split("\0")
    .filter((path) => path.endsWith("package.json"));
  const version = new RegExp(
    `("(?:@ubugeeei/slidx-[^"]+|slidx)"\\s*:\\s*)"${escapeRegExp(from)}"`,
    "g",
  );

  for (const path of listed) {
    const source = readFileSync(path, "utf8");
    if (JSON.parse(source).private === true) continue;

    // Rewritten as text rather than re-serialised, so key order, indentation
    // and the trailing newline survive — the same reason nothing in this
    // repository rewrites a file it only needed to edit part of.
    writeFileSync(
      path,
      source
        .replace(/^(\s*"version"\s*:\s*)"[^"]+"/m, `$1"${to}"`)
        .replaceAll(version, `$1"${to}"`),
    );
  }
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

const from = currentVersion();
const to = bump(from, level);
const tag = `v${to}`;

process.stdout.write(`release: ${from} → ${to}\n`);

refuseUnlessReady(tag);

writeCargo(from, to);
writePackages(from, to);
// The lockfile records a version per workspace member, and nothing in `vp
// check` passes `--locked`, so a forgotten update is invisible until the
// release's binary builds fail on it.
run("cargo", ["update", "--workspace", "--quiet"]);

// The same check the release workflow runs before it publishes. If this fails,
// somewhere a version lives that this script does not know about, and the
// answer is to teach it rather than to tag anyway.
run("node", ["scripts/check-version.mjs", tag]);

if (dryRun) {
  process.stdout.write(
    "\n--dry-run: the tree is written and nothing is committed.\n" +
      "  git diff            to read it\n" +
      "  git checkout -- .   to undo it\n",
  );
  process.exit(0);
}

run("git", ["commit", "--all", "--message", `release: ${to}`]);
run("git", ["tag", "--annotate", tag, "--message", `slidx ${to}`]);
run("git", ["push", "origin", "main"]);
run("git", ["push", "origin", tag]);

process.stdout.write(
  `\n${tag} pushed. The release workflow verifies the tree again from the same\n` +
    "task graph CI uses, then publishes to crates.io and npm over OIDC.\n" +
    `  gh run watch $(gh run list --workflow=Release --limit 1 --json databaseId --jq '.[0].databaseId')\n`,
);
