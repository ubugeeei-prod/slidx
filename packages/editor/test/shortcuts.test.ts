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
import { fakeServer } from "./support";

interface Fixture {
  shortcuts: ShortcutSurface;
  session: ReturnType<typeof createSession>;
  server: ReturnType<typeof fakeServer>;
  modes: string[];
}

async function open(): Promise<Fixture> {
  const server = fakeServer();
  const session = createSession(server);
  await session.open();

  const modes: string[] = [];
  const canvas: CanvasSurface = {
    root: document.createElement("section"),
    render() {},
    showMarkdown: () => modes.push("markdown"),
    showVisual: () => modes.push("visual"),
    focusText: () => modes.push("text"),
    listen() {},
  };

  return {
    shortcuts: createShortcuts(session, canvas, { present: () => modes.push("present") }),
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

  it("adds after the selected slide through the shared edit operation", async () => {
    const fixture = await open();
    fixture.session.select({ slide: 0 });

    key(fixture.shortcuts, "m", { ctrlKey: true });
    await settled();

    expect(fixture.server.ops).toEqual([{ op: "insertSlide", at: 1, body: "## New slide" }]);
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

  it("uses V, T, M, and P to address explicit editor commands without writing the deck", async () => {
    const fixture = await open();

    key(fixture.shortcuts, "m");
    key(fixture.shortcuts, "v");
    key(fixture.shortcuts, "t");
    key(fixture.shortcuts, "p");

    expect(fixture.modes).toEqual(["markdown", "visual", "text", "present"]);
    expect(fixture.server.ops).toEqual([]);
  });

  it("shows a complete reference with question mark and closes it with Escape", async () => {
    const fixture = await open();
    const dialog = fixture.shortcuts.root.querySelector<HTMLElement>(".slidx-shortcuts-dialog")!;

    const event = key(fixture.shortcuts, "?", { shiftKey: true });

    expect(event.defaultPrevented).toBe(true);
    expect(dialog.hidden).toBe(false);
    expect(dialog.textContent).toContain("Duplicate slide");
    expect(dialog.textContent).toContain("Edit slide text");

    key(fixture.shortcuts, "Escape");
    expect(dialog.hidden).toBe(true);
  });

  it("never steals a command from fields, contenteditable text, or an IME", async () => {
    const fixture = await open();

    const source = document.createElement("textarea");
    key(fixture.shortcuts, "d", { metaKey: true }, source);

    const editable = document.createElement("p");
    editable.contentEditable = "true";
    key(fixture.shortcuts, "m", {}, editable);

    key(fixture.shortcuts, "d", { metaKey: true, isComposing: true });
    await settled();

    expect(fixture.server.ops).toEqual([]);
    expect(fixture.modes).toEqual([]);
  });

  it("does not repeat an editing operation while a key is held", async () => {
    const fixture = await open();

    key(fixture.shortcuts, "d", { metaKey: true, repeat: true });
    await settled();

    expect(fixture.server.ops).toEqual([]);
  });
});
