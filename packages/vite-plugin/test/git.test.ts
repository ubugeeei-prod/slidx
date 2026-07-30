/**
 * What the deck's history reads, and what it refuses to run.
 *
 * Half of these are about argument injection. Nothing here goes through a
 * shell, so a metacharacter is inert — but a *value* that begins with a dash is
 * still read by git as an option, and a repository is full of values somebody
 * else chose: a branch name, an author name, a path. Each of those has a test
 * that puts a git option where the value goes.
 */

import { execFile } from "node:child_process";
import { access, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

import { describe, expect, it } from "vite-plus/test";

import { openRepository, isRevision } from "../src/git";

const run = promisify(execFile);

interface Author {
  name?: string;
  email?: string;
}

async function git(root: string, ...args: string[]): Promise<string> {
  const { stdout } = await run("git", ["-C", root, ...args]);
  return stdout;
}

/** A repository holding a two-slide deck, with one commit per state. */
async function repository(options: { directory?: string; author?: Author } = {}): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "slidx-git-"));
  const slides = options.directory ?? "slides";

  await mkdir(join(root, slides), { recursive: true });
  await git(root, "init", "--quiet");
  await git(root, "config", "user.email", options.author?.email ?? "author@example.com");
  await git(root, "config", "user.name", options.author?.name ?? "The Author");
  // Commits carry a committer as well as an author, and git refuses to make
  // one without both when the environment has no identity of its own.
  await git(root, "config", "commit.gpgsign", "false");

  await writeFile(join(root, slides, "0001.md"), "# Making decks fast\n");
  await writeFile(join(root, slides, "0002.md"), "# What goes wrong\n\nthe wifi\n");
  await git(root, "add", "-A");
  await git(root, "commit", "--quiet", "-m", "the deck as the author wrote it");

  await writeFile(join(root, slides, "0003.md"), "# The fix\n");
  await git(root, "add", "-A");
  await git(root, "commit", "--quiet", "-m", "add the closing slide");

  return root;
}

describe("reading a deck's history", () => {
  it("says a directory is not a repository rather than failing to load", async () => {
    // A deck in a directory nobody ran `git init` in is an ordinary situation.
    // The editor still has to open.
    const plain = await mkdtemp(join(tmpdir(), "slidx-plain-"));

    expect(await openRepository(plain)).toBeNull();
  });

  it("finds the repository a deck is inside rather than looking for a .git directory", async () => {
    // A worktree's `.git` is a file, a submodule's points elsewhere, and
    // `GIT_DIR` overrides both. git knows; a directory check does not.
    const root = await repository();
    const inside = join(root, "slides");

    expect(await openRepository(inside)).not.toBeNull();
  });

  it("has an empty history in a repository with no commits yet", async () => {
    // What `git init` leaves. `git log` exits non-zero on it, and an editor
    // panel that reported that as a failure would be wrong about a new deck.
    const root = await mkdtemp(join(tmpdir(), "slidx-empty-"));
    await git(root, "init", "--quiet");
    await mkdir(join(root, "slides"), { recursive: true });

    const repo = await openRepository(root);
    expect(await repo!.log("slides", 20)).toEqual([]);
  });

  it("names who changed the deck, when, and what they called it", async () => {
    const root = await repository();
    const commits = await (await openRepository(root))!.log("slides", 20);

    expect(commits).toHaveLength(2);
    expect(commits[0]).toMatchObject({
      author: "The Author",
      subject: "add the closing slide",
    });
    // Newest first, which is the order a person looks for a change in.
    expect(commits[1]!.subject).toBe("the deck as the author wrote it");
    expect(commits[0]!.rev).toMatch(/^[0-9a-f]{40}$/);
    expect(Number.isNaN(Date.parse(commits[0]!.date))).toBe(false);
  });

  it("lists only the commits that touched the deck", async () => {
    const root = await repository();
    await writeFile(join(root, "README.md"), "# A repository that is not only a deck\n");
    await git(root, "add", "-A");
    await git(root, "commit", "--quiet", "-m", "write a readme");

    const commits = await (await openRepository(root))!.log("slides", 20);

    expect(commits.map((commit) => commit.subject)).not.toContain("write a readme");
  });

  it("reads the deck as the tree had it, not as the working copy has it", async () => {
    const root = await repository();
    const repo = await openRepository(root);
    const [newest, oldest] = await repo!.log("slides", 20);

    // Something the working copy says and the commit does not.
    await writeFile(join(root, "slides", "0001.md"), "# A title from after the commit\n");

    expect(await repo!.filesAt(oldest!.rev, "slides", [".md"])).toEqual([
      { name: "0001.md", source: "# Making decks fast\n" },
      { name: "0002.md", source: "# What goes wrong\n\nthe wifi\n" },
    ]);
    expect(await repo!.filesAt(newest!.rev, "slides", [".md"])).toHaveLength(3);
  });

  it("has no earlier version to compare the deck's first commit against", async () => {
    // `git show <root-commit>^` has no answer, and neither does this.
    const root = await repository();
    const repo = await openRepository(root);
    const [newest, oldest] = await repo!.log("slides", 20);

    expect(await repo!.parentOf(oldest!.rev)).toBeNull();
    expect(await repo!.parentOf(newest!.rev)).toBe(oldest!.rev);
  });
});

