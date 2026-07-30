/**
 * The deck's history, read through the git the author already has.
 *
 * No git library. A deck lives in the author's repository, with their hooks,
 * their `core.autocrlf`, their `.gitattributes`, their submodules and their
 * signing key — and a reimplementation that agreed with all of that on every
 * platform is a large dependency to take on for the sake of reading a log. The
 * binary on PATH is the one their repository is already configured for, and it
 * is installed everywhere this feature makes sense.
 *
 * # Nothing here is a shell
 *
 * Every argument is passed as an argument. `execFile` with an array never
 * builds a command line, so a deck in `~/talks/Vue Fes 2026` needs no quoting
 * and a semicolon in a directory name is a character rather than the end of a
 * command.
 *
 * That is only half of it. A value that begins with a dash is read by git as an
 * *option* however it was passed, and a repository is full of values somebody
 * else chose — a branch, an author, a path. So:
 *
 * - **A revision is an object name or it is refused**, before a process starts.
 *   The panel only ever names a commit this module gave it, so the rule can be
 *   as narrow as hexadecimal. `--upload-pack=…` runs a program of the caller's
 *   choosing and never gets the chance.
 * - **A path follows `--`**, so a deck directory called `--output` is a
 *   directory rather than a file to write.
 * - **A branch and an author are output**, never arguments. The log is read
 *   from HEAD, whatever HEAD points at.
 *
 * # Absence is an answer
 *
 * No git, no repository, and no commits are three ordinary states of a machine
 * with a deck on it, and none of them may stop the editor loading. Each comes
 * back as nothing rather than as a throw.
 */

import { execFile } from "node:child_process";
import { promisify } from "node:util";

const run = promisify(execFile);

/** One commit that touched the deck. */
export interface Commit {
  /** The full object name, which is the only thing this module accepts back. */
  rev: string;
  author: string;
  /** ISO-8601, so whoever shows it decides how a date is spelled. */
  date: string;
  /** The commit's own first line, as the author wrote it. */
  subject: string;
}

/** One file, as a commit had it. */
export interface TreeFile {
  /** The name inside the deck directory. */
  name: string;
  source: string;
}

export interface Repository {
  /** Commits that touched `directory`, newest first. */
  log(directory: string, limit: number): Promise<Commit[]>;
  /** The deck's files as a commit had them, in the order a deck is read. */
  filesAt(rev: string, directory: string, extensions: string[]): Promise<TreeFile[]>;
  /** The commit before this one, or `null` for the first commit of all. */
  parentOf(rev: string): Promise<string | null>;
}

/**
 * A revision this module will pass to git.
 *
 * Deliberately narrower than what git accepts. Every revision the editor names
 * came out of [`Repository.log`], so it is always a full object name, and a
 * rule that admits nothing else is a rule with no gap in it to reason about.
 * Seven characters is the shortest abbreviation git itself prints.
 */
export function isRevision(value: string): boolean {
  return /^[0-9a-f]{7,64}$/.test(value);
}

/**
 * The repository a directory is inside, or nothing.
 *
 * Asks git rather than looking for a `.git` directory: a worktree's is a file,
 * a submodule's points elsewhere, and `GIT_DIR` overrides both.
 */
export async function openRepository(root: string): Promise<Repository | null> {
  const found = await git(root, ["rev-parse", "--show-toplevel"]);
  if (found === null) return null;

  return repository(root);
}

function repository(root: string): Repository {
  return {
    async log(directory, limit) {
      // Fields separated by a unit separator and records by a NUL, because a
      // commit message can contain any byte a text file can — including a
      // newline, which is why the obvious format is the wrong one. NUL is the
      // one byte a commit message cannot hold, and `-z` is git saying so.
      const output = await git(root, [
        "log",
        "-z",
        `--max-count=${Math.max(1, Math.trunc(limit))}`,
        "--format=%H\x1f%an\x1f%aI\x1f%s",
        "--",
        directory,
      ]);

      // A repository with no commits at all: what `git init` leaves, and an
      // ordinary state for a deck somebody started this morning.
      if (output === null) return [];

      return output
        .split("\0")
        .filter((record) => record.length > 0)
        .map(commitFrom)
        .filter((commit): commit is Commit => commit !== null);
    },

    async filesAt(rev, directory, extensions) {
      if (!isRevision(rev)) return [];

      const listed = await git(root, ["ls-tree", "-z", "--name-only", rev, "--", `${directory}/`]);
      if (listed === null) return [];

      const paths = listed
        .split("\0")
        .filter((path) => path.length > 0)
        .filter((path) => extensions.some((extension) => path.toLowerCase().endsWith(extension)))
        // The same order the deck reader uses, which is why the convention is
        // `0001.md`: a numeric prefix sorts correctly as a string.
        .sort((a, b) => a.localeCompare(b, "en"));

      // One `git show` per file rather than a `cat-file --batch` protocol.
      // A deck is dozens of files and these run at once, so the whole read is
      // a few milliseconds on a click — and a batch parser would be a second
      // way to get a blob wrong for a saving nobody would feel.
      const files = await Promise.all(
        paths.map(async (path) => {
          const source = await git(root, ["show", `${rev}:${path}`]);
          return source === null ? null : { name: nameIn(directory, path), source };
        }),
      );

      return files.filter((file): file is TreeFile => file !== null);
    },

    async parentOf(rev) {
      if (!isRevision(rev)) return null;

      // `<rev>^` fails on the first commit of all rather than answering, which
      // is the case this returns nothing for.
      const parent = await git(root, ["rev-parse", "--verify", "--quiet", `${rev}^{commit}^`]);

      return parent === null ? null : parent.trim() || null;
    },
  };
}

function commitFrom(record: string): Commit | null {
  // The subject is whatever is left, so a message that happens to contain a
  // unit separator loses nothing and cannot shift the fields before it.
  const [rev, author, date, ...rest] = record.split("\x1f");
  if (rev === undefined || author === undefined || date === undefined) return null;

  return { rev: rev.trim(), author, date, subject: rest.join("\x1f") };
}

/** A tree path, as a name inside the deck directory. */
function nameIn(directory: string, path: string): string {
  return path.startsWith(`${directory}/`) ? path.slice(directory.length + 1) : path;
}

/**
 * One git command, or nothing.
 *
 * Every failure is the same answer: no git on PATH, not a repository, no
 * commits yet, a path that was never committed. None of them is a reason to
 * stop, and telling them apart here would only move the decision to a caller
 * that would treat them the same way.
 */
async function git(cwd: string, args: string[]): Promise<string | null> {
  try {
    const { stdout } = await run("git", ["-C", cwd, ...args], {
      // A slide file is text and a deck is not large. The cap is here so a
      // repository that is not what this expects cannot buffer without bound.
      maxBuffer: 32 * 1024 * 1024,
      encoding: "utf8",
      // Reads only, and none of these take a lock — but a git configured with
      // a pager or an editor would otherwise be waiting for a terminal that
      // this process does not have.
      env: { ...process.env, GIT_PAGER: "cat", GIT_OPTIONAL_LOCKS: "0" },
    });

    return stdout;
  } catch {
    return null;
  }
}
