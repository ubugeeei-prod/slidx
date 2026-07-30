/**
 * The resize handles, drawn over the canvas rather than in it.
 *
 * Same rule as the arrange overlay and for the same reason: the moment the editor
 * puts an element inside the deck page, the preview stops being the page the
 * build emits. So these are fixed-position elements in the editor's own document,
 * at rectangles read from inside the frame.
 *
 * Flat, like everything slidx draws. The handle is a hairline bar rather than a
 * square with a shadow — a shadow is the first thing a projector turns to mud,
 * and this sits beside the author's editor all day.
 */

export const RESIZE_STYLESHEET = `
.slidx-resize {
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 3;
}

/*
 * The handle is the only thing here a pointer can reach.
 *
 * The overlay covers the canvas, so anything in it that took events would stop a
 * line of the slide from being clicked — and a line of the slide is edited in
 * place.
 */
.slidx-resize-grip {
  position: fixed;
  display: grid;
  place-items: center;
  padding: 0;
  min-height: 0;
  pointer-events: auto;
  border: 0;
  background: transparent;
  cursor: ew-resize;
}

.slidx-resize-grip::before {
  content: "";
  width: 2px;
  height: 60%;
  min-height: var(--slidx-e-tight);
  background: var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
}

.slidx-resize-grip:hover::before,
.slidx-resize-grip[data-moving="true"]::before {
  background: var(--slidx-e-accent);
}

/*
 * The share the file will say, snapped, while the pointer is still moving.
 *
 * Drawn as the box rather than as a number floating near the cursor: what the
 * author is choosing is a width, and a label alone would make them imagine it.
 */
.slidx-resize-ghost {
  position: fixed;
  border: var(--slidx-e-hairline) solid var(--slidx-e-accent);
  border-radius: var(--slidx-e-radius);
  pointer-events: none;
}

.slidx-resize[data-resizing="false"] .slidx-resize-ghost { display: none; }

/*
 * The name, next to the box it describes.
 *
 * The word that will be in the diff is the word on screen. A percentage here
 * would be a second vocabulary for one thing, and the number is exactly what
 * this feature refuses to let anybody write.
 */
.slidx-resize-name {
  position: fixed;
  padding: 0 var(--slidx-e-tight);
  color: var(--slidx-e-canvas);
  background: var(--slidx-e-accent);
  border-radius: var(--slidx-e-radius);
  font-size: 11px;
  white-space: nowrap;
  pointer-events: none;
}

.slidx-resize[data-resizing="false"] .slidx-resize-name { display: none; }

.slidx-resize-status {
  position: fixed;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}
`;

/** Puts the overlay's stylesheet into a document, once. */
export function applyResizeStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-resize]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-resize", "");
  style.textContent = RESIZE_STYLESHEET;
  document.head.append(style);
}
