/**
 * How a release reaches the remote, which is where the old one could not.
 *
 * `release.mjs` used to end with `git push origin main`. `main` takes pull
 * requests and requires `ci`, so that push was refused on every version — and
 * it refused *after* the script had committed and tagged locally, leaving a
 * half-applied release behind and a tag that made the next attempt refuse too.
 *
 * Its tests read the files it wrote and never the push it ended with, so a
 * script that could not complete passed everything. The half that talks to a
 * remote is therefore exercised here against a real one: a bare repository in a
 * temporary directory, which is enough to prove where a tag lands.
 */

import { execFileSync, spawnSync } from "node:child_process";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";

const RELEASE = join(import.meta.dirname, "..", "release.mjs");
const source = await readFile(RELEASE, "utf8");

/** The script with its prose taken out, so a comment cannot satisfy a check. */
const code = source.replaceAll(/\/\*[\s\S]*?\*\/|\/\/.*/g, "");

/** What runs after `--tag` has been handled, which is the pull-request half. */
const bumping = code.slice(code.indexOf("if (tagOnly)"));

function git(cwd, ...argv) {
  return execFileSync("git", argv, { cwd, encoding: "utf8" }).trim();
}

/** A clone of a bare remote whose `main` carries `version`. */
async function repository(version) {
  const root = await mkdtemp(join(tmpdir(), "slidx-release-"));
  const remote = join(root, "remote.git");
  const work = join(root, "work");

  execFileSync("git", ["init", "--bare", "--initial-branch=main", remote]);
  execFileSync("git", ["clone", "--quiet", remote, work]);
  git(work, "config", "user.email", "release@example.com");
  git(work, "config", "user.name", "The Release");

  await writeFile(
    join(work, "Cargo.toml"),
    `[workspace]\n\n[workspace.package]\nversion = "${version}"\n`,
  );
  git(work, "add", "-A");
  git(work, "commit", "--quiet", "--message", `release: ${version}`);
  git(work, "push", "--quiet", "origin", "main");

  return { root, remote, work };
}

const tagging = (work) =>
  spawnSync(process.execPath, [RELEASE, "--tag"], { cwd: work, encoding: "utf8" });

describe("tagging what merged", () => {
  it("puts the tag on the remote, naming the version origin/main carries", async () => {
    const { remote, work } = await repository("0.3.0");
    const result = tagging(work);

    expect(result.status, result.stderr).toBe(0);
    expect(git(remote, "tag", "--list")).toBe("v0.3.0");
  });

  it("tags the commit on origin/main rather than whatever is checked out", async () => {
    // The two are different questions: one is what was reviewed and merged,
    // and the other is whatever happens to be lying around. Tagging the second
    // is how a tag comes to name bytes nobody agreed to publish.
    const { remote, work } = await repository("0.3.0");
    const merged = git(work, "rev-parse", "origin/main");

    await writeFile(join(work, "Cargo.toml"), '[workspace.package]\nversion = "9.9.9"\n');
    git(work, "commit", "--quiet", "--all", "--message", "not reviewed");

    expect(tagging(work).status).toBe(0);
    expect(git(remote, "tag", "--list")).toBe("v0.3.0");
    expect(git(remote, "rev-list", "-n", "1", "v0.3.0")).toBe(merged);
  });

  it("refuses a version that is already tagged on the remote", async () => {
    const { work } = await repository("0.3.0");
    expect(tagging(work).status).toBe(0);

    const again = tagging(work);
    expect(again.status).toBe(1);
    expect(again.stderr).toContain("already");
  });

  it("says how to clear a tag left behind here by a failure", async () => {
    const { work } = await repository("0.3.0");
    git(work, "tag", "--annotate", "v0.3.0", "--message", "leftover");

    const result = tagging(work);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("git tag -d v0.3.0");
  });
});

describe("the shape of the half that cannot be run here", () => {
  it("never pushes to main, which is the bug this pair of commands exists for", () => {
    // Read past the comments, because the header explains the bug by naming
    // the command that caused it — the same reason `check-claims.mjs` does not
    // quote the phrases it forbids.
    expect(code).not.toMatch(/"push",[^)]*"main"/);
    expect(code).not.toMatch(/push origin main/);
  });

  it("pushes a branch and opens a pull request for it", () => {
    expect(code).toMatch(/"switch",\s*"--create"/);
    expect(code).toMatch(/"pr",\s*\n?\s*"create"/);
    expect(code).toMatch(/"pr",\s*"merge",\s*"--auto"/);
  });

  it("tags nothing while opening the pull request", () => {
    // A tag written before the merge names a commit the squash will not
    // produce, and the old script left one behind on every failure — which is
    // what made each next attempt refuse.
    expect(bumping).not.toMatch(/"tag"/);
  });

  it("offers both halves in its usage", () => {
    const usage = spawnSync(process.execPath, [RELEASE], { encoding: "utf8" });

    expect(usage.status).toBe(2);
    expect(usage.stderr).toContain("vp run release <major|minor|patch>");
    expect(usage.stderr).toContain("vp run release --tag");
  });
});
