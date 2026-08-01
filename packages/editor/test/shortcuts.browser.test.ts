/**
 * Keyboard and clipboard commands in an actual browser document.
 *
 * The DOM suite proves the command branches. This suite owns the boundary it
 * cannot emulate: focus moving into the preview iframe, native selection,
 * trusted keyboard input, and the ClipboardEvent/DataTransfer pair Chromium
 * supplies when a person copies or pastes.
 */

import { afterEach, describe, expect, it } from "vite-plus/test";
import { userEvent } from "vitest/browser";

import { createCanvas, type CanvasSurface } from "../src/canvas";
import type { EditOp } from "../src/operations";
import { createSession } from "../src/session";
import { createShortcuts, type ShortcutSurface } from "../src/shortcuts";
import { deckOf, fakeServer, type FakeServer } from "./support";

interface Fixture {
  canvas: CanvasSurface;
  dispose(): void;
  preview: Document;
  root: HTMLElement;
  server: FakeServer;
  session: ReturnType<typeof createSession>;
  shortcuts: ShortcutSurface;
}

interface SeenClipboard {
  dataTransfer: boolean;
  defaultPrevented: boolean;
  event: boolean;
  type: "copy" | "paste";
  types: string[];
}

let active: Fixture | undefined;

afterEach(() => {
  active?.dispose();
  active = undefined;
  document.getSelection()?.removeAllRanges();
});

async function open(): Promise<Fixture> {
  const server = fakeServer();
  const session = createSession(server);
  await session.open();

  const run = (op: EditOp) => void session.run(op);
  const canvas = createCanvas(
    {
      run,
      selected() {},
      selectedBlock(block) {
        session.select({ block, range: undefined, text: undefined });
      },
    },
    {
      deckBase: "slides",
      bodyOf: (slide) => session.bodyOf(slide),
      blocksOf: (slide) => session.blocksOf(slide),
    },
  );
  const shortcuts = createShortcuts(session, canvas, {
    addSlide() {
      const at = session.state().selection.slide + 1;
      void session.run({ op: "insertSlide", at, body: "## New slide" });
    },
    focusCanvas() {},
    canvasFocused: () => false,
    present() {},
  });
  const root = document.createElement("main");
  root.tabIndex = -1;
  root.append(canvas.root, shortcuts.root);
  document.body.append(root);

  const frame = canvas.root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
  const preview = frame.contentDocument;
  if (!preview) throw new Error("the browser did not create a preview document");

  const keydown = (event: KeyboardEvent) => shortcuts.keydown(event);
  const copy = (event: ClipboardEvent) => shortcuts.copy(event);
  const paste = (event: ClipboardEvent) => shortcuts.paste(event);
  document.addEventListener("keydown", keydown);
  document.addEventListener("copy", copy);
  document.addEventListener("paste", paste);
  canvas.listen(keydown);
  canvas.listenClipboard(copy, paste);

  active = {
    canvas,
    preview,
    root,
    server,
    session,
    shortcuts,
    dispose() {
      canvas.destroy?.();
      document.removeEventListener("keydown", keydown);
      document.removeEventListener("copy", copy);
      document.removeEventListener("paste", paste);
      root.remove();
    },
  };
  return active;
}

function watchClipboard(document: Document): SeenClipboard[] {
  const seen: SeenClipboard[] = [];
  const view = document.defaultView;
  if (!view) throw new Error("the preview has no window");

  const record = (event: ClipboardEvent) => {
    seen.push({
      type: event.type as "copy" | "paste",
      event: event instanceof view.ClipboardEvent,
      dataTransfer:
        event.clipboardData !== null && event.clipboardData instanceof view.DataTransfer,
      defaultPrevented: event.defaultPrevented,
      types: [...(event.clipboardData?.types ?? [])],
    });
  };
  document.addEventListener("copy", record);
  document.addEventListener("paste", record);
  return seen;
}

function selectContents(element: Node): void {
  const document = element.ownerDocument;
  const selection = document?.getSelection();
  if (!document || !selection) throw new Error("the browser did not expose a selection");

  const range = document.createRange();
  range.selectNodeContents(element);
  selection.removeAllRanges();
  selection.addRange(range);
}

async function primary(key: string): Promise<void> {
  const modifier = navigator.platform.startsWith("Mac") ? "Meta" : "Control";
  await userEvent.keyboard(`{${modifier}>}${key}{/${modifier}}`);
}

async function settled(accepts: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (accepts()) return;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw new Error("the browser command did not settle");
}

