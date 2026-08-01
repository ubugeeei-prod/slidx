/**
 * Keyboard access to the visual editor.
 *
 * The shortcuts are paired with a real Session and a command-recording canvas.
 * This proves that deck changes reach the same transport and history boundary
 * as visible controls without connecting an iframe to a pretend dev server.
 */

import { describe, expect, it } from "vite-plus/test";

import type { CanvasSurface } from "../src/canvas";
import { createSession } from "../src/session";
import { createShortcuts, type ShortcutSurface } from "../src/shortcuts";
import { deckOf, fakeServer } from "./support";

interface Fixture {
  shortcuts: ShortcutSurface;
  session: ReturnType<typeof createSession>;
  server: ReturnType<typeof fakeServer>;
  modes: string[];
}

async function open(canEdit = true): Promise<Fixture> {
  const deck = deckOf("One", "Two", "Three");
  deck.access = { canEdit };
  const server = fakeServer(deck);
  const session = createSession(server);
  await session.open();

  const modes: string[] = [];
  let focused = false;
  const canvas: CanvasSurface = {
    root: document.createElement("section"),
    render() {},
    showMarkdown: () => modes.push("markdown"),
    showVisual: () => modes.push("visual"),
    focusText: () => modes.push("text"),
    finishTextSelection: () => modes.push("finish-text"),
    focusNotes: () => modes.push("notes"),
    focusFresh: () => modes.push("fresh"),
    addContent: () => modes.push("add"),
    insertContent: (kind) => modes.push(`insert:${kind}`),
    scheme: () => "light",
    palette: () => "light",
    zoom: () => 1,
    zoomIn: () => modes.push("zoom-in"),
    zoomOut: () => modes.push("zoom-out"),
    zoomFit: () => modes.push("zoom-fit"),
    listen() {},
    listenClipboard() {},
  };

  const addSlide = () => {
    const before = session.state().slides.length;
    const at = Math.min(session.state().selection.slide + 1, before);
    void session.run({ op: "createSlide", at, kind: "title-body" }).then(() => {
      if (session.state().slides.length <= before) return;
      session.select({ slide: at, block: undefined, range: undefined, text: undefined });
      canvas.focusFresh();
    });
  };

  return {
    shortcuts: createShortcuts(session, canvas, {
      addSlide,
      focusCanvas: () => {
        focused = !focused;
        modes.push(focused ? "focus" : "restore");
      },
      canvasFocused: () => focused,
      present: () => modes.push("present"),
    }),
    session,
    server,
    modes,
  };
}

const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

function key(
  shortcuts: ShortcutSurface,
  value: string,
  options: KeyboardEventInit = {},
  target: Document | Element = document,
): KeyboardEvent {
  const event = new KeyboardEvent("keydown", {
    key: value,
    bubbles: true,
    cancelable: true,
    view: window,
    ...options,
  });
  target.addEventListener("keydown", (sent) => shortcuts.keydown(sent as KeyboardEvent), {
    once: true,
  });
  target.dispatchEvent(event);
  return event;
}

function clipboard(
  shortcuts: ShortcutSurface,
  type: "copy" | "paste",
  data: DataTransfer = new DataTransfer(),
  target: Document | Element = document,
): ClipboardEvent {
  const event = new ClipboardEvent(type, {
    bubbles: true,
    cancelable: true,
    clipboardData: data,
  });
  target.addEventListener(type, (sent) => shortcuts[type](sent as ClipboardEvent), { once: true });
  target.dispatchEvent(event);
  return event;
}

function outlineButton(): HTMLButtonElement {
  const outline = document.createElement("section");
  outline.className = "slidx-outline";
  const button = document.createElement("button");
  outline.append(button);
  return button;
}

