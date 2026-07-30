/**
 * The one document every writer of a deck's bytes agrees on.
 *
 * # The problem this exists to not create
 *
 * `packages/editor/src/history.ts` refuses to keep a stack of whole sources,
 * and says why: it would mean the editor holds a second copy of the document
 * that can disagree with the file, which is the failure the whole architecture
 * exists to prevent. A CRDT is, on its face, exactly that second copy. So
 * adding one has to be done in a way that does not make that sentence false.
 *
 * The reconciliation is that a character-level CRDT and a byte-range splice
 * want the same thing: *change these bytes, leave every other byte alone.* So
 * the CRDT goes **under** the operation set rather than beside it. An operation
 * is still planned by `slidx_edit`, still produces a splice, and the splice is
 * still what decides the bytes — it is applied into this document instead of
 * straight into the files, and the files are written from what comes out.
 *
 * Two consequences are worth stating because they are what keep the claim true:
 *
 * **No browser holds one of these.** A connected editor is sent deck state, the
 * same shape `GET /__slidx/deck` already returns, and replaces its view of the
 * deck wholesale. It never merges anything locally, so there is still exactly
 * one thing in the system that can disagree with the file, and it is this.
 *
 * **With one editor connected, the bytes are unchanged.** Applying a splice into
 * this document and reading the text back is byte-for-byte what
 * `Edit::apply` would have written on its own. That is a property rather than a
 * hope: `test/collab.test.ts` runs the same operation with the document in the
 * path and with it out, and compares the files.
 *
 * # What it earns, stated honestly
 *
 * The file on disk is a second writer. An author edits `0004.md` in their own
 * text editor while a co-presenter drags a block in the canvas, and those two
 * changes arrive by different routes at nearly the same moment. Before this,
 * whichever finished last wrote the whole file and the other change was gone.
 *
 * The reason that is hard, and the reason it needs a CRDT rather than care, is
 * that a splice is a pair of *offsets*. Planning an operation means parsing a
 * deck, which means an await, and the author's editor is free to save during it.
 * The offsets are then stale: the bytes they name have moved. See [`Fork`] —
 * the splice is applied to a copy of the document taken before that wait, and
 * what merges back is the difference, so Yjs places the change by character
 * identity instead of by an index that no longer means anything.
 *
 * What it does *not* do is let two peers diverge and reconcile later. That would
 * need each peer to hold a document, which is the thing being refused. Every
 * change goes through the dev server, because the dev server is the only thing
 * that can write a file.
 *
 * # Why Yjs
 *
 * Because a subtly wrong merge corrupts a talk, and merge semantics are the
 * wrong place for original work. Yjs is the most-exercised CRDT on this
 * platform, its own test suite fuzzes concurrent editing against a reference
 * implementation, and it has one dependency.
 *
 * It costs the `slidx` binary **nothing**, and that is not an accident of
 * packaging. `slidx dev` starts the project's dev server rather than being one,
 * so the CRDT lives in the process that already has Node and the deck's
 * `node_modules`, and the prebuilt binary people are asked to `curl | sh` does
 * not grow a byte or a millisecond of start-up for a feature it does not run.
 * On the plugin's side it is roughly 90 kB of JavaScript that is loaded only
 * when the dev server starts, and never by a viewer of a built deck.
 */

import * as Y from "yjs";

/** The name of the text inside the document. Never read by a client. */
const DECK = "deck";

/** One byte range and the text that takes its place, in UTF-16 units. */
export interface TextSplice {
  /** Where the replacement starts, counted the way `Y.Text` counts. */
  readonly at: number;
  readonly remove: number;
  readonly text: string;
}

/**
 * The document as it stood when an operation was planned against it.
 *
 * This is the piece only a CRDT can provide, and the reason there is one here at
 * all. An operation's splice is a pair of *offsets*, and offsets are measured in
 * a document that has already moved by the time the splice is ready: planning
 * one means parsing a deck, which means an await, and the author's own text
 * editor is free to save a file in the middle of it. Applying the splice at its
 * literal offsets would then land it in the wrong place, and moving it by
 * guesswork would be a merge algorithm written here — the last thing anybody
 * should be writing by hand.
 *
 * So the splice is applied to a copy taken *before* the wait, and what merges is
 * the difference. Yjs decides where the change belongs by character identity
 * rather than by index, so a paragraph the author saved and an operation from
 * the canvas both survive whichever way round they arrived.
 */
