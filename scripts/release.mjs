/**
 * Cut a release, in the two halves the branch rules make it.
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
 * # Why it is two commands
 *
 * It used to be one, and it ended with `git push origin main`. `main` requires
 * a pull request and a passing `ci`, so that push was refused every time — the
 * script could not complete on any version, and nobody found out until the
 * first release, because its tests read the files it writes and never the push
 * it ends with.
 *
 * It failed in the worst available place, too: after committing and tagging
 * locally. Every attempt left a half-applied release behind and a tag that made
 * the next attempt refuse.
 *
 * So the version bump is an ordinary pull request like every other change, and
 * a release is a **tag on a commit that is already on main**:
 *
 *     vp run release minor    # writes the bump, opens the pull request
 *     vp run release --tag    # once it has merged, tags origin/main
 *
 * The second half reads the version out of `origin/main` rather than out of the
 * working tree, so it tags what was reviewed rather than what is lying around.
 *
 * Pushing the tag is what starts a publish that cannot be taken back, and it is
 * the only thing the second command does. Everything that could fail is made to
 * fail before it — a dirty tree, the wrong branch, a tag that exists here or on
 * the remote, a version the check rejects, a `main` that does not carry it.
 */

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const LEVELS = ["major", "minor", "patch"];

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const tagOnly = args.includes("--tag");
const level = args.find((argument) => !argument.startsWith("-"));

if (!tagOnly && !LEVELS.includes(level)) {
  process.stderr.write(
    `usage: vp run release <${LEVELS.join("|")}> [--dry-run]\n` +
      "       vp run release --tag\n\n" +
      "The first writes the version across the Cargo workspace and every\n" +
      "publishable package and opens the pull request for it. The second tags\n" +
      "origin/main once that has merged, which is what starts the release.\n",
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

  problems.push(...tagIsFree(tag));

  git("fetch", "--quiet", "origin", "main");
  if (git("rev-parse", "HEAD") !== git("rev-parse", "origin/main")) {
    problems.push("HEAD is not origin/main — pull, or push, before tagging");
  }

  refuse(problems);
}

/**
 * That the tag exists in neither place.
 *
 * Both, because they go wrong differently: one here refuses the next attempt
 * after a failure, and one on the remote is a version somebody has already
 * released.
 */
function tagIsFree(tag) {
  const problems = [];

  if (git("tag", "--list", tag) !== "") {
    problems.push(`${tag} already exists here — \`git tag -d ${tag}\` if it is a leftover`);
  }

  if (git("ls-remote", "--tags", "origin", tag) !== "") {
    problems.push(`${tag} is already on the remote — a version is released once`);
  }

  return problems;
}

function refuse(problems) {
  if (problems.length === 0) return;

  for (const problem of problems) process.stderr.write(`error: ${problem}\n`);
  process.exit(1);
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
  const version = new RegExp(`("(?:@slidxjs/[^"]+|slidx)"\\s*:\\s*)"${escapeRegExp(from)}"`, "g");

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

/**
 * Tags the commit on `origin/main`, which is the whole of a release.
 *
 * The version is read from `origin/main` rather than from the working tree,
 * because those are different questions: one is what was reviewed and merged,
 * and the other is whatever happens to be checked out. Tagging the second is
 * how a tag comes to name bytes nobody agreed to publish.
 *
 * Nothing is checked out or merged locally either — the tag is placed on
 * `origin/main` directly, so a stale local `main` cannot move it.
 */
function tagMergedVersion() {
  git("fetch", "--quiet", "--tags", "origin", "main");

  const merged = /^version\s*=\s*"([^"]+)"/m.exec(
    git("show", "origin/main:Cargo.toml").slice(
      git("show", "origin/main:Cargo.toml").indexOf("[workspace.package]"),
    ),
  );

  if (!merged) {
    process.stderr.write("error: no version in origin/main's [workspace.package]\n");
    process.exit(1);
  }

  const tag = `v${merged[1]}`;
  refuse(tagIsFree(tag));

  run("git", ["tag", "--annotate", tag, "origin/main", "--message", `slidx ${merged[1]}`]);
  run("git", ["push", "origin", tag]);

  process.stdout.write(
    `\n${tag} pushed, on ${git("rev-parse", "--short", "origin/main")}. The release workflow\n` +
      "verifies the tree again from the same task graph CI uses, then publishes to\n" +
      "crates.io and npm over OIDC.\n" +
      `  gh run watch $(gh run list --workflow=Release --limit 1 --json databaseId --jq '.[0].databaseId')\n`,
  );
}

if (tagOnly) {
  tagMergedVersion();
  process.exit(0);
}

const from = currentVersion();
const to = bump(from, level);
const tag = `v${to}`;
const branch = `release/${tag}`;

process.stdout.write(`release: ${from} → ${to}\n`);

refuseUnlessReady(tag);

writeCargo(from, to);
writePackages(from, to);
// The lockfile records a version per workspace member, and nothing in `vp
// check` passes `--locked`, so a forgotten update is invisible until the
// release's binary builds fail on it.
run("cargo", ["update", "--workspace", "--quiet"]);

// Brand tokens carry the workspace version so published assets can be traced
// back to the release that generated them. Regenerate after the Cargo bump,
// before verification, so a release never commits stale derived files.
run("pnpm", ["exec", "vp", "run", "generate:brand"]);

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

// On a branch, and nothing is tagged here. `main` takes pull requests only, and
// a tag written before the merge names a commit the merge will not produce —
// which is how the old one-command version left a tag behind on every failure
// and made the next attempt refuse.
run("git", ["switch", "--create", branch]);
run("git", ["commit", "--all", "--message", `release: ${to}`]);
run("git", ["push", "--set-upstream", "origin", branch]);

run("gh", [
  "pr",
  "create",
  "--title",
  `release: ${to}`,
  "--body",
  `The version, written across the Cargo workspace, the lockfile, every publishable package and the brand assets that carry it. \`check-version.mjs\` agrees with \`${tag}\`.\n\nNothing is published by merging this. \`vp run release --tag\` afterwards tags \`origin/main\`, and that is what starts the release.`,
]);
run("gh", ["pr", "merge", "--auto", "--squash"]);

// Back where the author started. The branch has served its purpose the moment
// it is pushed, and leaving somebody standing on it is how the next command
// runs against the wrong tree.
run("git", ["switch", "main"]);

process.stdout.write(
  `\n${branch} pushed, and set to merge when CI passes. Then:\n` +
    "  vp run release --tag\n\n" +
    "which reads the version off origin/main and tags the commit that merged.\n" +
    "That tag is what publishes, and it is the last thing that can be taken back.\n",
);