describe("visual editor shortcuts", () => {
  it("duplicates the selected slide through the shared edit operation", async () => {
    const fixture = await open();
    fixture.session.select({ slide: 1 });

    const event = key(fixture.shortcuts, "d", { metaKey: true });
    await settled();

    expect(event.defaultPrevented).toBe(true);
    expect(fixture.server.ops).toEqual([{ op: "duplicateSlide", slide: 1 }]);
  });

  it("duplicates the selected block instead, when one is selected", async () => {
    // One binding rather than two: "make another one of this" is one
    // intention, and the selection already says what "this" is.
    const fixture = await open();
    fixture.session.select({ slide: 1, block: 2 });

    key(fixture.shortcuts, "d", { metaKey: true });
    await settled();

    expect(fixture.server.ops).toEqual([{ op: "duplicateBlock", slide: 1, block: 2 }]);
  });

  it("duplicates the slide again once the block is deselected", async () => {
    const fixture = await open();
    fixture.session.select({ slide: 1, block: 2 });
    fixture.session.select({ block: undefined });

    key(fixture.shortcuts, "d", { metaKey: true });
    await settled();

    expect(fixture.server.ops).toEqual([{ op: "duplicateSlide", slide: 1 }]);
  });

  it("copies one slide as useful Markdown and pastes it after the selected slide", async () => {
    const fixture = await open();
    fixture.session.select({ slide: 1 });
    const data = new DataTransfer();

    const copied = clipboard(fixture.shortcuts, "copy", data);
    fixture.session.select({ slide: 2 });
    fixture.server.answer = deckOf("One", "Two", "Three", "Two copy");
    const pasted = clipboard(fixture.shortcuts, "paste", data);
    await settled();

    expect(copied.defaultPrevented).toBe(true);
    expect(data.getData("text/plain")).toBe("# Two");
    expect(JSON.parse(data.getData("application/x-slidx-slide"))).toEqual({
      version: 1,
      id: "two",
    });
    expect(pasted.defaultPrevented).toBe(true);
    expect(fixture.server.ops).toEqual([{ op: "duplicateSlide", slide: 1, after: 2 }]);
    expect(fixture.session.state().selection).toEqual({ slide: 3 });
  });

  it("leaves ordinary clipboard work alone in fields and for selected blocks or text", async () => {
    const fixture = await open();
    const source = document.createElement("textarea");
    const frame = document.createElement("iframe");
    document.body.append(frame);
    const editable = frame.contentDocument!.createElement("p");
    editable.contentEditable = "plaintext-only";
    frame.contentDocument!.body.append(editable);
    const slide = new DataTransfer();
    slide.setData("application/x-slidx-slide", JSON.stringify({ version: 1, id: "two" }));

    const fieldCopy = clipboard(fixture.shortcuts, "copy", new DataTransfer(), source);
    const fieldPaste = clipboard(fixture.shortcuts, "paste", slide, source);
    const editableCopy = clipboard(fixture.shortcuts, "copy", new DataTransfer(), editable);
    const editablePaste = clipboard(fixture.shortcuts, "paste", slide, editable);

    const selected = document.createElement("p");
    selected.textContent = "ordinary page selection";
    document.body.append(selected);
    const range = document.createRange();
    range.selectNodeContents(selected);
    document.getSelection()!.addRange(range);
    const pageCopy = clipboard(fixture.shortcuts, "copy");
    const pagePaste = clipboard(fixture.shortcuts, "paste", slide);
    document.getSelection()!.removeAllRanges();

    fixture.session.select({ block: 1 });
    const blockCopy = clipboard(fixture.shortcuts, "copy");
    const blockPaste = clipboard(fixture.shortcuts, "paste", slide);
    fixture.session.select({ block: undefined, range: { start: 0, end: 3 }, text: "Two" });
    const textCopy = clipboard(fixture.shortcuts, "copy");
    const textPaste = clipboard(fixture.shortcuts, "paste", slide);
    await settled();
    frame.remove();
    selected.remove();

    expect(
      [
        fieldCopy,
        fieldPaste,
        editableCopy,
        editablePaste,
        pageCopy,
        pagePaste,
        blockCopy,
        blockPaste,
        textCopy,
        textPaste,
      ].map((event) => event.defaultPrevented),
    ).toEqual([false, false, false, false, false, false, false, false, false, false]);
    expect(fixture.server.ops).toEqual([]);
  });

  it("does not claim malformed or unrelated clipboard content", async () => {
    const fixture = await open();
    const unrelated = new DataTransfer();
    unrelated.setData("text/plain", "words from another application");
    const malformed = new DataTransfer();
    malformed.setData("application/x-slidx-slide", "not json");
    const missing = new DataTransfer();
    missing.setData(
      "application/x-slidx-slide",
      JSON.stringify({ version: 1, id: "no-longer-in-this-deck" }),
    );

    const first = clipboard(fixture.shortcuts, "paste", unrelated);
    const second = clipboard(fixture.shortcuts, "paste", malformed);
    const third = clipboard(fixture.shortcuts, "paste", missing);
    await settled();

    expect(first.defaultPrevented).toBe(false);
    expect(second.defaultPrevented).toBe(false);
    expect(third.defaultPrevented).toBe(false);
    expect(fixture.server.ops).toEqual([]);
  });

  it("removes a selected block without requiring outline focus", async () => {
    const fixture = await open();
    fixture.session.select({ slide: 1, block: 2 });

    const event = key(fixture.shortcuts, "Delete");
    await settled();

    expect(event.defaultPrevented).toBe(true);
    expect(fixture.server.ops).toEqual([{ op: "removeBlock", slide: 1, block: 2 }]);
    expect(fixture.session.state().selection.block).toBeUndefined();
  });

  it("adds after the selected slide through the shared edit operation", async () => {
    const fixture = await open();
    fixture.session.select({ slide: 0 });
    fixture.server.answer = deckOf("One", "New slide", "Two", "Three");

    key(fixture.shortcuts, "m", { ctrlKey: true });
    await settled();

    expect(fixture.server.ops).toEqual([{ op: "createSlide", at: 1, kind: "title-body" }]);
    expect(fixture.session.state().selection.slide).toBe(1);
    expect(fixture.modes).toEqual(["fresh"]);
  });

  it("redoes with Ctrl+Y without replacing the macOS redo binding", async () => {
    const fixture = await open();
    await fixture.session.run({ op: "setHeading", slide: 0, text: "Retitled" });
    await fixture.session.undo();

    const mac = key(fixture.shortcuts, "y", { metaKey: true });
    const windows = key(fixture.shortcuts, "y", { ctrlKey: true });
    await settled();

    expect(mac.defaultPrevented).toBe(false);
    expect(windows.defaultPrevented).toBe(true);
    expect(fixture.server.reverted).toEqual([[{ splice: 1 }], [{ splice: -1 }]]);
    expect(fixture.session.state().canRedo).toBe(false);
  });

  it("jumps to the first and last slides with Home and End", async () => {
    const fixture = await open();
    fixture.session.select({ slide: 1, block: 2 });

    const first = key(fixture.shortcuts, "Home");
    expect(fixture.session.state().selection).toEqual({ slide: 0 });

    const last = key(fixture.shortcuts, "End");
    expect(fixture.session.state().selection).toEqual({ slide: 2 });
    expect(first.defaultPrevented).toBe(true);
    expect(last.defaultPrevented).toBe(true);
    expect(fixture.server.ops).toEqual([]);
  });

  it("navigates globally but only moves or removes a slide from the focused outline", async () => {
    const fixture = await open();

    key(fixture.shortcuts, "PageDown");
    expect(fixture.session.state().selection.slide).toBe(1);

    // A destructive key with no outline target belongs to the page, not slidx.
    key(fixture.shortcuts, "Delete");
    expect(fixture.server.ops).toEqual([]);

    const selected = outlineButton();
    key(fixture.shortcuts, "ArrowDown", { altKey: true }, selected);
    key(fixture.shortcuts, "Backspace", {}, selected);
    await settled();

    expect(fixture.server.ops).toEqual([
      { op: "moveSlide", slide: 1, to: 2 },
      { op: "removeSlide", slide: 1 },
    ]);
  });

  it("uses A, V, T, N, M, F, and P to address explicit editor commands without writing", async () => {
    const fixture = await open();

    key(fixture.shortcuts, "m");
    key(fixture.shortcuts, "v");
    key(fixture.shortcuts, "t");
    key(fixture.shortcuts, "n");
    key(fixture.shortcuts, "a");
    key(fixture.shortcuts, "f");
    key(fixture.shortcuts, "p");

    expect(fixture.modes).toEqual([
      "markdown",
      "visual",
      "text",
      "notes",
      "add",
      "focus",
      "present",
    ]);
    expect(fixture.server.ops).toEqual([]);
  });

  it("zooms the canvas without changing the browser or the deck", async () => {
    const fixture = await open();

    const out = key(fixture.shortcuts, "-", { metaKey: true });
    const into = key(fixture.shortcuts, "=", { metaKey: true });
    const plus = key(fixture.shortcuts, "+", { ctrlKey: true });
    const fit = key(fixture.shortcuts, "0", { metaKey: true });

    expect([out, into, plus, fit].every((event) => event.defaultPrevented)).toBe(true);
    expect(fixture.modes).toEqual(["zoom-out", "zoom-in", "zoom-in", "zoom-fit"]);
    expect(fixture.server.ops).toEqual([]);
  });

  it("turns the standard bold key into a source-backed text style", async () => {
    const fixture = await open();
    fixture.session.select({
      slide: 1,
      text: "Two",
      range: { start: 2, end: 5 },
    });
    const editable = document.createElement("p");
    editable.contentEditable = "true";

    const event = key(fixture.shortcuts, "b", { metaKey: true }, editable);
    await settled();

    expect(event.defaultPrevented).toBe(true);
    expect(fixture.server.ops).toEqual([
      {
        op: "addMark",
        slide: 1,
        range: { start: 2, end: 5 },
        attributes: { key: undefined, classes: [], properties: { weight: "bold" } },
      },
    ]);
  });

  it("blocks a native-only bold mutation when the selection is not addressable", async () => {
    const fixture = await open();
    const editable = document.createElement("p");
    editable.contentEditable = "true";

    const event = key(fixture.shortcuts, "b", { ctrlKey: true }, editable);
    await settled();

    expect(event.defaultPrevented).toBe(true);
    expect(fixture.server.ops).toEqual([]);
  });

  it("finishes contextual text styling with Escape and keeps its block selected", async () => {
    const fixture = await open();
    fixture.session.select({
      slide: 1,
      block: 0,
      text: "Two",
      range: { start: 2, end: 5 },
    });
    const editable = document.createElement("p");
    editable.contentEditable = "true";

    const event = key(fixture.shortcuts, "Escape", {}, editable);

    expect(event.defaultPrevented).toBe(true);
    expect(fixture.modes).toEqual(["finish-text"]);
    expect(fixture.session.state().selection).toMatchObject({
      slide: 1,
      block: 0,
      text: undefined,
      range: undefined,
    });
  });

  it("keeps review navigation and delivery keys while ignoring authoring keys", async () => {
    const fixture = await open(false);

    key(fixture.shortcuts, "d", { metaKey: true });
    key(fixture.shortcuts, "m", { ctrlKey: true });
    key(fixture.shortcuts, "t");
    key(fixture.shortcuts, "a");
    key(fixture.shortcuts, "PageDown");
    key(fixture.shortcuts, "n");
    key(fixture.shortcuts, "m");
    key(fixture.shortcuts, "p");
    await settled();

    expect(fixture.server.ops).toEqual([]);
    expect(fixture.session.state().selection.slide).toBe(1);
    expect(fixture.modes).toEqual(["notes", "markdown", "present"]);
  });

  it("restores the workspace with Escape while the canvas is focused", async () => {
    const fixture = await open();

    key(fixture.shortcuts, "f");
    const event = key(fixture.shortcuts, "Escape");

    expect(event.defaultPrevented).toBe(true);
    expect(fixture.modes).toEqual(["focus", "restore"]);
  });

  it("shows a complete reference with question mark and closes it with Escape", async () => {
    const fixture = await open();
    const dialog = fixture.shortcuts.root.querySelector<HTMLElement>(".slidx-shortcuts-dialog")!;

    const event = key(fixture.shortcuts, "?", { shiftKey: true });

    expect(event.defaultPrevented).toBe(true);
    expect(dialog.hidden).toBe(false);
    expect(
      [...dialog.querySelectorAll(".slidx-shortcut-group h3")].map((heading) =>
        heading.textContent?.trim(),
      ),
    ).toEqual(["Navigation", "Editing", "Slides", "View"]);
    expect(dialog.textContent).toContain("Copy the selected slide");
    expect(dialog.textContent).toContain("Paste the copied slide");
    expect(dialog.textContent).toContain("Duplicate the selected block, or the slide");
    expect(dialog.textContent).toContain("First / last slide");
    expect(dialog.textContent).toContain("Search commands and slides");
    expect(dialog.textContent).toContain("Remove the selected block, or the focused slide");
    expect(dialog.textContent).toContain("Add content");
    expect(dialog.textContent).toContain("Edit slide text");
    expect(dialog.textContent).toContain("Toggle bold on selected words");
    expect(dialog.textContent).toContain("Write speaker notes");
    expect(dialog.textContent).toContain("Focus canvas / restore workspace");

    key(fixture.shortcuts, "Escape");
    expect(dialog.hidden).toBe(true);
  });

  it("never steals a command from fields, contenteditable text, or an IME", async () => {
    const fixture = await open();

    const source = document.createElement("textarea");
    key(fixture.shortcuts, "d", { metaKey: true }, source);
    fixture.session.select({ slide: 1 });
    const home = key(fixture.shortcuts, "Home", {}, source);

    const editable = document.createElement("p");
    editable.contentEditable = "true";
    key(fixture.shortcuts, "m", {}, editable);

    fixture.session.select({ slide: 1, text: "Two", range: { start: 2, end: 5 } });
    const input = document.createElement("input");
    const bold = key(fixture.shortcuts, "b", { metaKey: true }, input);

    key(fixture.shortcuts, "d", { metaKey: true, isComposing: true });
    await settled();

    expect(fixture.server.ops).toEqual([]);
    expect(fixture.modes).toEqual([]);
    expect(fixture.session.state().selection.slide).toBe(1);
    expect(home.defaultPrevented).toBe(false);
    expect(bold.defaultPrevented).toBe(false);
  });

  it("does not repeat an editing operation while a key is held", async () => {
    const fixture = await open();

    key(fixture.shortcuts, "d", { metaKey: true, repeat: true });
    await settled();

    expect(fixture.server.ops).toEqual([]);
  });

  it("does not repeat a key a focused control already handled", async () => {
    const fixture = await open();
    const event = new KeyboardEvent("keydown", {
      key: "ArrowRight",
      bubbles: true,
      cancelable: true,
    });
    event.preventDefault();

    fixture.shortcuts.keydown(event);

    expect(fixture.session.state().selection.slide).toBe(0);
  });
});