export interface Fork {
  /** The document's text at the moment the fork was taken. */
  readonly text: string;
  /** Applies a splice to the fork, merges it back, and returns the result. */
  merge(splice: TextSplice | null): string;
}

export interface SharedDeck {
  /** The deck source as every writer currently agrees it reads. */
  text(): string;
  /** Folds a new reading of the whole deck in, as the one splice between them. */
  adopt(source: string): boolean;
  /** A copy of the document to plan a change against. */
  fork(): Fork;
  destroy(): void;
}

export function createSharedDeck(source: string): SharedDeck {
  const doc = new Y.Doc();
  const text = doc.getText(DECK);
  if (source.length > 0) text.insert(0, source);

  return {
    text: () => text.toString(),

    adopt(source) {
      const splice = spliceBetween(text.toString(), source);
      if (splice === null) return false;

      // One transaction, so nothing observes a document that has briefly lost
      // a paragraph.
      doc.transact(() => {
        if (splice.remove > 0) text.delete(splice.at, splice.remove);
        if (splice.text.length > 0) text.insert(splice.at, splice.text);
      });

      return true;
    },

    fork() {
      const taken = Y.encodeStateAsUpdate(doc);
      const from = Y.encodeStateVector(doc);

      return {
        text: text.toString(),

        merge(splice) {
          if (splice === null) return text.toString();

          const branch = new Y.Doc();
          Y.applyUpdate(branch, taken);
          const copy = branch.getText(DECK);

          branch.transact(() => {
            if (splice.remove > 0) copy.delete(splice.at, splice.remove);
            if (splice.text.length > 0) copy.insert(splice.at, splice.text);
          });

          // Only what the branch added, so nothing the main document did in the
          // meantime is undone by replaying an older state over it.
          Y.applyUpdate(doc, Y.encodeStateAsUpdate(branch, from));
          branch.destroy();

          return text.toString();
        },
      };
    },

    destroy: () => doc.destroy(),
  };
}

/**
 * The one range that differs between two readings of a deck.
 *
 * Common prefix, common suffix, and everything between them replaced. That is
 * the same statement a `slidx_edit` splice makes, which is why an operation's
 * result and a file that changed on disk can both come through one door.
 *
 * An operation that changes two ranges at once — setting notes on a slide that
 * has several notes comments — collapses to one range covering both. The bytes
 * between them are rewritten identically, so not one byte of the file differs;
 * what it costs is a wider region for a concurrent edit to land inside, which
 * for two comments on one slide is nothing anybody can observe.
 *
 * Returns `null` when the two readings are the same, so an operation that asked
 * for what the deck already said touches nothing at all — the same rule
 * `EditBuilder` keeps on the other side of the boundary.
 */
export function spliceBetween(before: string, after: string): TextSplice | null {
  if (before === after) return null;

  const limit = Math.min(before.length, after.length);
  let prefix = 0;
  while (prefix < limit && before[prefix] === after[prefix]) prefix += 1;

  let suffix = 0;
  while (
    suffix < limit - prefix &&
    before[before.length - 1 - suffix] === after[after.length - 1 - suffix]
  ) {
    suffix += 1;
  }

  // A boundary inside a surrogate pair would still produce the right text, but
  // it puts a lone half of a character into the document — so back off to the
  // start of the pair. An emoji in a slide title is enough to reach this.
  prefix = safeStart(after, prefix);
  suffix = Math.min(suffix, before.length - prefix, after.length - prefix);
  suffix = safeEnd(after, suffix);
  suffix = Math.min(suffix, before.length - prefix, after.length - prefix);

  return {
    at: prefix,
    remove: before.length - prefix - suffix,
    text: after.slice(prefix, after.length - suffix),
  };
}

/** Pulls an index back off the low half of a surrogate pair. */
function safeStart(text: string, index: number): number {
  return index > 0 && isLowSurrogate(text.charCodeAt(index)) ? index - 1 : index;
}

/** Pulls a suffix length back so it does not begin on a low surrogate. */
function safeEnd(text: string, suffix: number): number {
  const index = text.length - suffix;

  return index < text.length && isLowSurrogate(text.charCodeAt(index)) ? suffix - 1 : suffix;
}

function isLowSurrogate(unit: number): boolean {
  return unit >= 0xdc00 && unit <= 0xdfff;
}
