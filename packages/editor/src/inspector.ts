/**
 * The editor for what the author has selected.
 *
 * The most specific target wins: selected words, then a block, then the slide.
 * Slide and deck settings stay one click away instead of being stacked below
 * every selection. Every control still emits one semantic operation; this
 * surface never composes Markdown.
 */

import { blockPanel } from "./block-inspector";
import type { BlockSpans } from "./client";
import { deckTransitionField, slideDelivery } from "./delivery-controls";
import { element, fill, field } from "./dom";
import type { BlockVisual } from "./freeform-color";
import type { SlideGeometry } from "./geometry";
import { applyInspectorStyles } from "./inspector-styles";
import { layoutField } from "./layout-picker";
import type { EditOp } from "./operations";
import type { Surface } from "./outline";
import type { EditorState } from "./session";
import { textPanel } from "./text-inspector";
import { themeField, type VisibleScheme } from "./theme-picker";

export interface InspectorHandlers {
  run(op: EditOp): void;
  selectBlock?(block: number | undefined): void;
}

export interface InspectorOptions {
  bodyOf(slide: number): string;
  blocksOf?(slide: number): readonly BlockSpans[];
  /** Real boxes from the rendered deck, when the canvas is visible. */
  geometry?(): SlideGeometry | undefined;
  /** Real computed colour from the rendered deck. */
  visualOf?(block: number): BlockVisual | undefined;
  /** The light or dark palette currently visible in the canvas. */
  scheme?(): VisibleScheme;
}

type InspectorTab = "text" | "block" | "slide" | "deck";

/** The keys worth offering by name. Anything else the author wrote still shows. */
const DECK_KEYS = ["title", "event", "duration", "aspect"];

export function createInspector(handlers: InspectorHandlers, options: InspectorOptions): Surface {
  const tabs = element("div", {
    class: "slidx-inspector-tabs",
    role: "tablist",
    "aria-label": "Inspector target",
    "aria-orientation": "horizontal",
  });
  const panels = element("div", { class: "slidx-inspector-panels" });
  const root = element("section", { class: "slidx-inspector", "aria-label": "Inspector" }, [
    element("header", { class: "slidx-panel-head slidx-inspector-head" }, [
      element("div", {}, [
        element("span", { class: "slidx-inspector-eyebrow" }, ["Properties"]),
        element("h2", {}, ["Inspector"]),
      ]),
    ]),
    tabs,
    panels,
  ]);

  let active: InspectorTab = "slide";
  let target = "";

  function activate(next: InspectorTab): void {
    active = next;
    for (const tab of tabs.querySelectorAll<HTMLElement>("[role=tab]")) {
      const selected = tab.dataset.tab === next;
      tab.setAttribute("aria-selected", String(selected));
      tab.tabIndex = selected ? 0 : -1;
    }
    for (const panel of panels.querySelectorAll<HTMLElement>("[role=tabpanel]")) {
      panel.hidden = panel.dataset.panel !== next;
    }
  }

  tabs.addEventListener("keydown", (event) => {
    const tab = (event.target as HTMLElement).closest<HTMLElement>("[role=tab]");
    if (!tab || !tabs.contains(tab)) return;

    const available = [...tabs.querySelectorAll<HTMLElement>("[role=tab]")];
    const at = available.indexOf(tab);
    const next =
      event.key === "ArrowRight"
        ? available[(at + 1) % available.length]
        : event.key === "ArrowLeft"
          ? available[(at - 1 + available.length) % available.length]
          : event.key === "Home"
            ? available[0]
            : event.key === "End"
              ? available.at(-1)
              : undefined;
    if (!next) return;

    event.preventDefault();
    activate(next.dataset.tab as InspectorTab);
    next.focus();
  });

  return {
    root,
    render(state) {
      applyInspectorStyles(root.ownerDocument);

      const specific = defaultTab(state);
      const nextTarget = selectionKey(state);
      if (nextTarget !== target) {
        active = specific;
        target = nextTarget;
      }

      const available: InspectorTab[] = [
        ...(state.selection.text ? (["text"] as const) : []),
        ...(state.selection.block === undefined ? [] : (["block"] as const)),
        "slide",
        "deck",
      ];
      if (!available.includes(active)) active = specific;

      fill(
        tabs,
        available.map((name) => tabButton(name, () => activate(name))),
      );
      fill(panels, [
        tabPanel("text", textPanel(state, handlers, options)),
        tabPanel("block", blockPanel(state, handlers, options)),
        tabPanel("slide", slidePanel(state, handlers)),
        tabPanel("deck", deckPanel(state, handlers, options)),
      ]);
      activate(active);
    },
  };
}

function defaultTab(state: EditorState): InspectorTab {
  if (state.selection.text) return "text";
  return state.selection.block === undefined ? "slide" : "block";
}

function selectionKey(state: EditorState): string {
  const { slide, block, range, text } = state.selection;
  return `${slide}:${block ?? ""}:${range?.start ?? ""}:${range?.end ?? ""}:${text ?? ""}`;
}

function tabButton(name: InspectorTab, choose: () => void): HTMLButtonElement {
  const labels: Record<InspectorTab, string> = {
    text: "Text",
    block: "Block",
    slide: "Slide",
    deck: "Deck",
  };
  const button = element(
    "button",
    {
      type: "button",
      class: "slidx-inspector-tab",
      role: "tab",
      "data-tab": name,
      "aria-controls": `slidx-inspector-${name}`,
    },
    [labels[name]],
  ) as HTMLButtonElement;
  button.addEventListener("click", choose);
  return button;
}

function tabPanel(name: InspectorTab, panel: HTMLElement): HTMLElement {
  panel.id = `slidx-inspector-${name}`;
  panel.dataset.panel = name;
  panel.setAttribute("role", "tabpanel");
  return panel;
}

function slidePanel(state: EditorState, handlers: InspectorHandlers): HTMLElement {
  const index = state.selection.slide;
  const slide = state.slides[index];
  if (!slide) return group("Slide", []);

  const written = index === 0 ? {} : (slide.frontmatter ?? {});
  const ordinary = without(written, "layout", "transition", "budget", "optional");

  return group("Slide", [
    layoutField(state, slide, index, (op) => handlers.run(op)),
    slideDelivery(state, slide, index, (op) => handlers.run(op)),
    ...keyFields([], ordinary, (key, value) =>
      handlers.run({ op: "setField", slide: index, key, value }),
    ),
  ]);
}

function deckPanel(
  state: EditorState,
  handlers: InspectorHandlers,
  options: InspectorOptions,
): HTMLElement {
  const written = without(
    state.slides[0]?.frontmatter ?? {},
    "layout",
    "theme",
    "transition",
    "budget",
    "optional",
  );

  return group("Deck", [
    themeField(state, options.scheme?.() ?? "light", (op) => handlers.run(op)),
    deckTransitionField(state, (op) => handlers.run(op)),
    ...keyFields(DECK_KEYS, written, (key, value) =>
      handlers.run({ op: "setField", slide: 0, key, value }),
    ),
  ]);
}

function without(values: Record<string, unknown>, ...removed: string[]): Record<string, unknown> {
  return Object.fromEntries(Object.entries(values).filter(([key]) => !removed.includes(key)));
}

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

function structured(value: unknown): boolean {
  return typeof value === "object" && value !== null;
}

function shown(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) return `${value.length} entries`;

  return value === null || value === undefined ? "" : "a mapping";
}

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
