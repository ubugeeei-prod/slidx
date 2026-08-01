/** Searchable commands and slide navigation from one keyboard-first surface. */

import { afterEach, describe, expect, it } from "vite-plus/test";

import type { CanvasSurface } from "../src/canvas";
import { createCommandPalette, type CommandPalette } from "../src/command-palette";
import { createSession } from "../src/session";
import { deckOf, fakeServer } from "./support";

afterEach(() => document.body.replaceChildren());

async function open(titles = ["One", "Two", "Three"], canEdit = true) {
  const deck = deckOf(...titles);
  deck.access = { canEdit };
  const server = fakeServer(deck);
  const session = createSession(server);
  await session.open();
  const actions: string[] = [];
  const canvas: CanvasSurface = {
    root: document.createElement("section"),
    render() {},
    showMarkdown: () => actions.push("markdown"),
    showVisual: () => actions.push("visual"),
    focusText: () => actions.push("text"),
    finishTextSelection: () => actions.push("finish-text"),
    focusNotes: () => actions.push("notes"),
    focusFresh: () => actions.push("fresh"),
    addContent: () => actions.push("content"),
    insertContent: (kind) => actions.push(`insert:${kind}`),
    scheme: () => "light",
    palette: () => "light",
    zoom: () => 1,
    zoomIn: () => actions.push("zoom-in"),
    zoomOut: () => actions.push("zoom-out"),
    zoomFit: () => actions.push("zoom-fit"),
    listen() {},
    listenClipboard() {},
  };
  let focused = false;
  const palette = createCommandPalette(session, canvas, {
    addSlide: () => actions.push("slide"),
    createSlide: (kind) => actions.push(`new:${kind}`),
    focusCanvas: () => {
      focused = !focused;
      actions.push(focused ? "focus" : "restore");
    },
    canvasFocused: () => focused,
    present: () => actions.push("present"),
    audience: () => actions.push("audience"),
    print: () => actions.push("print"),
  });
  document.body.append(palette.trigger, palette.root);
  palette.render(session.state());
  return { palette, session, server, actions, focused: () => focused };
}

function key(
  palette: CommandPalette,
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
  target.addEventListener("keydown", (sent) => palette.keydown(sent as KeyboardEvent), {
    once: true,
  });
  target.dispatchEvent(event);
  return event;
}

