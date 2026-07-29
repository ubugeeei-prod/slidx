/**
 * The document the editor is editing.
 *
 * This is the specification for the one rule the whole visual editor rests on:
 * **the Markdown source is the document**, and the store holds nothing that
 * could disagree with it.
 *
 * A canvas editor is normally built the other way round — a rich model in
 * memory, serialised on save — and that is where "the editor reordered my
 * frontmatter" and "my hand edit was lost" come from: two representations of
 * one thing, drifting. slidx claims the canvas and the file are two *views*,
 * so these tests guard the failures that would quietly end that claim.
 */

import { describe, expect, it } from "vitest";

import {
  canRedo,
  canUndo,
  createEditorStore,
  edited,
  opened,
  redone,
  reloaded,
  saved,
  undone,
} from "../src/index";

function store(source = "# One\n") {
  const store = createEditorStore();
  store.dispatch(opened({ source, path: "slides/0001.md" }));
  return store;
}

const doc = (s: ReturnType<typeof store>) => s.getState().document;

describe("holding the source", () => {
  it("holds the Markdown and nothing derived from it", () => {
    // Anything derived kept here is a second representation that can drift.
    const state = doc(store("# One\n"));

    expect(state.source).toBe("# One\n");
    expect(Object.keys(state).sort()).toEqual(["dirty", "future", "past", "path", "source"]);
  });

  it("opens clean", () => {
    expect(doc(store()).dirty).toBe(false);
  });

  it("is dirty after an edit and clean after a save", () => {
    const s = store();
    s.dispatch(edited("# Two\n"));
    expect(doc(s).dirty).toBe(true);

    s.dispatch(saved());
    expect(doc(s).dirty).toBe(false);
  });
});

describe("undo", () => {
  it("goes back to the previous source exactly", () => {
    const s = store("# One\n");
    s.dispatch(edited("# Two\n"));
    s.dispatch(undone());

    expect(doc(s).source).toBe("# One\n");
  });

  it("walks back through several edits", () => {
    const s = store("a");
    for (const next of ["b", "c", "d"]) s.dispatch(edited(next));

    s.dispatch(undone());
    s.dispatch(undone());

    expect(doc(s).source).toBe("b");
  });

  it("does nothing at the beginning", () => {
    const s = store("# One\n");
    s.dispatch(undone());

    expect(doc(s).source).toBe("# One\n");
    expect(canUndo(doc(s))).toBe(false);
  });

  it("redoes what it undid", () => {
    const s = store("a");
    s.dispatch(edited("b"));
    s.dispatch(undone());
    s.dispatch(redone());

    expect(doc(s).source).toBe("b");
  });

  it("drops the redo stack once a new edit lands", () => {
    // Keeping it would let a redo jump to a document that never existed.
    const s = store("a");
    s.dispatch(edited("b"));
    s.dispatch(undone());
    s.dispatch(edited("c"));

    expect(canRedo(doc(s))).toBe(false);
    s.dispatch(redone());
    expect(doc(s).source).toBe("c");
  });

  it("does not spend a step on an edit that changed nothing", () => {
    // Editors emit these constantly: a drag that ends where it started, a
    // re-render. Each one would otherwise cost a press of undo.
    const s = store("a");
    s.dispatch(edited("a"));

    expect(canUndo(doc(s))).toBe(false);
  });

  it("forgets the oldest edits rather than growing without bound", () => {
    const s = store("0");
    for (let i = 1; i <= 150; i += 1) s.dispatch(edited(String(i)));

    expect(doc(s).past.length).toBeLessThanOrEqual(100);
  });

  it("starts fresh when another file is opened", () => {
    // Undoing across a file boundary would write one deck's content into
    // another's file.
    const s = store("a");
    s.dispatch(edited("b"));
    s.dispatch(opened({ source: "# Other\n", path: "slides/0002.md" }));

    expect(canUndo(doc(s))).toBe(false);
  });
});

describe("someone editing the file in a text editor", () => {
  it("takes the external version", () => {
    // Hand-editing the Markdown is a supported way to use slidx, not a
    // conflict to resolve.
    const s = store("# One\n");
    s.dispatch(reloaded("# Edited by hand\n"));

    expect(doc(s).source).toBe("# Edited by hand\n");
  });

  it("keeps the editor's version reachable by undo", () => {
    const s = store("# One\n");
    s.dispatch(edited("# From the canvas\n"));
    s.dispatch(reloaded("# From the text editor\n"));
    s.dispatch(undone());

    expect(doc(s).source).toBe("# From the canvas\n");
  });

  it("is clean afterwards, because disk and memory agree", () => {
    const s = store("# One\n");
    s.dispatch(edited("# Changed\n"));
    s.dispatch(reloaded("# From disk\n"));

    expect(doc(s).dirty).toBe(false);
  });

  it("ignores a reload that brought the same bytes", () => {
    // A watcher fires on a save the editor itself performed.
    const s = store("# One\n");
    s.dispatch(reloaded("# One\n"));

    expect(canUndo(doc(s))).toBe(false);
  });
});
