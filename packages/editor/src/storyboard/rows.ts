/**
 * One row per slide: what the speaker says, how long it gets, and whether it
 * can go.
 *
 * The three questions a speaker actually has are "what am I saying here", "does
 * this fit" and "what do I cut", so those are the three things in a row. There
 * is deliberately no picture of the slide: the canvas is already the one answer
 * about what a slide looks like, and a second preview drawn another way would be
 * a second source of truth about layout.
 *
 * Both fields commit on blur rather than on every keystroke. An operation per
 * character would write the author's file per character, and the undo stack is a
 * list of operations — so it would also take a hundred presses to get back.
 */

import { element } from "../dom";
import type { EditOp } from "../operations";
import { formatSeconds, type SlideTime } from "./plan";

export interface RowHandlers {
  select(slide: number): void;
  run(op: EditOp): void;
  /** A drag has started on this slide. */
  lift(from: number): void;
  /** The drag ended in the gap above slide `gap`. */
  drop(gap: number): void;
}

/**
 * Every slide, with a drop target in each gap between them.
 *
 * The gaps are elements rather than a measurement of where a pointer is: a drop
 * *between* two slides is the gesture, and something has to be there to catch
 * it. They are hidden from assistive technology because dragging is not
 * available to it — the keyboard equivalent is Alt with an arrow key, which
 * operates on the selected row.
 */
export function slideRows(
  slides: readonly SlideTime[],
  selected: number,
  handlers: RowHandlers,
): HTMLElement[] {
  const nodes: HTMLElement[] = [];

  for (const slide of slides) {
    nodes.push(gap(slide.index, handlers), row(slide, selected, handlers));
  }
  nodes.push(gap(slides.length, handlers));

  return nodes;
}

function gap(at: number, handlers: RowHandlers): HTMLElement {
  const node = element("div", { class: "slidx-sb-drop", "data-gap": at, "aria-hidden": "true" });

  node.addEventListener("dragover", (event) => {
    event.preventDefault();
    node.setAttribute("data-over", "true");
  });
  node.addEventListener("dragleave", () => node.removeAttribute("data-over"));
  node.addEventListener("drop", (event) => {
    event.preventDefault();
    node.removeAttribute("data-over");
    handlers.drop(at);
  });

  return node;
}

function row(slide: SlideTime, selected: number, handlers: RowHandlers): HTMLElement {
  const node = element(
    "article",
    {
      class: "slidx-sb-slide",
      role: "listitem",
      draggable: true,
      "data-slide": slide.index,
      "data-optional": slide.optional,
      "aria-current": slide.index === selected,
    },
    [
      element("div", { class: "slidx-sb-grip", "aria-hidden": "true" }, ["⠿"]),
      message(slide, handlers),
      time(slide, handlers),
    ],
  );

  node.addEventListener("dragstart", () => handlers.lift(slide.index));

  return node;
}

/** The slide's title, and under it what the speaker says over it. */
function message(slide: SlideTime, handlers: RowHandlers): HTMLElement {
  const jump = element("button", { type: "button", class: "slidx-sb-jump" }, [
    element("span", { class: "slidx-sb-number" }, [String(slide.index + 1)]),
    element("span", { class: "slidx-sb-title" }, [slide.title]),
  ]);
  jump.addEventListener("click", () => handlers.select(slide.index));

  const notes = element("textarea", {
    class: "slidx-sb-message",
    rows: 2,
    "aria-label": `What the speaker says on slide ${slide.index + 1}`,
    placeholder: "What do you say here?",
  });
  notes.value = slide.message;
  notes.addEventListener("blur", () => {
    if (notes.value === slide.message) return;
    handlers.run({ op: "setNotes", slide: slide.index, notes: notes.value });
  });

  // Said in a sentence rather than left as an empty field. A slide with no notes
  // is a slide whose message has not been written down yet, and that is a thing
  // worth knowing about a talk — a blank box only looks like a slide with
  // nothing to say.
  const unwritten =
    slide.message.length === 0
      ? [element("p", { class: "slidx-sb-unwritten" }, ["No message written down yet."])]
      : [];

  return element("div", { class: "slidx-sb-main" }, [jump, ...unwritten, notes]);
}

/** The budget, what slidx made of it, and whether the slide can be dropped. */
function time(slide: SlideTime, handlers: RowHandlers): HTMLElement {
  const budget = element("input", {
    type: "text",
    class: "slidx-sb-budget",
    "aria-label": `Budget for slide ${slide.index + 1}`,
    placeholder: "from notes",
  });
  budget.value = slide.written;
  budget.addEventListener("blur", () => {
    const value = budget.value.trim();
    if (value === slide.written) return;
    handlers.run(
      value.length === 0
        ? { op: "removeField", slide: slide.index, key: "budget" }
        : { op: "setField", slide: slide.index, key: "budget", value },
    );
  });

  const cut = element("input", {
    type: "checkbox",
    class: "slidx-sb-optional",
    "aria-label": `Slide ${slide.index + 1} can be cut`,
  });
  cut.checked = slide.optional;
  // The value written is the slide's, not the box's: a checkbox reports the
  // state it has just been put into, and reading it back would make the
  // operation depend on the order two DOM events happened to fire in.
  cut.addEventListener("click", () =>
    handlers.run({
      op: "setField",
      slide: slide.index,
      key: "optional",
      value: !slide.optional,
    }),
  );

  return element("div", { class: "slidx-sb-time" }, [
    budget,
    // Which of the two numbers is in force, always. It is also how an author
    // finds out that `budget: soonish` was not understood: the length goes on
    // reading "from notes" while their text sits in the field above it.
    element("p", { class: "slidx-sb-length", "data-source": slide.source }, [length(slide)]),
    element("label", { class: "slidx-sb-cut" }, [cut, element("span", {}, ["Can be cut"])]),
  ]);
}

function length(slide: SlideTime): string {
  if (slide.seconds === 0) return "no length yet";

  return `${formatSeconds(slide.seconds)} ${slide.source === "budget" ? "budgeted" : "from notes"}`;
}
