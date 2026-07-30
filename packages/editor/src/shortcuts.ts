/**
 * Keyboard commands for the visual editor.
 *
 * These are deck operations, never DOM imitations of deck operations. Duplicate,
 * remove, move, and insert therefore pass through the same edit engine, history,
 * and collaboration transport as their visible controls.
 */

import type { CanvasSurface } from "./canvas";
import { element } from "./dom";
import type { Surface } from "./outline";
import type { Session } from "./session";

interface Shortcut {
  keys: string[];
  label: string;
}

const REFERENCE: Shortcut[] = [
  { keys: ["⌘/Ctrl", "Z"], label: "Undo" },
  { keys: ["⇧", "⌘/Ctrl", "Z"], label: "Redo" },
  { keys: ["⌘/Ctrl", "D"], label: "Duplicate slide" },
  { keys: ["⌘/Ctrl", "M"], label: "Add slide" },
  { keys: ["Page ↑", "Page ↓"], label: "Previous / next slide" },
  { keys: ["↑", "↓"], label: "Navigate the focused outline" },
  { keys: ["⌥/Alt", "↑", "↓"], label: "Move the focused slide" },
  { keys: ["⌫/Delete"], label: "Remove the focused slide" },
  { keys: ["V"], label: "Visual mode" },
  { keys: ["T"], label: "Edit slide text" },
  { keys: ["M"], label: "Markdown mode" },
  { keys: ["P"], label: "Open presenter view" },
  { keys: ["?"], label: "Keyboard shortcuts" },
  { keys: ["Esc"], label: "Close keyboard shortcuts" },
];

export interface ShortcutSurface extends Surface {
  keydown(event: KeyboardEvent): void;
}

export interface ShortcutActions {
  present(): void;
}

/** Creates the reference UI and the key listener that drives it. */
export function createShortcuts(
  session: Session,
  canvas: CanvasSurface,
  actions: ShortcutActions,
): ShortcutSurface {
  const dialog = reference();
  const open = element(
    "button",
    {
      type: "button",
      class: "slidx-shortcuts-open",
      "aria-label": "Keyboard shortcuts",
      "aria-expanded": false,
      title: "Keyboard shortcuts (?)",
    },
    ["?"],
  );
  const root = element("aside", { class: "slidx-shortcuts" }, [open, dialog]);

  function show(): void {
    dialog.hidden = false;
    open.setAttribute("aria-expanded", "true");
    dialog.querySelector<HTMLElement>(".slidx-shortcuts-close")?.focus();
  }

  function hide(): void {
    if (dialog.hidden) return;
    dialog.hidden = true;
    open.setAttribute("aria-expanded", "false");
    open.focus();
  }

  open.addEventListener("click", show);
  dialog.querySelector(".slidx-shortcuts-close")?.addEventListener("click", hide);

  return {
    root,
    render() {},
    keydown(event) {
      if (event.isComposing || event.repeat || writingIn(event)) return;

      const key = event.key.toLowerCase();
      const primary = event.metaKey || event.ctrlKey;

      if (primary && key === "z") {
        handled(event, () => void (event.shiftKey ? session.redo() : session.undo()));
        return;
      }

      if (primary && key === "d") {
        handled(event, () => {
          if (session.state().slides.length > 0) {
            void session.run({ op: "duplicateSlide", slide: session.state().selection.slide });
          }
        });
        return;
      }

      if (primary && key === "m") {
        handled(event, () => {
          const at = Math.min(session.state().selection.slide + 1, session.state().slides.length);
          void session.run({ op: "insertSlide", at, body: "## New slide" });
        });
        return;
      }

      if (key === "?") {
        handled(event, show);
        return;
      }

      if (primary || event.shiftKey) return;

      if (key === "escape" && !dialog.hidden) {
        handled(event, hide);
        return;
      }

      if (key === "v") {
        handled(event, () => canvas.showVisual());
        return;
      }

      if (key === "t") {
        handled(event, () => canvas.focusText());
        return;
      }

      if (key === "m") {
        handled(event, () => canvas.showMarkdown());
        return;
      }

      if (key === "p") {
        handled(event, () => actions.present());
        return;
      }

      if (key === "pageup" || key === "pagedown") {
        handled(event, () => selectBy(session, key === "pageup" ? -1 : 1));
        return;
      }

      if (!inOutline(event)) return;

      if (key === "arrowup" || key === "arrowdown") {
        handled(event, () => {
          const by = key === "arrowup" ? -1 : 1;
          if (event.altKey) moveBy(session, by);
          else selectBy(session, by);
        });
        return;
      }

      if (key === "backspace" || key === "delete") {
        handled(event, () => {
          const state = session.state();
          if (state.slides.length > 1) {
            void session.run({ op: "removeSlide", slide: state.selection.slide });
          }
        });
      }
    },
  };
}

function reference(): HTMLElement {
  const title = element("h2", { id: "slidx-shortcuts-title" }, ["Keyboard shortcuts"]);
  const close = element(
    "button",
    { type: "button", class: "slidx-shortcuts-close", "aria-label": "Close keyboard shortcuts" },
    ["Close"],
  );
  const entries = REFERENCE.map(({ keys, label }) =>
    element("div", { class: "slidx-shortcut" }, [
      element(
        "dt",
        {},
        keys.map((key) => element("kbd", {}, [key])),
      ),
      element("dd", {}, [label]),
    ]),
  );

  const dialog = element(
    "section",
    {
      class: "slidx-shortcuts-dialog",
      role: "dialog",
      "aria-labelledby": "slidx-shortcuts-title",
    },
    [
      element("header", { class: "slidx-shortcuts-head" }, [title, close]),
      element("dl", { class: "slidx-shortcuts-list" }, entries),
    ],
  );
  dialog.hidden = true;
  return dialog;
}

function handled(event: KeyboardEvent, action: () => void): void {
  event.preventDefault();
  action();
}

function writingIn(event: KeyboardEvent): boolean {
  return [event.target, event.view?.document.activeElement].some((candidate) => {
    if (!candidate || (candidate as Node).nodeType !== Node.ELEMENT_NODE) return false;
    const element = candidate as Element;
    return (
      element.matches("input, textarea, select") ||
      element.closest("[contenteditable='true']") !== null
    );
  });
}

function inOutline(event: KeyboardEvent): boolean {
  const target = event.target;
  return target instanceof Element && target.closest(".slidx-outline") !== null;
}

function selectBy(session: Session, by: number): void {
  const state = session.state();
  const slide = Math.max(0, Math.min(state.selection.slide + by, state.slides.length - 1));
  if (slide === state.selection.slide) return;
  session.select({ slide, range: undefined, text: undefined });
}

function moveBy(session: Session, by: number): void {
  const state = session.state();
  const from = state.selection.slide;
  const to = from + by;
  if (to < 0 || to >= state.slides.length) return;
  void session.run({ op: "moveSlide", slide: from, to });
}
