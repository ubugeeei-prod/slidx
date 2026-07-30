/**
 * Bringing a change to the canvas without reloading the page it is on.
 *
 * An iframe that reloads throws away the caret, the selection and the scroll
 * position, which is the whole cost of editing on the canvas: an author changes
 * a word, the frame blinks, and they are somewhere else on the slide. The
 * gesture that made the change is the one that undoes its own context.
 *
 * # Why the new markup still comes from the route
 *
 * The canvas is an iframe on the deck's own page and has to stay that — it is
 * the reason a preview cannot disagree with the build. So this does not render
 * anything. It asks the same URL the frame is already showing for the page it
 * now serves, and moves the slide element across. What lands in the frame is
 * byte for byte what the route serves, which is byte for byte what the build
 * emits; the only thing that did not happen is the document being thrown away.
 *
 * # Why there is no channel
 *
 * The presenter view and the audience screen need one because they are two
 * documents that cannot reach each other. The editor and its canvas are not:
 * the frame is same-origin, the editor already reads its layout to draw the
 * overlays, and it already knows the deck changed — the answer to the operation
 * it sent, or the announcement from the watcher when somebody edited the file in
 * another editor, is the same state update either way. A transport between two
 * halves of one page would be a second way for them to disagree.
 *
 * # When it falls back to a reload
 *
 * **A slide with steps.** Staging is bound to elements once, when the slide's
 * script runs — anchors are resolved by mutating the DOM, so re-resolving them
 * would give a different answer. Swapping the markup under a bound stage leaves
 * it writing to nodes that are no longer on the page, and the slide would show
 * every stop at once. Re-running the script instead would leave two keyboard
 * handlers and two channels behind, which is worse than a reload.
 *
 * **Anything that does not answer.** A route that fails, a page that is not a
 * slide, a frame that has not loaded. Reloading is always correct and only
 * costs the thing this module exists to save.
 */

/** What is worth putting back after the slide has been replaced. */
export interface Caret {
  /** The block the caret was in, by its index in source order. */
  block: number;
  /** Which editable line of that block, counted in document order. */
  line: number;
  /** How far into the line's text, counted in the text a reader sees. */
  at: number;
}

/** Whether the page in the frame can be brought up to date where it stands. */
export function canPatch(frame: HTMLIFrameElement, stopCount: number): boolean {
  // More than one stop means a stage is bound to the elements about to be
  // replaced, and a stage bound to nothing shows every stop at once.
  if (stopCount > 1) return false;

  // A frame that has not loaded a slide yet has nothing to replace, and the
  // first render of the editor is exactly that.
  return frame.contentDocument?.querySelector(".slidx-slide") != null;
}

/**
 * Replaces the slide in a frame with the one the route now serves.
 *
 * `false` when the page could not be read or does not hold a slide, which is a
 * caller's cue to reload rather than to leave the canvas showing the deck as it
 * was a keystroke ago.
 */
export async function patch(
  frame: HTMLIFrameElement,
  route: string,
  send: typeof globalThis.fetch = globalThis.fetch.bind(globalThis),
): Promise<boolean> {
  const showing = frame.contentDocument?.querySelector(".slidx-slide");
  if (!showing) return false;

  let served: Element | null = null;
  try {
    // No store, because the deck changes under the editor constantly and a
    // cached answer is a wrong one within a keystroke.
    const response = await send(route, { cache: "no-store" });
    if (!response.ok) return false;

    const page = new DOMParser().parseFromString(await response.text(), "text/html");
    served = page.querySelector(".slidx-slide");
  } catch {
    return false;
  }

  if (!served) return false;

  showing.replaceWith(frame.contentDocument!.importNode(served, true));
  return true;
}

/**
 * Where the caret is in the slide, in terms that survive the markup changing.
 *
 * The block's index and the line's position in it rather than the element,
 * because the element is about to be replaced by one the server rendered.
 */
export function caretIn(document: Document): Caret | undefined {
  const active = document.activeElement;
  if (!active?.hasAttribute("contenteditable")) return undefined;

  const block = Number(active.closest("[data-slidx-block]")?.getAttribute("data-slidx-block"));
  if (!Number.isInteger(block)) return undefined;

  return { block, line: lineIn(active), at: offsetIn(document, active) };
}

/**
 * Puts the caret back where it was.
 *
 * Selecting a collapsed range rather than only focusing the element: a
 * `contenteditable` that is focused with no range puts the caret at its start,
 * so an author who was mid-sentence would find themselves at the beginning of
 * the line — a smaller version of the same complaint.
 */
export function restore(document: Document, line: Element, at: number): void {
  (line as HTMLElement).focus?.();

  const found = walk(line, at);
  if (!found) return;

  const range = document.createRange();
  range.setStart(found.node, found.offset);
  range.collapse(true);

  const selection = document.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
}

/** Which editable line of its own block an element is. */
function lineIn(active: Element): number {
  const block = active.closest("[data-slidx-block]");
  if (!block) return 0;

  return [...block.querySelectorAll("[contenteditable]")].indexOf(active);
}

/** How far into an element's text the caret is. */
function offsetIn(document: Document, active: Element): number {
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0) return 0;

  const range = selection.getRangeAt(0).cloneRange();
  const measured = document.createRange();
  measured.selectNodeContents(active);
  measured.setEnd(range.startContainer, range.startOffset);

  return measured.toString().length;
}

/** The text node an offset into an element's text falls in. */
function walk(line: Element, at: number): { node: Node; offset: number } | undefined {
  let seen = 0;

  const find = (node: Node): { node: Node; offset: number } | undefined => {
    for (const child of node.childNodes) {
      if (child.nodeType === 3) {
        const length = (child.textContent ?? "").length;
        if (at <= seen + length) return { node: child, offset: at - seen };

        seen += length;
        continue;
      }

      const found = find(child);
      if (found) return found;
    }

    return undefined;
  };

  return find(line);
}
