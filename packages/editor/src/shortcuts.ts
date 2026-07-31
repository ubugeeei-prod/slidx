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

interface ShortcutGroup {
  label: string;
  shortcuts: Shortcut[];
}

const REFERENCE: ShortcutGroup[] = [
  {
    label: "Navigation",
    shortcuts: [
      { keys: ["←", "→"], label: "Previous / next slide, or move the selected block" },
      { keys: ["Page ↑", "Page ↓"], label: "Previous / next slide" },
      { keys: ["Home", "End"], label: "First / last slide" },
      { keys: ["↑", "↓"], label: "Navigate the focused outline" },
    ],
  },
  {
    label: "Editing",
    shortcuts: [
      { keys: ["⌘/Ctrl", "Z"], label: "Undo" },
      { keys: ["⇧", "⌘/Ctrl", "Z"], label: "Redo" },
      { keys: ["Ctrl", "Y"], label: "Redo" },
      { keys: ["⌘/Ctrl", "D"], label: "Duplicate the selected block, or the slide" },
      { keys: ["↑", "↓"], label: "Move the selected block up or down its region" },
    ],
  },
  {
    label: "Slides",
    shortcuts: [
      { keys: ["⌘/Ctrl", "M"], label: "Add slide" },
      { keys: ["⌥/Alt", "↑", "↓"], label: "Move the focused slide" },
      { keys: ["⌫/Delete"], label: "Remove the focused slide" },
    ],
  },
  {
    label: "View",
    shortcuts: [
      { keys: ["V"], label: "Visual mode" },
      { keys: ["T"], label: "Edit slide text" },
      { keys: ["M"], label: "Markdown mode" },
      { keys: ["P"], label: "Open presenter view" },
      { keys: ["?"], label: "Keyboard shortcuts" },
      { keys: ["Esc"], label: "Close keyboard shortcuts" },
    ],
  },
];

export interface ShortcutSurface extends Surface {
  keydown(event: KeyboardEvent): void;
}

export interface ShortcutActions {
  present(): void;
  /**
   * Moves the selected block, answering false when nothing could be measured.
   *
   * Handed in rather than done here, because moving a block is `nudge` over a
   * measured slide and only the arrange surface has measured one.
   */
  nudge?(index: number, key: string): boolean;
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
      // A focused control gets the first chance to interpret a key. Panel
      // separators, range inputs and future canvas controls mark a handled key
      // this way; treating it as a global command as well would resize a panel
      // and page the deck in the same press.
      if (event.defaultPrevented || event.isComposing || event.repeat || writingIn(event)) return;

      const key = event.key.toLowerCase();
      const primary = event.metaKey || event.ctrlKey;

      if (primary && key === "z") {
        handled(event, () => void (event.shiftKey ? session.redo() : session.undo()));
        return;
      }

      // The other redo spelling used on Windows and Linux. It deliberately
      // stays Ctrl-only: Command-Y has a different native meaning on macOS,
      // while Command-Shift-Z above is the platform's redo binding.
      if (event.ctrlKey && !event.metaKey && !event.shiftKey && key === "y") {
        handled(event, () => void session.redo());
        return;
      }

      // Duplicates what is selected, which is a block when there is one and the
      // whole slide otherwise. One binding rather than two, because "make
      // another one of this" is one intention and the selection already says
      // what "this" is.
      if (primary && key === "d") {
        handled(event, () => {
          const { slide, block } = session.state().selection;
          if (session.state().slides.length === 0) return;

          void session.run(
            block === undefined
              ? { op: "duplicateSlide", slide }
              : { op: "duplicateBlock", slide, block },
          );
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

      if (key === "home" || key === "end") {
        handled(event, () => selectEdge(session, key === "home" ? "first" : "last"));
        return;
      }

      // Arrows mean one of two things, and the selection is what decides which.
      //
      // With a block selected they move it, because that is what an author who
      // just clicked one is reaching for — and the move is semantic, so it
      // lands as a region and a position in the file rather than a coordinate.
      //
      // With nothing selected they page the deck, which is what every other
      // surface that shows slides does with them.
      //
      // Neither happens inside the outline: it has its own arrows, and its own
      // reasons for them.
      if (!inOutline(event) && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
        const block = session.state().selection.block;

        if (block === undefined) {
          handled(event, () => selectBy(session, event.key === "ArrowLeft" ? -1 : 1));
          return;
        }
      }

      if (!inOutline(event) && event.key.startsWith("Arrow")) {
        const block = session.state().selection.block;
        if (block === undefined || actions.nudge === undefined) return;

        // Only when the surface could act on it. A slide whose geometry cannot
        // be read — the Markdown view is up, or the canvas has not loaded — has
        // nothing to move, and swallowing the key there would leave an author
        // pressing a dead arrow with no way to tell why.
        if (actions.nudge(block, event.key)) event.preventDefault();
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
  const groups = REFERENCE.map(({ label, shortcuts }, index) => {
    const id = `slidx-shortcuts-group-${index}`;
    const entries = shortcuts.map(({ keys, label }) =>
      element("div", { class: "slidx-shortcut" }, [
        element(
          "dt",
          {},
          keys.map((key) => element("kbd", {}, [key])),
        ),
        element("dd", {}, [label]),
      ]),
    );

    return element("section", { class: "slidx-shortcut-group", "aria-labelledby": id }, [
      element("h3", { id }, [label]),
      element("dl", { class: "slidx-shortcuts-list" }, entries),
    ]);
  });

  const dialog = element(
    "section",
    {
      class: "slidx-shortcuts-dialog",
      role: "dialog",
      "aria-labelledby": "slidx-shortcuts-title",
    },
    [
      element("header", { class: "slidx-shortcuts-head" }, [title, close]),
      element("div", { class: "slidx-shortcut-groups" }, groups),
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

function selectEdge(session: Session, edge: "first" | "last"): void {
  const state = session.state();
  if (state.slides.length === 0) return;

  const slide = edge === "first" ? 0 : state.slides.length - 1;
  if (slide === state.selection.slide) return;
  session.select({ slide, block: undefined, range: undefined, text: undefined });
}

function moveBy(session: Session, by: number): void {
  const state = session.state();
  const from = state.selection.slide;
  const to = from + by;
  if (to < 0 || to >= state.slides.length) return;
  void session.run({ op: "moveSlide", slide: from, to });
}
