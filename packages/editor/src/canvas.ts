/**
 * The slide, rendered as it will really look.
 *
 * The canvas is an iframe pointed at the deck's own route, so the preview is
 * the page the build emits, produced by the same WebAssembly module, through
 * the same shell and the same theme. That is why slidx compiled the pipeline to
 * WebAssembly rather than a native addon in the first place — a preview drawn
 * by a second renderer would be a second source of truth about layout, which is
 * exactly the bug this architecture exists to prevent.
 *
 * It also means the deck's CSS is on the other side of a document boundary and
 * cannot reach the tool around it.
 *
 * # What is editable in place
 *
 * The heading, because a heading is plain text on both sides and setting one is
 * an operation that carries plain text. Everything else in the body is edited
 * as the Markdown it is: rendered HTML does not convert back without a second
 * writer, and there is only ever one writer.
 */

import { element } from "./dom";
import type { EditOp } from "./operations";
import type { EditorState } from "./session";
import type { Surface } from "./outline";

export interface CanvasHandlers {
  run(op: EditOp): void;
  selected(text: string, at: number): void;
}

export interface CanvasOptions {
  /** The route the deck is served under, so the frame shows the real page. */
  deckBase: string;
  bodyOf(slide: number): string;
}

export function createCanvas(handlers: CanvasHandlers, options: CanvasOptions): Surface {
  const frame = element("iframe", { class: "slidx-canvas-frame", title: "Slide preview" });
  const source = element("textarea", {
    class: "slidx-canvas-source",
    spellcheck: false,
    "aria-label": "Slide Markdown",
  });

  const toggle = element("button", { type: "button", class: "slidx-canvas-toggle" }, ["Markdown"]);
  const stage = element("div", { class: "slidx-canvas-stage" }, [frame, source]);
  const root = element("section", { class: "slidx-canvas", "aria-label": "Slide" }, [
    element("header", { class: "slidx-panel-head" }, [element("h2", {}, ["Slide"]), toggle]),
    stage,
  ]);

  let slide = 0;
  let editing = false;
  let shown = "";

  toggle.addEventListener("click", () => {
    editing = !editing;
    stage.setAttribute("data-editing", String(editing));
    if (editing) source.focus();
  });

  // On blur rather than on every keystroke: an operation per character would
  // write the file per character, and the undo stack is a list of operations.
  source.addEventListener("blur", () => {
    if (source.value === shown) return;
    handlers.run({ op: "setBody", slide, body: source.value });
  });

  frame.addEventListener("load", () => {
    const document = frame.contentDocument;
    if (document) attachEditing(document, slide, handlers);
  });

  return {
    root,
    render(state) {
      const moved = state.selection.slide !== slide;
      slide = state.selection.slide;

      const body = options.bodyOf(slide);
      // Only when it differs, so a re-render does not take the cursor away
      // from an author in the middle of a word.
      if (body !== shown) {
        shown = body;
        source.value = body;
      }

      const next = routeFor(options.deckBase, slide);
      if (moved || frame.getAttribute("data-source") !== state.source) {
        frame.setAttribute("data-source", state.source);
        frame.setAttribute("src", `${next}?at=${Date.now()}`);
      }
    },
  };
}

/** The deck's own URL for one slide. Slides are one-based in a URL. */
export function routeFor(base: string, slide: number): string {
  const prefix = base ? `/${base}` : "";

  return slide === 0 ? `${prefix}/` : `${prefix}/${slide + 1}/`;
}

/**
 * Makes the rendered slide answer to a cursor.
 *
 * Takes a document rather than the frame so it can be driven without one.
 */
export function attachEditing(document: Document, slide: number, handlers: CanvasHandlers): void {
  const body = document.querySelector(".slidx-slide-body");
  if (!body) return;

  const heading = body.querySelector("h1, h2, h3, h4, h5, h6");

  if (heading) {
    heading.setAttribute("contenteditable", "true");
    heading.addEventListener("blur", () => commitHeading(heading, slide, handlers));
    heading.addEventListener("keydown", (event) => {
      const key = (event as KeyboardEvent).key;
      // Enter commits rather than starting a second line: a heading is one
      // line in the file, so a second one here would have nowhere to go.
      if (key !== "Enter") return;
      event.preventDefault();
      commitHeading(heading, slide, handlers);
    });
  }

  const report = () => {
    const selection = document.getSelection();
    const text = selection?.toString() ?? "";
    if (text.trim().length === 0) return;

    handlers.selected(text, offsetIn(body, selection!));
  };

  document.addEventListener("mouseup", report);
  document.addEventListener("keyup", report);
}

function commitHeading(heading: Element, slide: number, handlers: CanvasHandlers): void {
  const text = (heading.textContent ?? "").trim();
  if (text.length === 0) return;

  handlers.run({ op: "setHeading", slide, text });
}

/**
 * How far into the rendered slide a selection starts.
 *
 * Counted in the text a reader sees, which is what decides *which* appearance
 * of a repeated phrase was picked.
 */
function offsetIn(body: Element, selection: Selection): number {
  const range = selection.getRangeAt(0).cloneRange();
  range.selectNodeContents(body);
  range.setEnd(selection.getRangeAt(0).startContainer, selection.getRangeAt(0).startOffset);

  return range.toString().length;
}
