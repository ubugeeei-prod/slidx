/**
 * The store as a whole, and the boundary it must not cross.
 *
 * Redux Toolkit belongs to the editor, which is a dev-time application. An
 * audience slide ships no JavaScript at all — that is the property slidx is
 * sold on — so the last test here is a guard rather than a unit test: it fails
 * if the runtime ever gains this dependency by accident.
 */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

import {
  blockSelected,
  checked,
  createEditorStore,
  edited,
  hasBlocking,
  hasRange,
  opened,
  rangeSelected,
  slideSelected,
} from "../src/index";

describe("the store", () => {
  it("starts with an empty document, no selection, and no findings", () => {
    const state = createEditorStore().getState();

    expect(state.document.source).toBe("");
    expect(state.selection.block).toBeNull();
    expect(state.diagnostics.findings).toEqual([]);
  });

  it("keeps selection out of the document's history", () => {
    // Clicking a block is not an edit, and undoing an edit should not move the
    // cursor somewhere the person did not put it.
    const store = createEditorStore();
    store.dispatch(opened({ source: "a", path: null }));
    store.dispatch(blockSelected(2));

    expect(store.getState().document.past).toEqual([]);
  });

  it("does not touch the selection when the document changes", () => {
    const store = createEditorStore();
    store.dispatch(blockSelected(3));
    store.dispatch(edited("changed"));

    expect(store.getState().selection.block).toBe(3);
  });
});

describe("selection", () => {
  it("drops a block index when the slide changes", () => {
    // A block index is scoped to one slide; keeping it would point at whatever
    // happened to be there.
    const store = createEditorStore();
    store.dispatch(blockSelected(4));
    store.dispatch(slideSelected(2));

    expect(store.getState().selection.block).toBeNull();
  });

  it("refuses a range with no block to be a range of", () => {
    const store = createEditorStore();
    store.dispatch(rangeSelected({ start: 0, end: 4 }));

    expect(store.getState().selection.range).toBeNull();
  });

  it("reports a range that could carry a mark", () => {
    const store = createEditorStore();
    store.dispatch(blockSelected(0));
    store.dispatch(rangeSelected({ start: 2, end: 8 }));

    expect(hasRange(store.getState().selection)).toBe(true);
  });

  it("does not count an empty range as a selection", () => {
    const store = createEditorStore();
    store.dispatch(blockSelected(0));
    store.dispatch(rangeSelected({ start: 3, end: 3 }));

    expect(hasRange(store.getState().selection)).toBe(false);
  });
});

describe("diagnostics", () => {
  it("replaces findings wholesale rather than merging them", () => {
    // A diagnostic lingering after the line it referred to was deleted is
    // worse than a moment with none: people stop trusting the panel.
    const store = createEditorStore();
    store.dispatch(checked([{ severity: "error", code: "a", message: "one" }]));
    store.dispatch(checked([{ severity: "warning", code: "b", message: "two" }]));

    expect(store.getState().diagnostics.findings).toHaveLength(1);
    expect(hasBlocking(store.getState().diagnostics)).toBe(false);
  });
});

describe("the boundary", () => {
  it("keeps Redux out of the runtime an audience loads", () => {
    // The property slidx is sold on. A dependency added here by habit would
    // erode it silently, so this reads the manifest rather than trusting it.
    const manifest = JSON.parse(
      readFileSync(resolve(process.cwd(), "packages/runtime/package.json"), "utf8"),
    ) as { dependencies?: Record<string, string> };

    expect(Object.keys(manifest.dependencies ?? {})).toEqual([]);
  });
});