function search(palette: CommandPalette, value: string): void {
  const input = palette.root.querySelector<HTMLInputElement>(".slidx-command-input")!;
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function selected(palette: CommandPalette): HTMLElement {
  return palette.root.querySelector<HTMLElement>('[aria-selected="true"]')!;
}

describe("the command palette", () => {
  it("opens from fields with the primary K command and focuses search", async () => {
    const { palette } = await open();
    const field = document.createElement("input");
    document.body.append(field);

    const event = key(palette, "k", { metaKey: true }, field);

    expect(event.defaultPrevented).toBe(true);
    expect(palette.root.hidden).toBe(false);
    expect(palette.trigger.getAttribute("aria-expanded")).toBe("true");
    expect(document.activeElement).toBe(
      palette.root.querySelector<HTMLInputElement>(".slidx-command-input"),
    );
  });

  it("shows actions and named slides without requiring a query", async () => {
    const { palette } = await open(["Opening", "A difficult middle", "Finish"]);
    palette.show();

    expect(palette.root.textContent).toContain("Edit slide text");
    expect(palette.root.textContent).toContain("A difficult middle");
    expect(palette.root.querySelectorAll('[role="option"]')).toHaveLength(14);
    expect(selected(palette).textContent).toContain("Add slide");
  });

  it("finds a slide by title and goes there", async () => {
    const { palette, session } = await open(["Opening", "日本語の要点", "Finish"]);
    palette.show();
    search(palette, "日本語");

    const result = palette.root.querySelector<HTMLButtonElement>('[role="option"]')!;
    expect(result.textContent).toContain("日本語の要点");
    result.click();

    expect(session.state().selection.slide).toBe(1);
    expect(palette.root.hidden).toBe(true);
  });

  it("normalizes full-width search text before matching", async () => {
    const { palette } = await open(["Opening", "Section 2", "Finish"]);
    palette.show();
    search(palette, "Ｓｅｃｔｉｏｎ　２");

    expect(palette.root.querySelectorAll('[role="option"]')).toHaveLength(1);
    expect(selected(palette).textContent).toContain("Section 2");
  });

  it("runs a filtered action with Enter", async () => {
    const { palette, actions } = await open();
    palette.show();
    search(palette, "markdown");

    const event = key(palette, "Enter");

    expect(event.defaultPrevented).toBe(true);
    expect(actions).toEqual(["markdown"]);
    expect(palette.root.hidden).toBe(true);
  });

  it("inserts a precise block from search without opening a second menu", async () => {
    const { palette, session, actions } = await open();
    session.select({ slide: 1, block: 0 });
    palette.render(session.state());
    palette.show();
    search(palette, "insert quote");

    expect(selected(palette).textContent).toContain("Insert quote");
    expect(selected(palette).textContent).toContain("after the selected block");
    expect(selected(palette).querySelector(".slidx-command-type")!.textContent).toBe("block");
    selected(palette).click();

    expect(actions).toEqual(["insert:quote"]);
    expect(palette.root.hidden).toBe(true);
  });

  it("creates a searched narrative starting point in one command", async () => {
    const { palette, actions } = await open();
    palette.show();
    search(palette, "comparison slide");

    expect(selected(palette).textContent).toContain("New comparison slide");
    expect(selected(palette).textContent).toContain("Two equal sides");
    expect(selected(palette).querySelector(".slidx-command-type")!.textContent).toBe("new");
    selected(palette).click();

    expect(actions).toEqual(["new:comparison"]);
  });

  it("leads with contextual text commands and previews semantic tones", async () => {
    const { palette, session } = await open();
    session.select({ slide: 1, text: "Two", range: { start: 2, end: 5 } });
    palette.render(session.state());
    palette.show();

    const rows = [...palette.root.querySelectorAll<HTMLElement>('[role="option"]')];
    expect(rows[0]!.textContent).toContain("Bold selected text");
    expect(rows[0]!.textContent).toContain("⌘/Ctrl B");
    expect(rows[0]!.querySelector(".slidx-command-type")!.textContent).toBe("text");
    expect(rows[1]!.textContent).toContain("Use mono typeface");
    expect(
      palette.root.querySelector('[data-command-tone="theme"]')!.getAttribute("aria-current"),
    ).toBe("true");
    expect(
      palette.root
        .querySelector('[data-command-tone="theme"]')!
        .querySelector(".slidx-command-type")!.textContent,
    ).toBe("Current");
    expect(palette.root.querySelector('[data-command-tone="accent"]')).not.toBeNull();
  });

  it("styles selected words through the same operation as the canvas controls", async () => {
    const { palette, session, server } = await open();
    session.select({ slide: 1, text: "Two", range: { start: 2, end: 5 } });
    palette.render(session.state());
    palette.show();
    search(palette, "accent text tone");

    selected(palette).click();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(server.ops).toEqual([
      {
        op: "addMark",
        slide: 1,
        range: { start: 2, end: 5 },
        attributes: { key: undefined, classes: ["accent"], properties: {} },
      },
    ]);
    expect(palette.root.hidden).toBe(true);
  });

  it("finishes text styling from search without losing the selected block", async () => {
    const { palette, session, actions } = await open();
    session.select({
      slide: 1,
      block: 0,
      text: "Two",
      range: { start: 2, end: 5 },
    });
    palette.render(session.state());
    palette.show();
    search(palette, "finish text styling");

    selected(palette).click();

    expect(actions).toContain("finish-text");
    expect(session.state().selection).toMatchObject({
      slide: 1,
      block: 0,
      text: undefined,
      range: undefined,
    });
  });

  it("opens the current slide's speaking surface as an action", async () => {
    const { palette, actions } = await open();
    palette.show();
    search(palette, "speaker notes");

    selected(palette).click();

    expect(actions).toEqual(["notes"]);
    expect(palette.root.hidden).toBe(true);
  });

  it("finds audience and printable outputs by the words people use for them", async () => {
    const { palette, actions } = await open();
    palette.show();
    search(palette, "projector");
    selected(palette).click();

    palette.show();
    search(palette, "PDF");
    selected(palette).click();

    expect(actions).toEqual(["audience", "print"]);
  });

  it("keeps review commands available and disables authoring for a view-only link", async () => {
    const { palette, session, server, actions } = await open(["One", "Two"], false);
    palette.show();
    search(palette, "add slide");

    expect(palette.root.querySelector('[role="option"]')!.getAttribute("aria-disabled")).toBe(
      "true",
    );

    search(palette, "speaker notes");
    expect(selected(palette).textContent).toContain("Read speaker notes");
    selected(palette).click();

    palette.show();
    search(palette, "PDF");
    selected(palette).click();
    expect(actions).toEqual(["notes", "print"]);

    session.select({ slide: 1, text: "Two", range: { start: 2, end: 5 } });
    palette.render(session.state());
    palette.show();
    search(palette, "bold selected text");
    const bold = palette.root.querySelector<HTMLButtonElement>('[role="option"]')!;
    expect(bold.getAttribute("aria-disabled")).toBe("true");
    bold.click();
    expect(server.ops).toEqual([]);

    palette.show();
    search(palette, "insert quote");
    const quote = palette.root.querySelector<HTMLButtonElement>('[role="option"]')!;
    expect(quote.getAttribute("aria-disabled")).toBe("true");
    quote.click();
    expect(actions).toEqual(["notes", "print"]);
  });

  it("names the reversible canvas focus action for the state it will enter", async () => {
    const { palette, actions, focused } = await open();
    palette.show();
    search(palette, "focus canvas");
    selected(palette).click();

    expect(focused()).toBe(true);
    expect(actions).toEqual(["focus"]);

    palette.show();
    search(palette, "restore workspace");
    expect(selected(palette).textContent).toContain("Restore workspace");
  });

  it("moves through enabled results without landing on unavailable history", async () => {
    const { palette } = await open();
    palette.show();

    expect(selected(palette).textContent).toContain("Add slide");
    key(palette, "ArrowDown");
    expect(selected(palette).textContent).toContain("Add content");
    key(palette, "ArrowUp");
    expect(selected(palette).textContent).toContain("Add slide");
  });

  it("shows a designed empty state for a query with no answer", async () => {
    const { palette } = await open();
    palette.show();
    search(palette, "nothing in this deck has this phrase");

    expect(palette.root.querySelectorAll('[role="option"]')).toHaveLength(0);
    expect(palette.root.querySelector<HTMLElement>(".slidx-command-empty")!.hidden).toBe(false);
    expect(palette.root.textContent).toContain("No matching action or slide");
  });

  it("closes with Escape and restores focus to its trigger", async () => {
    const { palette } = await open();
    palette.show();

    key(palette, "Escape");

    expect(palette.root.hidden).toBe(true);
    expect(palette.trigger.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(palette.trigger);
  });
});
