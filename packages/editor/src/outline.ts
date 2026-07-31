/**
 * The deck as a list.
 *
 * Jump, insert, remove, reorder. Reordering is a `moveSlide` operation and not
 * a rewrite — the slide's bytes are the ones that were already in the file, so
 * a deck reordered from here diffs as moved lines. That is the whole reason
 * the operation exists rather than the editor sending a new body.
 *
 * Diagnostics are shown here as a dot on the row they belong to. The pipeline
 * returns them with every parse, so the outline is where an author sees which
 * slide has a problem without opening it.
 */

import { routeFor } from "./canvas";
import type { SlideSummary } from "./client";
import { element } from "./dom";
import type { EditOp } from "./operations";
import type { EditorState } from "./session";

export interface OutlineHandlers {
  select(slide: number): void;
  run(op: EditOp): void;
}

export interface OutlineOptions {
  /** The route the deck is served under, so every preview is the real page. */
  deckBase?: string;
}

export interface Surface {
  root: HTMLElement;
  render(state: EditorState): void;
  /**
   * Let go of anything that outlives the element.
   *
   * Optional because most surfaces are their own DOM and nothing else: removing
   * the frame is all the teardown they need. It exists for the one that holds a
   * connection, which a removed element does not close.
   */
  destroy?(): void;
}

export function createOutline(handlers: OutlineHandlers, options: OutlineOptions = {}): Surface {
  const list = element("ol", { class: "slidx-outline-list" });
  const root = element("section", { class: "slidx-outline", "aria-label": "Slides" }, [
    element("header", { class: "slidx-panel-head" }, [
      element("h2", {}, ["Slides"]),
      addButton(handlers, () => count),
    ]),
    list,
  ]);

  let count = 0;
  let dragging: number | undefined;
  let source: string | undefined;
  let refresh = 0;
  const rows = new Map<string, OutlineRow>();

  const drag: DragHandlers = {
    start: (from) => {
      dragging = from;
    },
    drop: (to) => {
      if (dragging === undefined || dragging === to) return;
      handlers.run({ op: "moveSlide", slide: dragging, to });
      dragging = undefined;
    },
  };

  return {
    root,
    render(state) {
      count = state.slides.length;
      // The session renders its empty starting state before the first read.
      // There is no preview to refresh then, so the first real deck receives
      // the clean public route rather than looking like an edit already landed.
      const changed = rows.size > 0 && source !== undefined && source !== state.source;
      if (changed) refresh += 1;
      source = state.source;

      reconcile(
        list,
        state.slides.map((slide, index) => {
          const found = rows.get(slide.id);
          const kept = found ?? row(slide.id, handlers, drag);
          rows.set(slide.id, kept);
          update(
            kept,
            slide,
            index,
            state,
            routeFor(options.deckBase ?? "slides", index),
            changed ? refresh : undefined,
          );
          return kept;
        }),
        rows,
      );
    },
  };
}

interface DragHandlers {
  start(from: number): void;
  drop(to: number): void;
}

interface OutlineRow {
  id: string;
  root: HTMLElement;
  open: HTMLButtonElement;
  frame: HTMLIFrameElement;
  number: HTMLElement;
  title: HTMLElement;
  caption: HTMLElement;
  duplicate: HTMLButtonElement;
  remove: HTMLButtonElement;
  index: number;
  route?: string;
}

