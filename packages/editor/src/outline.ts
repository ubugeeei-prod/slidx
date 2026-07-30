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

import { element, fill } from "./dom";
import type { EditOp } from "./operations";
import type { EditorState } from "./session";

export interface OutlineHandlers {
  select(slide: number): void;
  run(op: EditOp): void;
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

export function createOutline(handlers: OutlineHandlers): Surface {
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

  return {
    root,
    render(state) {
      count = state.slides.length;

      fill(
        list,
        state.slides.map((slide, index) =>
          row(slide.title, index, state, handlers, {
            start: (from) => {
              dragging = from;
            },
            drop: (to) => {
              if (dragging === undefined || dragging === to) return;
              handlers.run({ op: "moveSlide", slide: dragging, to });
              dragging = undefined;
            },
          }),
        ),
      );
    },
  };
}

interface DragHandlers {
  start(from: number): void;
  drop(to: number): void;
}

function row(
  title: string | undefined,
  index: number,
  state: EditorState,
  handlers: OutlineHandlers,
  drag: DragHandlers,
): HTMLElement {
  const worst = severityOn(state, index);

  // A real button rather than a row with a click handler: an outline is how an
  // author moves through a deck, and reaching for the mouse to do it is what
  // makes a tool tiring.
  const open = element("button", { type: "button", class: "slidx-outline-open" }, [
    element("span", { class: "slidx-outline-number" }, [String(index + 1)]),
    element("span", { class: "slidx-outline-title" }, [title ?? `Slide ${index + 1}`]),
    worst ? element("span", { class: "slidx-dot", title: worst }) : "",
  ]);

  const remove = element(
    "button",
    { type: "button", class: "slidx-outline-remove", "aria-label": `Remove slide ${index + 1}` },
    ["×"],
  );

  const item = element(
    "li",
    {
      class: "slidx-outline-row",
      draggable: true,
      "aria-current": index === state.selection.slide,
      "data-slide": index,
      "data-severity": worst,
    },
    [open, remove],
  );

  open.addEventListener("click", () => handlers.select(index));
  remove.addEventListener("click", () => handlers.run({ op: "removeSlide", slide: index }));

  item.addEventListener("dragstart", () => drag.start(index));
  item.addEventListener("dragover", (event) => event.preventDefault());
  item.addEventListener("drop", (event) => {
    event.preventDefault();
    drag.drop(index);
  });

  return item;
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
