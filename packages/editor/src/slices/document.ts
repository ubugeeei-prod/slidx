/**
 * The document the editor is editing.
 *
 * # The one decision everything else follows from
 *
 * **The Markdown source is the document. The store holds it and nothing else
 * that could disagree with it.**
 *
 * A canvas editor is normally built the other way round: a rich model in
 * memory, serialised out on save. That is where every "the editor reordered my
 * frontmatter" and "my hand edit was lost" bug comes from — two representations
 * of one thing, drifting. slidx claims the canvas and the file are two *views*,
 * and this is where that claim is either kept or quietly abandoned.
 *
 * So the state is a string, plus the parse of that string. Everything derived
 * — the outline, the marks, the diagnostics — is recomputed from it and never
 * edited directly. An edit is an operation applied to the *source*.
 *
 * # Undo is a stack of sources, not a stack of inverses
 *
 * Inverse operations are the clever answer and the fragile one: every
 * operation needs a correct inverse, and one wrong inverse corrupts the
 * document in a way the user cannot see until much later. A deck is kilobytes,
 * so keeping whole snapshots costs nothing measurable and cannot drift.
 */

import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

/** How many edits back a person can go. */
const HISTORY_LIMIT = 100;

export interface DocumentState {
  /** The Markdown. This is the document; everything else is derived. */
  source: string;
  /** Where it came from, so a save knows what to write. */
  path: string | null;
  /** Sources before the current one, oldest first. */
  past: string[];
  /** Sources undone from, newest first. */
  future: string[];
  /** True when the source differs from what was last saved. */
  dirty: boolean;
}

const initialState: DocumentState = {
  source: "",
  path: null,
  past: [],
  future: [],
  dirty: false,
};

export const documentSlice = createSlice({
  name: "document",
  initialState,
  reducers: {
    /** Replaces everything, as when a file is opened. Clears the history. */
    opened(state, action: PayloadAction<{ source: string; path: string | null }>) {
      state.source = action.payload.source;
      state.path = action.payload.path;
      state.past = [];
      state.future = [];
      state.dirty = false;
    },

    /**
     * Records the result of an edit.
     *
     * Takes the *new source* rather than an operation: applying the operation
     * is the compiler's job, and a reducer that also applied edits would be a
     * second implementation of the edit semantics.
     */
    edited(state, action: PayloadAction<string>) {
      // An edit that changed nothing must not cost an undo step. Editors emit
      // these constantly — a drag that ends where it started, a re-render.
      if (action.payload === state.source) return;

      state.past.push(state.source);
      if (state.past.length > HISTORY_LIMIT) state.past.shift();

      state.source = action.payload;
      // A new edit makes the redo stack unreachable, and keeping it would let
      // a redo jump to a document that never existed.
      state.future = [];
      state.dirty = true;
    },

    undone(state) {
      const previous = state.past.pop();
      if (previous === undefined) return;

      state.future.unshift(state.source);
      state.source = previous;
      state.dirty = true;
    },

    redone(state) {
      const next = state.future.shift();
      if (next === undefined) return;

      state.past.push(state.source);
      state.source = next;
      state.dirty = true;
    },

    /** The source reached disk. History is kept; only cleanliness changes. */
    saved(state) {
      state.dirty = false;
    },

    /**
     * The file changed underneath the editor.
     *
     * Someone edited the Markdown in their text editor, which is a *supported*
     * way to use slidx rather than a conflict to resolve. The external version
     * wins and becomes an undo step, so nothing is lost either way.
     */
    reloaded(state, action: PayloadAction<string>) {
      if (action.payload === state.source) return;

      state.past.push(state.source);
      state.source = action.payload;
      state.future = [];
      state.dirty = false;
    },
  },
});

export const { opened, edited, undone, redone, saved, reloaded } = documentSlice.actions;

export const canUndo = (state: DocumentState): boolean => state.past.length > 0;
export const canRedo = (state: DocumentState): boolean => state.future.length > 0;
