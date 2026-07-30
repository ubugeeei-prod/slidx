/**
 * The deck's past, as the editor asks for it.
 *
 * A deck is plain Markdown in a git repository and the renderer is a pure
 * function of that Markdown, so a commit is enough to say exactly what the deck
 * was. This module turns a commit into the two things a speaker wants from one:
 * who changed the deck and when, and *what changed* — not `+34 −6`, but the
 * slides that moved and the budget that grew.
 *
 * # It never stops the editor loading
 *
 * A deck in a directory nobody ran `git init` in is an ordinary situation, and
 * so is a machine with no git on it. Both come back as an answer saying which
 * one it is, so the panel can say so plainly and stay out of the way. Nothing
 * here throws for the absence of a repository.
 *
 * # The source it compares is the source a build reads
 *
 * A commit's slide files are joined by [`joinDeck`](./files) — the same
 * function, with the same separator, that the dev server uses on the working
 * copy. A second way of assembling a deck from its files would be a second
 * answer about where one slide ends, which is exactly what the editor's byte
 * spans depend on not existing.
 */

import { basename } from "node:path";

import { readDeck, type DeckSource } from "./deck";
import { hasGit, isRevision, openRepository, type Commit, type Repository } from "./git";
import { joinDeck } from "./files";
import type { ResolvedOptions } from "./options";
import { summarise, type DeckSummary } from "./pipeline";

/**
 * How far back the panel looks.
 *
 * A deck has tens of commits, not thousands, and a list nobody scrolls to the
 * end of is a list that could have been shorter. Reading more would cost a
 * process per extra commit for rows an author never sees.
 */
const DEPTH = 60;

/** What there is to show, or why there is nothing. */
export interface HistoryAnswer {
  /** True when this deck is in a repository that can be read. */
  available: boolean;
  /**
   * What to say instead. Present only when `available` is false, and written
   * for an author rather than for a log.
   */
  reason?: string;
  /** Commits that touched the deck, newest first. */
  commits: Commit[];
}

/** What happened when the deck was asked to go back. */
export interface RestoreAnswer {
  /** The commit the deck now matches. Absent when nothing was done. */
  restored?: string;
  /**
   * The commit to name to undo this, which is where the deck was a moment ago.
   *
   * Undo is not a special path: it is the same operation naming this.
   */
  previous?: string;
  /** Why nothing was done, written for an author. */
  refused?: string;
  /** What is unsaved, when that is the reason. */
  changed?: string[];
}

/** Reads one project's deck history for as long as the dev server runs. */
export interface DeckHistory {
  commits(): Promise<HistoryAnswer>;
  /**
   * What one commit did to the deck, against the commit before it.
   *
   * `null` when the revision is not one this repository has — which includes
   * every revision that is not an object name at all.
   */
  changeAt(rev: string): Promise<DeckSummary | null>;
  /**
   * The deck's files as one commit had them, for rendering the pages it was.
   *
   * Shaped exactly like a read from disk, because it is handed to the same
   * renderer: the preview at a commit has to be the deck's own page, produced
   * by the same WebAssembly module through the same shell and the same theme.
   * A second way of drawing it would be a second answer about layout, which is
   * the bug this architecture exists to prevent.
   */
  deckAt(rev: string): Promise<DeckSource | null>;
  /** Puts the deck back to a commit, or says why it did not. */
  restore(rev: string): Promise<RestoreAnswer>;
}

export function createDeckHistory(root: string, options: ResolvedOptions): DeckHistory {
  /**
   * The commit this session last put in the working copy.
   *
   * Remembered so that undoing a restore is possible at all. A restore leaves
   * the deck dirty by construction — that is what an author reviews and
   * commits — and the guard below refuses to write over anything unsaved. So
   * without knowing which dirt is its own, the panel would offer an undo that
   * always refused.
   */
  let placed: string | undefined;

  /** The deck's files as one commit had them, shaped like a read from disk. */
  async function filesAt(repository: Repository, rev: string): Promise<DeckSource> {
    const found = await repository.filesAt(rev, options.srcDir, options.extensions);
    const files = found.map((file) => ({
      path: file.name,
      label: file.name,
      source: file.source,
    }));

    return { files, source: joinDeck(files, options.separator).source };
  }

  /** The deck as one commit had it, joined the way the parser reads it. */
  async function sourceAt(repository: Repository, rev: string): Promise<string> {
    return (await filesAt(repository, rev)).source;
  }

  /**
   * True when the deck on disk is byte for byte what a commit had.
   *
   * Compared file by file rather than through the joined source, because the
   * joined source is trimmed at every file's edges — and a guard about not
   * losing an author's bytes cannot be built on a comparison that ignores some
   * of them.
   */
  async function matches(repository: Repository, rev: string): Promise<boolean> {
    const [tree, disk] = await Promise.all([
      filesAt(repository, rev),
      readDeck(root, options.srcDir, options.extensions, options.separator),
    ]);

    if (tree.files.length !== disk.files.length) return false;

    return disk.files.every((file, index) => {
      const committed = tree.files[index];
      return committed?.label === basename(file.label) && committed.source === file.source;
    });
  }

  return {
    async commits() {
      const repository = await openRepository(root);

      if (repository === null) {
        return {
          available: false,
          reason: (await hasGit(root))
            ? "This deck is not in a git repository, so there is no history to read."
            : "git is not installed, so there is no history to read.",
          commits: [],
        };
      }

      return { available: true, commits: await repository.log(options.srcDir, DEPTH) };
    },

    async changeAt(rev) {
      if (!isRevision(rev)) return null;

      const repository = await openRepository(root);
      if (repository === null) return null;

      // Asked before anything is read, because every read below answers
      // nothing for a failure. Without this, a revision the repository does
      // not have would look exactly like a commit whose deck was empty — and
      // the panel would report a deck arriving with no slides in it.
      if ((await repository.resolve(rev)) === null) return null;

      const after = await sourceAt(repository, rev);
      const parent = await repository.parentOf(rev);

      // No parent is the deck's first commit. Absence rather than an empty
      // string: comparing against nothing says the deck arrived, comparing
      // against an empty deck says every slide was added, and only one of
      // those is what happened.
      const before = parent === null ? undefined : await sourceAt(repository, parent);

      return summarise(before, after, { separator: options.separator });
    },

    async deckAt(rev) {
      if (!isRevision(rev)) return null;

      const repository = await openRepository(root);
      if (repository === null) return null;
      if ((await repository.resolve(rev)) === null) return null;

      return filesAt(repository, rev);
    },

    async restore(rev) {
      if (!isRevision(rev)) return { refused: "That is not a commit this deck has." };

      const repository = await openRepository(root);
      if (repository === null) {
        return {
          refused: "This deck is not in a git repository, so there is nothing to go back to.",
        };
      }

      if ((await repository.resolve(rev)) === null) {
        return { refused: "This repository does not have that commit any more." };
      }

      // Where the deck is now, which is the commit an undo would name. Clean
      // means HEAD; otherwise the only acceptable dirt is a restore this
      // session made, which is not work anybody would lose.
      const changed = await repository.changesIn(options.srcDir);
      const previous =
        changed.length === 0
          ? await repository.head()
          : placed !== undefined && (await matches(repository, placed))
            ? placed
            : null;

      if (previous === null) {
        return {
          refused:
            "The deck has changes that are not committed. Save or discard them first — " +
            "going back would write over them.",
          changed,
        };
      }

      if (!(await repository.restore(rev, options.srcDir))) {
        return { refused: "git would not put the deck back." };
      }

      placed = rev;

      return { restored: rev, previous };
    },
  };
}