describe("what cannot become an argument", () => {
  it("refuses a revision that is a git option rather than an object name", async () => {
    // The panel only ever names a commit it was given, so the rule can be as
    // narrow as an object name. `--upload-pack` runs a program of the caller's
    // choosing; `--output` writes a file. Neither reaches a process.
    for (const hostile of [
      "--upload-pack=touch /tmp/slidx-pwned",
      "--output=/tmp/slidx-pwned",
      "-n1",
      "HEAD",
      "main",
      "../../etc/passwd",
      "deadbeef; touch /tmp/slidx-pwned",
      "",
    ]) {
      expect(isRevision(hostile), hostile).toBe(false);
    }

    expect(isRevision("0123456789abcdef0123456789abcdef01234567")).toBe(true);
    expect(isRevision("0123456")).toBe(true);
  });

  it("answers nothing for a revision it refused rather than reaching for git", async () => {
    const root = await repository();
    const repo = await openRepository(root);

    expect(await repo!.filesAt("--upload-pack=touch /tmp/slidx-pwned", "slides", [".md"])).toEqual(
      [],
    );
    expect(await repo!.parentOf("--upload-pack=echo")).toBeNull();
  });

  it("reads a deck out of a directory whose name is shell punctuation", async () => {
    // Every argument is passed as an argument, so `;` is four characters in a
    // directory name rather than the end of a command. The marker file is what
    // proves it: a shell would have made one.
    const marker = join(tmpdir(), `slidx-pwned-${process.pid}`);
    const root = await repository({ directory: `my slides; touch ${marker}` });

    const repo = await openRepository(root);
    const commits = await repo!.log(`my slides; touch ${marker}`, 20);

    expect(commits).toHaveLength(2);
    expect(await repo!.filesAt(commits[1]!.rev, `my slides; touch ${marker}`, [".md"])).toHaveLength(
      2,
    );
    await expect(access(marker)).rejects.toThrow();
  });

  it("reads a deck out of a directory whose name begins with a dash", async () => {
    // A pathspec is separated from the options by `--`, so a directory called
    // `--output` is a directory rather than a place to write a file.
    const root = await repository({ directory: "--output" });
    const repo = await openRepository(root);

    expect(await repo!.log("--output", 20)).toHaveLength(2);
  });

  it("carries an author name that looks like a git option back as text", async () => {
    // An author name is output. It is never an argument, and a repository
    // someone else wrote is full of values they chose.
    const name = "--upload-pack=touch /tmp/slidx-author; echo";
    const root = await repository({ author: { name } });

    const commits = await (await openRepository(root))!.log("slides", 20);

    expect(commits[0]!.author).toBe(name);
    await expect(access("/tmp/slidx-author")).rejects.toThrow();
  });

  it("keeps a commit subject from forging a second commit or shifting a field", async () => {
    // Records are separated by a NUL, which a commit message cannot contain,
    // rather than by a newline, which every commit message can. The separators
    // this format does use are ordinary text inside a subject, so a message
    // holding them has to come back whole and in the right field.
    const subject = "a subject\x1fpretending to be\x1eanother record";
    const root = await repository();
    await writeFile(join(root, "slides", "0004.md"), "# One more\n");
    await git(root, "add", "-A");
    await git(root, "commit", "--quiet", "-m", subject);

    const commits = await (await openRepository(root))!.log("slides", 20);

    expect(commits).toHaveLength(3);
    expect(commits[0]!.subject).toBe(subject);
    expect(commits[0]!.author).toBe("The Author");
    expect(commits[0]!.rev).toMatch(/^[0-9a-f]{40}$/);
  });

  it("stays inside the deck when a branch name is shell punctuation", async () => {
    // A branch name is never an argument here: the log is read from HEAD,
    // whatever HEAD happens to be pointing at. Spaces are not a legal ref, so
    // this is the worst a branch name is allowed to be.
    const root = await repository();
    await git(root, "switch", "--quiet", "-c", "wip/$(touch_/tmp/slidx-branch)");

    const commits = await (await openRepository(root))!.log("slides", 20);

    expect(commits).toHaveLength(2);
    await expect(access("/tmp/slidx-branch")).rejects.toThrow();
  });
});
