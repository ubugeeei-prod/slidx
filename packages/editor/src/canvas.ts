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
 * Every line of text whose words the pipeline can point at in the file: a
 * heading, a paragraph, a list item, a table cell, and the words inside a mark.
 * The addresses come with the deck — `slidx_edit` reports where each block and
 * each mark is — and [`text`](./text) walks the text a reader sees against them
 * until each run has a byte range. A line whose words are not written anywhere in
 * the source, because a reference or a footnote produced them, gets no range and
 * is not offered for editing at all. Refusing beats guessing: a guess here
 * splices the wrong bytes of somebody's talk.
 *
 * The Markdown view stays, and is still the way to add a block, split a
 * paragraph, or write a fence. What it is no longer is the only way to change a
 * word.
 *
 * # How a change reaches the page
 *
 * By replacing the slide element inside the frame with the one the route now
 * serves, rather than by reloading the frame — see [`live`](./live). Reloading
 * throws away the caret and the scroll position, which is the whole cost of
 * editing on the canvas: an author changes a word, the frame blinks, and they
 * are somewhere else on the slide. Nothing about the preview is weakened by it;
 * the markup still comes from the deck's own route, which is the page the build
 * emits. A slide with steps still reloads, for the reason `live` gives.
 */

import { element } from "./dom";
import type { BlockSpans } from "./client";
import { BLOCK_ATTRIBUTE } from "./geometry";
import { canPatch, caretIn, patch, restore, type Caret } from "./live";
import type { EditOp } from "./operations";
import type { Surface } from "./outline";
import { changeBetween, editableIn, planBlock, rangeOf, type TextPlan } from "./text";

/**
 * Marks a document whose selection is already being reported.
 *
 * The slide is replaced in place when a change lands, so the wiring runs again
 * on a page that already has its listeners — and a second pair would report
 * every selection twice.
 */
const REPORTING_ATTRIBUTE = "data-slidx-reporting";

export interface CanvasHandlers {
  run(op: EditOp): void;
  selected(text: string, at: number): void;
}

/** The slide as the file has it, which is what a text edit names bytes of. */
export interface EditableSource {
  body(): string;
  blocks(): readonly BlockSpans[];
}

export interface CanvasOptions {
  /** The route the deck is served under, so the frame shows the real page. */
  deckBase: string;
  bodyOf(slide: number): string;
  /** Where the slide's blocks and marks are. Absent leaves the page read-only. */
  blocksOf?(slide: number): readonly BlockSpans[];
  /** Injected so bringing the frame up to date is testable without a server. */
  fetch?: typeof globalThis.fetch;
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
  /** Where the author was before the change that is about to land. */
  let held: Caret | undefined;

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

  /** Makes the page in the frame editable, and puts the caret back if it moved. */
  function bind(): void {
    const page = frame.contentDocument;
    if (!page) return;

    const lines = attachEditing(page, slide, handlers, {
      body: () => options.bodyOf(slide),
      blocks: () => options.blocksOf?.(slide) ?? [],
    });

    const wanted = held;
    held = undefined;
    if (!wanted) return;

    const block = page.querySelector(`[${BLOCK_ATTRIBUTE}="${wanted.block}"]`);
    // The line may have gone — a block deleted, or a paragraph that stopped
    // being addressable. Then the caret has nowhere to be, which is what a
    // reload would have done anyway.
    const line = block ? [...block.querySelectorAll("[contenteditable]")][wanted.line] : undefined;
    if (line && lines.includes(line)) restore(page, line, wanted.at);
  }

  frame.addEventListener("load", () => bind());

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
      if (!moved && frame.getAttribute("data-source") === state.source) return;

      frame.setAttribute("data-source", state.source);

      // A slide the author is typing on keeps its document. Everything else —
      // another slide, a staged one, a page that cannot be read — reloads,
      // which is always correct and only costs the caret.
      if (!moved && canPatch(frame, state.slides[slide]?.stopCount ?? 1)) {
        held = caretIn(frame.contentDocument!);

        void patch(frame, next, options.fetch).then((patched) => {
          if (!patched) {
            frame.setAttribute("src", `${next}?at=${Date.now()}`);
            return;
          }

          // Everything that measures inside this frame waits for a load — the
          // lines that become editable, the grips the arrange overlay draws,
          // the handles the resize overlay draws — and a slide replaced in
          // place never loads. Saying so once, through the frame, is what keeps
          // the overlays off the boxes the slide had before the change; an
          // overlay added later needs nothing here.
          frame.dispatchEvent(new Event("load"));
        });

        return;
      }