describe("editor commands in Browser Mode", () => {
  it("keeps native text selection copy and paste inside the visual preview", async () => {
    const fixture = await open();
    const source = fixture.preview.createElement("p");
    source.contentEditable = "true";
    source.textContent = "native visual selection";
    const target = fixture.preview.createElement("p");
    target.contentEditable = "true";
    fixture.preview.body.append(source, target);
    const seen = watchClipboard(fixture.preview);

    source.focus();
    selectContents(source);
    await userEvent.copy();
    target.focus();
    await userEvent.paste();
    await primary("d");
    await userEvent.keyboard("m");

    expect(target.textContent).toBe("native visual selectionm");
    expect(fixture.server.ops).toEqual([]);
    expect(seen).toHaveLength(2);
    expect(seen[0]).toMatchObject({
      type: "copy",
      event: true,
      dataTransfer: true,
      defaultPrevented: false,
    });
    // Chromium fills the system clipboard only after the unhandled copy event
    // has completed, so its DataTransfer is intentionally still empty here.
    expect(seen[0]!.types).toEqual([]);
    expect(seen[1]).toMatchObject({
      type: "paste",
      event: true,
      dataTransfer: true,
      defaultPrevented: false,
    });
    expect(seen[1]!.types).toContain("text/plain");
  });

  it("leaves clipboard and keyboard commands native while an input owns focus", async () => {
    const fixture = await open();
    fixture.session.select({ slide: 1 });
    const source = fixture.preview.createElement("input");
    source.value = "field text";
    const target = fixture.preview.createElement("input");
    fixture.preview.body.append(source, target);
    const seen = watchClipboard(fixture.preview);

    source.focus();
    source.select();
    await userEvent.copy();
    target.focus();
    await userEvent.paste();
    await primary("d");
    target.setSelectionRange(0, 0);
    await userEvent.keyboard("m");

    expect(target.value).toBe("mfield text");
    expect(fixture.session.state().selection.slide).toBe(1);
    expect(fixture.server.ops).toEqual([]);
    expect(
      seen.map(({ type, event, dataTransfer, defaultPrevented }) => ({
        type,
        event,
        dataTransfer,
        defaultPrevented,
      })),
    ).toEqual([
      { type: "copy", event: true, dataTransfer: true, defaultPrevented: false },
      { type: "paste", event: true, dataTransfer: true, defaultPrevented: false },
    ]);
  });

  it("copies a semantic slide through the browser clipboard and pastes it elsewhere", async () => {
    const fixture = await open();
    const focus = fixture.preview.createElement("button");
    focus.textContent = "Canvas";
    fixture.preview.body.append(focus);
    const seen = watchClipboard(fixture.preview);
    fixture.session.select({ slide: 1 });
    focus.focus();
    fixture.preview.getSelection()?.removeAllRanges();

    await userEvent.copy();
    fixture.session.select({ slide: 2 });
    fixture.server.answer = deckOf("One", "Two", "Three", "Two copy");
    focus.focus();
    await userEvent.paste();
    await settled(() => fixture.server.ops.length === 1);

    expect(fixture.server.ops).toEqual([{ op: "duplicateSlide", slide: 1, after: 2 }]);
    expect(fixture.session.state().selection).toEqual({ slide: 3 });
    expect(seen).toHaveLength(2);
    expect(seen[0]).toMatchObject({
      type: "copy",
      event: true,
      dataTransfer: true,
      defaultPrevented: true,
    });
    expect(seen[0]!.types).toContain("text/plain");
    expect(seen[0]!.types).toContain("application/x-slidx-slide");
    expect(seen[1]).toMatchObject({
      type: "paste",
      event: true,
      dataTransfer: true,
      defaultPrevented: true,
    });
    expect(seen[1]!.types).toContain("application/x-slidx-slide");
  });

  it("runs the major keyboard commands from the preview iframe", async () => {
    const fixture = await open();
    const focus = fixture.preview.createElement("button");
    focus.textContent = "Canvas";
    const editable = fixture.preview.createElement("p");
    editable.contentEditable = "true";
    editable.textContent = "Editable line";
    fixture.preview.body.append(focus, editable);
    const stage = fixture.canvas.root.querySelector<HTMLElement>(".slidx-canvas-stage")!;

    focus.focus();
    await userEvent.keyboard("{PageDown}");
    expect(fixture.session.state().selection.slide).toBe(1);
    focus.focus();
    await userEvent.keyboard("{End}");
    expect(fixture.session.state().selection.slide).toBe(2);
    focus.focus();
    await userEvent.keyboard("{Home}");
    expect(fixture.session.state().selection.slide).toBe(0);

    focus.focus();
    await primary("d");
    await settled(() => fixture.server.ops.length === 1);
    focus.focus();
    await primary("m");
    await settled(() => fixture.server.ops.length === 2);

    focus.focus();
    await userEvent.keyboard("m");
    expect(stage.dataset.editing).toBe("true");
    focus.focus();
    await userEvent.keyboard("v");
    expect(stage.dataset.editing).toBe("false");
    focus.focus();
    await userEvent.keyboard("t");
    expect(fixture.preview.activeElement).toBe(editable);

    focus.focus();
    await userEvent.keyboard("?");
    expect(
      fixture.shortcuts.root.querySelector<HTMLElement>(".slidx-shortcuts-dialog")!.hidden,
    ).toBe(false);
    expect(fixture.server.ops).toEqual([
      { op: "duplicateSlide", slide: 0 },
      { op: "insertSlide", at: 1, body: "## New slide" },
    ]);
  });
});