function row(id: string, handlers: OutlineHandlers, drag: DragHandlers): OutlineRow {
  const number = element("span", { class: "slidx-outline-number" });
  const frame = element("iframe", {
    class: "slidx-outline-frame",
    loading: "lazy",
    tabindex: -1,
    "aria-hidden": true,
  });
  const preview = element("span", { class: "slidx-outline-preview" }, [frame]);
  const title = element("span", { class: "slidx-outline-title" });
  const caption = element("span", { class: "slidx-outline-caption" }, [title]);

  // The button lies over the visual row rather than containing the iframe.
  // An iframe is interactive content even with pointer events disabled, and
  // nesting one in a button would make invalid, confusing accessibility HTML.
  const open = element("button", { type: "button", class: "slidx-outline-open" });

  const remove = element("button", { type: "button", class: "slidx-outline-remove" }, ["×"]);
  const duplicate = element(
    "button",
    {
      type: "button",
      class: "slidx-outline-duplicate",
      title: "Duplicate slide",
    },
    ["⧉"],
  );

  const item = element(
    "li",
    {
      class: "slidx-outline-row",
      draggable: true,
    },
    [number, preview, caption, open, duplicate, remove],
  );

  const result: OutlineRow = {
    id,
    root: item,
    open,
    frame,
    number,
    title,
    caption,
    duplicate,
    remove,
    index: 0,
  };

  open.addEventListener("click", () => handlers.select(result.index));
  duplicate.addEventListener("click", () =>
    handlers.run({ op: "duplicateSlide", slide: result.index }),
  );
  remove.addEventListener("click", () => handlers.run({ op: "removeSlide", slide: result.index }));

  item.addEventListener("dragstart", () => drag.start(result.index));
  item.addEventListener("dragover", (event) => event.preventDefault());
  item.addEventListener("drop", (event) => {
    event.preventDefault();
    drag.drop(result.index);
  });

  return result;
}

/** Updates a kept row without removing its iframe from the document. */
function update(
  row: OutlineRow,
  slide: SlideSummary,
  index: number,
  state: EditorState,
  route: string,
  refresh: number | undefined,
): void {
  const worst = severityOn(state, index);
  const label = slide.title ?? `Slide ${index + 1}`;
  row.index = index;
  row.number.textContent = String(index + 1);
  row.title.textContent = label;
  row.open.setAttribute("aria-label", `Open slide ${index + 1}: ${label}`);
  row.duplicate.setAttribute("aria-label", `Duplicate slide ${index + 1}`);
  row.remove.setAttribute("aria-label", `Remove slide ${index + 1}`);
  row.root.setAttribute("data-slide", String(index));

  if (index === state.selection.slide) row.root.setAttribute("aria-current", "true");
  else row.root.removeAttribute("aria-current");

  if (worst) row.root.setAttribute("data-severity", worst);
  else row.root.removeAttribute("data-severity");

  const dot = row.caption.querySelector<HTMLElement>(".slidx-dot");
  if (worst && !dot) row.caption.append(element("span", { class: "slidx-dot", title: worst }));
  else if (worst) dot!.title = worst;
  else dot?.remove();

  // A state change that did not change the source — selecting a slide or
  // receiving somebody's presence — must not navigate every preview again.
  // A source change does: the route is the same URL but the page it serves is
  // now different, and the query is only a cache-busting viewing concern.
  if (row.route !== route || refresh !== undefined) {
    row.route = route;
    row.frame.setAttribute("src", refresh === undefined ? route : `${route}?outline=${refresh}`);
  }
}

/**
 * Puts keyed rows in deck order, moving only rows whose order really changed.
 *
 * `replaceChildren` would detach every iframe even when the only change was
 * `aria-current`, and browsers are allowed to reload a frame moved through the
 * DOM. A presence pulse would then become a request for every slide.
 */
function reconcile(
  list: HTMLOListElement,
  wanted: OutlineRow[],
  rows: Map<string, OutlineRow>,
): void {
  const ids = new Set(wanted.map((row) => row.id));
  for (const [id, row] of rows) {
    if (ids.has(id)) continue;
    row.root.remove();
    rows.delete(id);
  }

  let at: ChildNode | null = list.firstChild;
  for (const row of wanted) {
    if (row.root === at) {
      at = at.nextSibling;
      continue;
    }

    list.insertBefore(row.root, at);
  }
}

function addButton(handlers: OutlineHandlers, at: () => number): HTMLElement {
  const button = element("button", { type: "button", class: "slidx-add" }, ["Add slide"]);

  // A new slide arrives with a heading rather than empty, because an empty
  // slide in the outline is indistinguishable from a bug.
  button.addEventListener("click", () =>
    handlers.run({ op: "insertSlide", at: at(), body: "## New slide" }),
  );

  return button;
}

/** The worst thing the linter said about one slide, or nothing. */
function severityOn(state: EditorState, index: number): string | undefined {
  const found = state.diagnostics.filter((finding) => finding.slideIndex === index);
  if (found.length === 0) return undefined;

  return found.some((finding) => finding.severity === "error") ? "error" : "warning";
}
