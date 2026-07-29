/**
 * The editor's store.
 *
 * Redux Toolkit is a real dependency of this package and of nothing else in
 * slidx. The editor is a dev-time application with genuinely interlocking
 * state — a canvas, an inspector, an outline, a history, live diagnostics —
 * which is what a store is for. An audience slide ships no JavaScript at all,
 * and a test in this package asserts that the runtime never gains this
 * dependency by accident.
 */

import { configureStore, type ConfigureStoreOptions } from "@reduxjs/toolkit";

import { diagnosticsSlice } from "./slices/diagnostics";
import { documentSlice } from "./slices/document";
import { selectionSlice } from "./slices/selection";

export const reducer = {
  document: documentSlice.reducer,
  selection: selectionSlice.reducer,
  diagnostics: diagnosticsSlice.reducer,
};

export function createEditorStore(options: Partial<ConfigureStoreOptions> = {}) {
  return configureStore({ reducer, ...options });
}

export type EditorStore = ReturnType<typeof createEditorStore>;
export type EditorState = ReturnType<EditorStore["getState"]>;
export type EditorDispatch = EditorStore["dispatch"];
