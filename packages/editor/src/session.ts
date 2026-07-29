/**
 * The editor's one piece of state, and the only thing that changes it.
 *
 * Every surface reads this and none of them writes a deck: a click on the
 * outline, a committed heading, a class typed into the inspector all become an
 * [`EditOp`](./operations) handed to `run`. That keeps the number of things
 * that can disagree with the file at one.
 *
 * Deliberately not a framework. slidx is framework-agnostic and every
 * integration is opt-in and removable; an editor built on Vue or React would
 * make that claim false in the one place an author looks at all day. What a
 * store is actually for here is subscription, which is thirty lines.
 */

import { sliceBytes } from "./bytes";
import type { DeckState, EditorClient, Finding, SlideSpans, SlideSummary } from "./client";
import { createHistory, type History } from "./history";
import type { EditOp, EditRefusal } from "./operations";

/** What is selected, which is what the inspector is about. */
/**
 * Where the author is.
 *
 * The optional fields are written `| undefined` on purpose. These are patched
 * rather than replaced, and moving to another slide *clears* the range — so a
 * caller has to be able to say "this field is now nothing", which is a
 * different statement from leaving the field out of the patch.
 */
export interface Selection {
  slide: number;
  /** A range of the current slide's source body, when text is selected. */
  range?: { start: number; end: number } | undefined;
  /** The text that range names, for showing back to the author. */
  text?: string | undefined;
}

export interface EditorState {
  source: string;
  spans: SlideSpans[];
  slides: SlideSummary[];
  diagnostics: Finding[];
  selection: Selection;
  canUndo: boolean;
  canRedo: boolean;
  /** What the last operation was refused for, cleared by the next one. */
  refusal?: EditRefusal | undefined;
  /** A message about something that stopped, rather than something refused. */
  problem?: string | undefined;
}

export interface Session {
  state(): EditorState;
  subscribe(listener: (state: EditorState) => void): () => void;
  /** Reads the deck as it is on disk. */
  open(): Promise<void>;
  run(op: EditOp): Promise<void>;
  undo(): Promise<void>;
  redo(): Promise<void>;
  select(selection: Partial<Selection>): void;
  /** The Markdown of one slide's body, as the author wrote it. */
  bodyOf(slide: number): string;
}

const EMPTY: EditorState = {
  source: "",
  spans: [],
  slides: [],
  diagnostics: [],
  selection: { slide: 0 },
  canUndo: false,
  canRedo: false,
};

export function createSession(client: EditorClient, history: History = createHistory()): Session {
  let state = EMPTY;
  const listeners = new Set<(state: EditorState) => void>();

  function set(change: Partial<EditorState>): void {
    state = { ...state, ...change };
    for (const listener of listeners) listener(state);
  }

  /** The deck part of the answer, with the selection kept in range. */
  function adopt(deck: DeckState, extra: Partial<EditorState> = {}): void {
    const slide = Math.min(state.selection.slide, Math.max(deck.deck.slides.length - 1, 0));

    set({
      source: deck.source,
      spans: deck.spans,
      slides: deck.deck.slides,
      diagnostics: deck.deck.diagnostics,
      // A selection is a range in a body that has just been rewritten, so it
      // cannot survive the edit that rewrote it. The slide it was on can.
      selection: { slide },
      canUndo: history.canUndo,
      canRedo: history.canRedo,
      refusal: undefined,
      problem: undefined,
      ...extra,
    });
  }

  async function attempt(work: () => Promise<void>): Promise<void> {
    try {
      await work();
    } catch (error) {
      set({ problem: error instanceof Error ? error.message : String(error) });
    }
  }

  return {
    state: () => state,

    subscribe(listener) {
      listeners.add(listener);
      listener(state);
      return () => listeners.delete(listener);
    },

    open() {
      return attempt(async () => {
        history.clear();
        adopt(await client.deck());
      });
    },

    run(op) {
      return attempt(async () => {
        const answer = await client.apply(op);
        if (answer.error) {
          set({ refusal: answer.error });
          return;
        }

        history.applied(answer.undo ?? []);
        adopt(answer);
      });
    },

    undo() {
      return attempt(async () => {
        const edit = history.nextUndo();
        if (!edit) return;

        const answer = await client.revert(edit);
        history.undone(answer.undo ?? []);
        adopt(answer);
      });
    },

    redo() {
      return attempt(async () => {
        const edit = history.nextRedo();
        if (!edit) return;

        const answer = await client.revert(edit);
        history.redone(answer.undo ?? []);
        adopt(answer);
      });
    },

    select(selection) {
      set({ selection: { ...state.selection, ...selection } });
    },

    bodyOf(slide) {
      const span = state.spans[slide]?.body;
      return span ? sliceBytes(state.source, span.start, span.end) : "";
    },
  };
}
