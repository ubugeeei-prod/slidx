/** The top-level commands and the state of the file they act on. */

import { afterEach, describe, expect, it } from "vite-plus/test";

import { createAppbar } from "../src/appbar";
import type { EditorState } from "../src/session";
import { deckOf } from "./support";

const surfaces: ReturnType<typeof createAppbar>[] = [];

afterEach(() => {
  for (const surface of surfaces.splice(0)) surface.destroy?.();
  document.body.replaceChildren();
});

function state(over: Partial<EditorState> = {}): EditorState {
  const opened = deckOf("One", "Two");
  return {
    source: opened.source,
    spans: opened.spans,
    title: opened.deck.title,
    slides: opened.deck.slides,
    layouts: opened.deck.layouts,
    activeTheme: opened.deck.activeTheme,
    themeLocked: opened.deck.themeLocked,
    themes: opened.deck.themes,
    transitions: opened.deck.transitions,
    durationSeconds: opened.deck.durationSeconds,
    diagnostics: [],
    selection: { slide: 0 },
    viewers: [],
    canUndo: false,
    canRedo: false,
    writing: false,
    ...over,
  };
}

function open() {
  const actions: string[] = [];
  const surface = createAppbar({
    undo: () => actions.push("undo"),
    redo: () => actions.push("redo"),
    present: () => actions.push("present"),
    audience: () => actions.push("audience"),
    print: () => actions.push("print"),
  });
  surfaces.push(surface);
  return { surface, root: surface.root, actions };
}

describe("the deck command bar", () => {
  it("says it is opening before it has called an unread deck saved", () => {
    const opened = open();
    opened.surface.render(state({ source: "", spans: [], slides: [] }));

    expect(opened.root.querySelector(".slidx-appbar-status")!.textContent).toBe("Opening…");
  });

  it("lets a read failure replace the opening state", () => {
    const opened = open();
    opened.surface.render(
      state({ source: "", spans: [], slides: [], problem: "The deck could not be read." }),
    );

    expect(opened.root.querySelector(".slidx-appbar-status")!.textContent).toBe("Open failed");
  });

  it("keeps presentation direct and gathers the other finished-deck surfaces", () => {
    const opened = open();
    document.body.append(opened.root);
    opened.surface.render(state({ canUndo: true, canRedo: true }));

    opened.root.querySelector<HTMLButtonElement>('[aria-label="Undo"]')!.click();
    opened.root.querySelector<HTMLButtonElement>('[aria-label="Redo"]')!.click();
    opened.root.querySelector<HTMLButtonElement>('[aria-label="Open presenter view"]')!.click();
    const toggle = opened.root.querySelector<HTMLButtonElement>(
      '[aria-label="More presentation outputs"]',
    )!;
    toggle.click();
    opened.root.querySelector<HTMLButtonElement>('[role="menuitem"]')!.click();
    toggle.click();
    opened.root.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')[1]!.click();

    expect(opened.actions).toEqual(["undo", "redo", "present", "audience", "print"]);
    expect(opened.root.querySelector(".slidx-appbar-status")!.textContent).toBe("Saved");
    expect(document.title).toBe("A Deck — editor");
  });

  it("drives the delivery menu with arrows and restores focus on Escape", () => {
    const opened = open();
    document.body.append(opened.root);
    opened.surface.render(state());
    const toggle = opened.root.querySelector<HTMLButtonElement>(
      '[aria-label="More presentation outputs"]',
    )!;
    const menu = opened.root.querySelector<HTMLElement>('[role="menu"]')!;
    const items = [...menu.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];

    toggle.click();
    expect(menu.hidden).toBe(false);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(document.activeElement).toBe(items[0]);

    items[0]!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    expect(document.activeElement).toBe(items[1]);
    items[1]!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));

    expect(menu.hidden).toBe(true);
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(toggle);
  });

  it("closes and disables delivery when there is no deck", () => {
    const opened = open();
    document.body.append(opened.root);
    opened.surface.render(state());
    const toggle = opened.root.querySelector<HTMLButtonElement>(
      '[aria-label="More presentation outputs"]',
    )!;
    toggle.click();

    opened.surface.render(state({ source: "", spans: [], slides: [] }));

    expect(toggle.disabled).toBe(true);
    expect(opened.root.querySelector<HTMLElement>('[role="menu"]')!.hidden).toBe(true);
  });

  it("locks history controls while a change is being written", () => {
    const opened = open();
    opened.surface.render(state({ canUndo: true, canRedo: true, writing: true }));

    expect(opened.root.querySelector(".slidx-appbar-status")!.textContent).toBe("Saving…");
    expect(opened.root.querySelector<HTMLButtonElement>('[aria-label="Undo"]')!.disabled).toBe(
      true,
    );
    expect(opened.root.querySelector<HTMLButtonElement>('[aria-label="Redo"]')!.disabled).toBe(
      true,
    );
  });

  it("names a view-only link before somebody tries to change it", () => {
    const opened = open();
    opened.surface.render(state({ canEdit: false, canUndo: true, canRedo: true }));

    const status = opened.root.querySelector<HTMLElement>(".slidx-appbar-status")!;
    expect(status.textContent).toBe("View only");
    expect(status.dataset.state).toBe("readonly");
    expect(status.title).toContain("cannot change");
    expect(opened.root.querySelector<HTMLButtonElement>('[aria-label="Undo"]')!.disabled).toBe(
      true,
    );
    expect(opened.root.querySelector<HTMLButtonElement>('[aria-label="Redo"]')!.disabled).toBe(
      true,
    );
  });

  it("distinguishes a refused operation from a write failure", () => {
    const opened = open();
    const status = opened.root.querySelector<HTMLElement>(".slidx-appbar-status")!;

    opened.surface.render(state({ refusal: { error: "noSuchBlock", block: 9 } }));
    expect(status.textContent).toBe("Not applied");
    expect(status.dataset.state).toBe("warning");

    opened.surface.render(state({ problem: "The deck could not be written." }));
    expect(status.textContent).toBe("Write stopped");
    expect(status.dataset.state).toBe("error");
    expect(status.title).toContain("could not be written");
  });
});
