/**
 * What the editor is pointing at.
 *
 * Kept separate from the document because it changes far more often and means
 * nothing to a file: clicking a block is not an edit, and undoing an edit
 * should not move the cursor somewhere the person did not put it.
 *
 * A selection is *addresses*, never content. Holding a copy of the selected
 * text here would be a second representation of the document — the exact
 * failure the document slice exists to avoid.
 */

import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

/** A range inside a block, in byte offsets, as the compiler addresses it. */
export interface TextRange {
  start: number;
  end: number;
}

export interface SelectionState {
  slide: number;
  /** Index of the top-level block within the slide, if one is selected. */
  block: number | null;
  /** Range within that block, if the person selected text rather than a block. */
  range: TextRange | null;
}

const initialState: SelectionState = { slide: 0, block: null, range: null };

export const selectionSlice = createSlice({
  name: "selection",
  initialState,
  reducers: {
    slideSelected(state, action: PayloadAction<number>) {
      // Moving to another slide invalidates a block index, which is scoped to
      // one slide. Keeping it would point at whatever happened to be there.
      if (action.payload !== state.slide) {
        state.block = null;
        state.range = null;
      }
      state.slide = Math.max(0, action.payload);
    },

    blockSelected(state, action: PayloadAction<number | null>) {
      state.block = action.payload;
      state.range = null;
    },

    rangeSelected(state, action: PayloadAction<TextRange | null>) {
      // A range with no block has nothing to be a range *of*.
      state.range = state.block === null ? null : action.payload;
    },

    cleared(state) {
      state.block = null;
      state.range = null;
    },
  },
});

export const { slideSelected, blockSelected, rangeSelected, cleared } = selectionSlice.actions;

/** True when a range is selected and could carry a mark. */
export function hasRange(state: SelectionState): boolean {
  return state.block !== null && state.range !== null && state.range.end > state.range.start;
}
