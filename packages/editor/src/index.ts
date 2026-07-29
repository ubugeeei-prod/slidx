/**
 * `@slidx/editor` — the visual editor's state layer.
 *
 * The claim slidx makes is that the canvas and the Markdown file are two views
 * of one document. This package is where that is either kept or quietly
 * abandoned, so it holds one rule above all others: **the source is the
 * document**, and nothing in the store may disagree with it.
 *
 * Everything derived — the outline, the marks, the diagnostics — is recomputed
 * from the source. An edit is an operation applied to the source, performed by
 * the compiler, and the store records the result.
 */

export { createEditorStore, reducer } from "./store";
export type { EditorDispatch, EditorState, EditorStore } from "./store";

export {
  canRedo,
  canUndo,
  documentSlice,
  edited,
  opened,
  redone,
  reloaded,
  saved,
  undone,
} from "./slices/document";
export type { DocumentState } from "./slices/document";

export {
  blockSelected,
  cleared,
  hasRange,
  rangeSelected,
  selectionSlice,
  slideSelected,
} from "./slices/selection";
export type { SelectionState, TextRange } from "./slices/selection";

export {
  checked,
  checkStarted,
  diagnosticsSlice,
  findingsFor,
  hasBlocking,
} from "./slices/diagnostics";
export type { DiagnosticsState, Finding } from "./slices/diagnostics";
