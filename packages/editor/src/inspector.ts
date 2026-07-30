/**
 * What is selected, and what can be said about it.
 *
 * Three things at once, in the order an author reaches for them: the phrase
 * they just selected, the slide they are on, and the deck. Each field writes
 * one frontmatter key through `setField`, which replaces that key's bytes and
 * leaves every other key — including the ones this version of slidx has never
 * heard of — exactly as typed.
 *
 * Giving a selection a class is how "select these words and animate them" is
 * spelled in a file, and it is `addMark`. The class lands in the Markdown as
 * `[three words]{.accent}`, which a theme styles and a step can target by name.
 */

import { element, fill, field } from "./dom";
import { locateSelection } from "./selection";
import type { EditOp } from "./operations";
import type { Surface } from "./outline";
import type { EditorState } from "./session";

export interface InspectorHandlers {
  run(op: EditOp): void;
}

export interface InspectorOptions {
  bodyOf(slide: number): string;
}

/** The keys worth offering by name. Anything else the author wrote still shows. */
const DECK_KEYS = ["title", "event", "duration", "theme", "aspect"];
const SLIDE_KEYS = ["layout", "transition", "budget", "optional"];

export function createInspector(handlers: InspectorHandlers, options: InspectorOptions): Surface {
  const panels = element("div", { class: "slidx-inspector-panels" });
  const root = element("section", { class: "slidx-inspector", "aria-label": "Inspector" }, [
    element("header", { class: "slidx-panel-head" }, [element("h2", {}, ["Inspector"])]),
    panels,
  ]);

  return {
    root,
    render(state) {
      fill(panels, [
        selectionPanel(state, handlers, options),
        slidePanel(state, handlers),
        deckPanel(state, handlers),
      ]);
    },
  };
}

function selectionPanel(
  state: EditorState,
  handlers: InspectorHandlers,
  options: InspectorOptions,
): HTMLElement {
  const selected = state.selection.text ?? "";

  if (selected.length === 0) {
    return group("Selection", [
      element("p", { class: "slidx-hint" }, ["Select words in the slide to give them a name."]),
    ]);
  }

  const located = locateSelection(options.bodyOf(state.selection.slide), selected, 0);

  if ("problem" in located) {
    return group("Selection", [
      element("p", { class: "slidx-hint" }, [
        `“${selected}” is written differently in the Markdown, so it cannot be addressed yet.`,
      ]),
    ]);
  }

  const classes = element("input", { type: "text", placeholder: "accent", value: "" });
  const key = element("input", { type: "text", placeholder: "result", value: "" });
  const apply = element("button", { type: "button", class: "slidx-add" }, ["Add mark"]);

  apply.addEventListener("click", () =>
    handlers.run({
      op: "addMark",
      slide: state.selection.slide,
      range: located.range,
      attributes: {
        key: key.value.trim() || undefined,
        classes: classes.value
          .split(/\s+/)
          .map((name) => name.trim())
          .filter(Boolean),
      },
    }),
  );

  return group("Selection", [
    element("p", { class: "slidx-selected" }, [located.text]),
    field("Classes", classes),
    field("Name", key),
    apply,
  ]);
}

function slidePanel(state: EditorState, handlers: InspectorHandlers): HTMLElement {
  const index = state.selection.slide;
  const slide = state.slides[index];
  if (!slide) return group("Slide", []);

  const notes = element("textarea", { rows: 4, "aria-label": "Speaker notes" });
  notes.value = slide.notes.join("\n\n");
  notes.addEventListener("blur", () =>
    handlers.run({ op: "setNotes", slide: index, notes: notes.value }),
  );

  // The first slide's block is the deck's, so showing everything written in it
  // here would repeat the whole Deck panel one heading higher up.
  const written = index === 0 ? {} : (slide.frontmatter ?? {});

  return group("Slide", [
    ...keyFields(SLIDE_KEYS, written, (key, value) =>
      handlers.run({ op: "setField", slide: index, key, value }),
    ),
    field("Notes", notes),
  ]);
}

/**
 * The deck's own keys.
 *
 * They are the first slide's, which is what the parser already believes: a deck
 * and its opening slide share one frontmatter block.
 */
function deckPanel(state: EditorState, handlers: InspectorHandlers): HTMLElement {
  const written = state.slides[0]?.frontmatter ?? {};

  return group(
    "Deck",
    keyFields(DECK_KEYS, written, (key, value) =>
      handlers.run({ op: "setField", slide: 0, key, value }),
    ),
  );
}

/**
 * One input per key, offered keys first and then whatever else is written.
 *
 * An author who wrote a key slidx does not know about still sees it, because a
 * tool that hides what it does not understand is a tool that loses it.
 */
function keyFields(
  offered: string[],
  written: Record<string, unknown>,
  commit: (key: string, value: unknown) => void,
): HTMLElement[] {
  const keys = [...offered, ...Object.keys(written).filter((key) => !offered.includes(key))];

  return keys.map((key) => {
    const current = written[key];
    const input = element("input", { type: "text", "data-key": key });
    input.value = shown(current);

    // A key whose value is a list or a mapping is shown and not edited. A text
    // box cannot express `steps:`, and committing what one holds would replace
    // an author's whole timeline with the string `[object Object]` — which is
    // what happened before the timeline made that key ordinary.
    if (structured(current)) {
      input.setAttribute("readonly", "");
      input.setAttribute("title", `\`${key}\` is a list, and is edited where it is drawn.`);
      return field(key, input);
    }

    input.addEventListener("blur", () => {
      const value = input.value.trim();
      if (value === shown(current)) return;
      commit(key, coerce(value));
    });

    return field(key, input);
  });
}

/** True when the value is a list or a mapping rather than one scalar. */
function structured(value: unknown): boolean {
  return typeof value === "object" && value !== null;
}

/** A frontmatter value as one line of text. */
function shown(value: unknown): string {
  if (value === undefined || value === null) return "";
  if (Array.isArray(value)) return `${value.length} entries`;
  if (structured(value)) return "a mapping";

  return String(value);
}

/**
 * What a typed field means.
 *
 * `optional: true` has to reach the file as a boolean, and a budget of `90s`
 * has to stay a string. Anything ambiguous stays text and the pipeline decides
 * how to quote it.
 */
function coerce(value: string): unknown {
  if (value === "true") return true;
  if (value === "false") return false;

  return value;
}

function group(name: string, children: (Node | string)[]): HTMLElement {
  return element("div", { class: "slidx-group", "data-group": name.toLowerCase() }, [
    element("h3", {}, [name]),
    ...children,
  ]);
}
