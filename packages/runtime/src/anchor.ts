/**
 * Resolving step anchors to the elements they stage.
 *
 * The compiler in `slidx_core` cannot know what HTML a Markdown renderer will
 * produce, so it does not try. It leaves an empty marker span where the author
 * put the marker, and this module works out what that marker referred to once
 * the DOM exists.
 *
 * Three positions are possible, and each maps to one rule. They are checked in
 * order, because an anchor at the end of a list item satisfies the third rule
 * too — but staging the list instead of the item would be wrong.
 *
 * 1. **Alone in a block.** The anchor's parent has no text of its own, so the
 *    author wrote the marker on its own line to stage the block above it. The
 *    staged element is the previous element sibling, and the empty wrapper is
 *    removed.
 * 2. **Inside a list item or table row.** The anchor has an `<li>` or `<tr>`
 *    ancestor, so the author was staging that one item — not the whole list,
 *    which is what rule 3 would give.
 * 3. **Anywhere else.** The staged element is the closest ancestor that is a
 *    direct child of the slide root: the block the marker is written in.
 *
 * The same three cases are documented from the authoring side in
 * `crates/slidx_core/src/markers.rs`.
 */

/** Attribute the compiler writes onto every anchor. */
export const ANCHOR_ATTRIBUTE = "data-slidx-step";

/**
 * Every anchor in one slide, in document order.
 *
 * Scoped to `root` because anchor ids restart on each slide — keeping them
 * slide-local is what lets an edit to one slide leave every other slide's
 * output byte-identical.
 */
export function findAnchors(root: Element): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(`[${ANCHOR_ATTRIBUTE}]`));
}

/**
 * Returns the element an anchor stages, or `null` if it refers to nothing.
 *
 * Null is a real answer, not a failure: a marker written before any content
 * has nothing to stage, and the caller should drop that step rather than
 * staging the whole slide.
 */
export function resolveAnchor(root: Element, anchor: HTMLElement): HTMLElement | null {
  const parent = anchor.parentElement;
  if (!parent) return null;

  if (parent !== root && isWrapperOnly(parent)) {
    const staged = previousElement(parent);
    parent.remove();
    return staged;
  }

  const item = anchor.closest<HTMLElement>("li, tr");
  if (item && root.contains(item)) return item;

  return closestBlock(root, anchor);
}

/**
 * True when an element holds the anchor and nothing else of substance.
 *
 * Whitespace does not count: a Markdown renderer is free to put newlines and
 * indentation around the span, and treating that as content would silently
 * switch the anchor to rule 3.
 */
function isWrapperOnly(element: Element): boolean {
  if ((element.textContent ?? "").trim() !== "") return false;

  return Array.from(element.children).every((child) => child.hasAttribute(ANCHOR_ATTRIBUTE));
}

function previousElement(element: Element): HTMLElement | null {
  const previous = element.previousElementSibling;
  return previous instanceof HTMLElement ? previous : null;
}

/** The anchor's closest ancestor that sits directly inside the slide. */
function closestBlock(root: Element, anchor: HTMLElement): HTMLElement | null {
  let current: HTMLElement | null = anchor;

  while (current && current.parentElement && current.parentElement !== root) {
    current = current.parentElement;
  }

  return current && current !== anchor ? current : null;
}