      frame.setAttribute("src", `${next}?at=${Date.now()}`);
    },
  };
}

/** The deck's own URL for one slide. Slides are one-based in a URL. */
export function routeFor(base: string, slide: number): string {
  const prefix = base ? `/${base}` : "";

  return slide === 0 ? `${prefix}/` : `${prefix}/${slide + 1}/`;
}

/**
 * Makes the rendered slide answer to a cursor, and says which lines it opened.
 *
 * Takes a document rather than the frame so it can be driven without one. The
 * source is read now rather than at commit time, because now is the moment the
 * page and the file are known to agree: this runs on the load, or on the swap,
 * that the render caused.
 */
export function attachEditing(
  document: Document,
  slide: number,
  handlers: CanvasHandlers,
  source: EditableSource,
): Element[] {
  const body = document.querySelector(".slidx-slide-body");
  if (!body) return [];

  const text = source.body();
  const blocks = source.blocks();
  const opened: Element[] = [];

  for (const wrapper of body.querySelectorAll(`[${BLOCK_ATTRIBUTE}]`)) {
    const index = Number(wrapper.getAttribute(BLOCK_ATTRIBUTE));
    const spans = Number.isInteger(index) ? blocks[index] : undefined;
    if (spans === undefined) continue;

    for (const [line, plan] of planBlock(text, spans, editableIn(wrapper))) {
      open(line, plan, slide, handlers);
      opened.push(line);
    }
  }

  // The body is looked up again on every report rather than closed over: the
  // slide is replaced in place when a change lands, and an offset measured
  // against the element that used to be there is measured against nothing.
  const report = () => {
    const selection = document.getSelection();
    const text = selection?.toString() ?? "";
    const showing = document.querySelector(".slidx-slide-body");
    if (text.trim().length === 0 || !showing) return;

    handlers.selected(text, offsetIn(showing, selection!));
  };

  // Once per document, not once per swap: the listeners are on the document and
  // the slide inside it is what gets replaced.
  if (!document.body.hasAttribute(REPORTING_ATTRIBUTE)) {
    document.body.setAttribute(REPORTING_ATTRIBUTE, "");
    document.addEventListener("mouseup", report);
    document.addEventListener("keyup", report);
  }

  return opened;
}

/**
 * Lets one line be typed in, and commits what it says when it is left.
 *
 * On blur rather than on every keystroke, for the reason the Markdown view has:
 * an operation per character would write the file per character, and the undo
 * stack is a list of operations.
 *
 * Nothing intercepts a paste. Only the line's text is ever read, so markup a
 * browser drops into it changes nothing that is sent and is gone on the next
 * render — and an editor that fought the clipboard would break dictation and an
 * input method with it.
 */
function open(line: Element, plan: TextPlan, slide: number, handlers: CanvasHandlers): void {
  line.setAttribute("contenteditable", "true");

  line.addEventListener("keydown", (event) => {
    // Enter commits rather than starting a second line: this is one line of the
    // file, and a second one here would have nowhere to go. Splitting a
    // paragraph is still a thing the Markdown view does.
    if ((event as KeyboardEvent).key !== "Enter") return;

    event.preventDefault();
    commit(line, plan, slide, handlers);
  });

  line.addEventListener("blur", () => commit(line, plan, slide, handlers));
}

function commit(line: Element, plan: TextPlan, slide: number, handlers: CanvasHandlers): void {
  const change = changeBetween(plan.plain, line.textContent ?? "");
  if (change === undefined) return;

  // A line emptied by accident is left alone. Deleting a block is an operation
  // of its own, and a stray Backspace on the last word of a heading should not
  // be the way to reach it.
  if (line.textContent?.trim().length === 0) return;

  const range = rangeOf(plan, change);
  if (range === undefined) return;

  handlers.run({ op: "setText", slide, range, text: change.text });
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