describe("arrows, which mean one of two things", () => {
  /** A fixture whose arrange surface reports what it was asked to move. */
  async function withNudge(moves: boolean) {
    const server = fakeServer();
    const session = createSession(server);
    await session.open();

    const asked: { block: number; key: string }[] = [];
    const canvas: CanvasSurface = {
      root: document.createElement("section"),
      render() {},
      showMarkdown() {},
      showVisual() {},
      focusText() {},
      finishTextSelection() {},
      focusNotes() {},
      focusFresh() {},
      addContent() {},
      insertContent() {},
      scheme: () => "light",
      palette: () => "light",
      zoom: () => 1,
      zoomIn() {},
      zoomOut() {},
      zoomFit() {},
      listen() {},
      listenClipboard() {},
    };

    const shortcuts = createShortcuts(session, canvas, {
      addSlide: () => {},
      focusCanvas: () => {},
      canvasFocused: () => false,
      present: () => {},
      nudge: (block, key) => {
        asked.push({ block, key });
        return moves;
      },
    });

    return { session, shortcuts, asked, server };
  }

  it("pages the deck when nothing is selected", async () => {
    const fixture = await withNudge(true);
    fixture.session.select({ slide: 0, block: undefined });

    const event = key(fixture.shortcuts, "ArrowRight");

    expect(event.defaultPrevented).toBe(true);
    expect(fixture.session.state().selection.slide).toBe(1);
    expect(fixture.asked).toEqual([]);
  });

  it("moves the block when one is selected, rather than paging out from under it", async () => {
    const fixture = await withNudge(true);
    fixture.session.select({ slide: 1, block: 2 });

    const event = key(fixture.shortcuts, "ArrowRight");

    expect(event.defaultPrevented).toBe(true);
    expect(fixture.session.state().selection.slide).toBe(1);
    expect(fixture.asked).toEqual([{ block: 2, key: "ArrowRight" }]);
  });

  it("moves it up and down its region too, which paging never did", async () => {
    const fixture = await withNudge(true);
    fixture.session.select({ slide: 0, block: 1 });

    key(fixture.shortcuts, "ArrowUp");
    key(fixture.shortcuts, "ArrowDown");

    expect(fixture.asked.map(({ key: pressed }) => pressed)).toEqual(["ArrowUp", "ArrowDown"]);
  });

  it("leaves the key alone when there is nothing to measure", async () => {
    // The Markdown view is up, or the canvas has not loaded. Swallowing the key
    // there would leave an author pressing a dead arrow with no way to tell why.
    const fixture = await withNudge(false);
    fixture.session.select({ slide: 0, block: 1 });

    const event = key(fixture.shortcuts, "ArrowUp");

    expect(event.defaultPrevented).toBe(false);
  });

  it("never steals an arrow from the outline, which has its own", async () => {
    const fixture = await withNudge(true);
    fixture.session.select({ slide: 0, block: 1 });

    key(fixture.shortcuts, "ArrowRight", {}, outlineButton());

    expect(fixture.asked).toEqual([]);
  });

  it("never steals one from a caret in text", async () => {
    const fixture = await withNudge(true);
    fixture.session.select({ slide: 0, block: 1 });

    key(fixture.shortcuts, "ArrowLeft", {}, document.createElement("textarea"));

    expect(fixture.asked).toEqual([]);
  });
});
