/** The selected frame, its eight handles, and the guides shown while it moves. */

export const FREEFORM_STYLESHEET = `
.slidx-freeform {
  position: fixed;
  inset: 0;
  z-index: 7;
  pointer-events: none;
}

.slidx-freeform[data-active="false"] > :not(.slidx-freeform-status) { display: none; }

.slidx-freeform-frame,
.slidx-freeform-guide,
.slidx-freeform-label {
  position: fixed;
}

.slidx-freeform-frame,
.slidx-freeform-guide,
.slidx-freeform-label { pointer-events: none; }

.slidx-freeform-frame {
  outline: 1px solid color-mix(in srgb, var(--slidx-e-accent) 20%, transparent);
  outline-offset: 0;
}

.slidx-freeform-move,
.slidx-freeform-handle {
  position: fixed;
  min-width: 0;
  min-height: 0;
  padding: 0;
  border: 0;
  background: transparent;
  pointer-events: auto;
}

.slidx-freeform-move:focus-visible,
.slidx-freeform-handle:focus-visible {
  outline: 0;
}

.slidx-freeform:has(.slidx-freeform-move:hover, .slidx-freeform-handle:hover) .slidx-freeform-frame,
.slidx-freeform:has(.slidx-freeform-move:focus-visible, .slidx-freeform-handle:focus-visible) .slidx-freeform-frame,
.slidx-freeform[data-manipulating="true"] .slidx-freeform-frame {
  outline-color: var(--slidx-e-accent);
}

/*
 * The move grip, marked the way the eight handles are.
 *
 * It had no mark at all, which made it an invisible button — and it was the
 * full width of the block, so the invisible part was the part an author was
 * trying to click on. A hit target larger than its mark is ordinary; a hit
 * target with no mark is a trap.
 *
 * A bar rather than the handles' square dot, because it does a different job:
 * those eight change the size, this one changes the place.
 */
.slidx-freeform-move {
  cursor: move;
  display: grid;
  place-items: center;
}

.slidx-freeform-move::before {
  content: "";
  width: 16px;
  height: 3px;
  border-radius: 2px;
  background: var(--slidx-e-line);
  opacity: 0.32;
}

.slidx-freeform-move:hover::before,
.slidx-freeform-move:focus-visible::before,
.slidx-freeform[data-manipulating="true"] .slidx-freeform-move::before {
  background: var(--slidx-e-accent);
  opacity: 1;
}

.slidx-freeform-handle {
  display: grid;
  place-items: center;
}

.slidx-freeform-handle::before {
  content: "";
  width: 5px;
  height: 5px;
  border: 1px solid var(--slidx-e-line);
  border-radius: 2px;
  background: var(--slidx-e-canvas);
  opacity: 0.32;
}

.slidx-freeform-handle:hover::before,
.slidx-freeform-handle:focus-visible::before {
  border-color: var(--slidx-e-canvas);
  background: var(--slidx-e-accent);
  opacity: 1;
}

.slidx-freeform[data-manipulating="true"] .slidx-freeform-handle::before {
  border-color: var(--slidx-e-accent);
  opacity: 1;
}

.slidx-freeform-handle[data-handle="n"],
.slidx-freeform-handle[data-handle="s"] { cursor: ns-resize; }
.slidx-freeform-handle[data-handle="e"],
.slidx-freeform-handle[data-handle="w"] { cursor: ew-resize; }
.slidx-freeform-handle[data-handle="ne"],
.slidx-freeform-handle[data-handle="sw"] { cursor: nesw-resize; }
.slidx-freeform-handle[data-handle="nw"],
.slidx-freeform-handle[data-handle="se"] { cursor: nwse-resize; }

.slidx-freeform-guide {
  background: var(--slidx-e-accent);
  opacity: 0.72;
}

.slidx-freeform-guide[data-kind="safe"] {
  background: var(--slidx-e-warning);
}

.slidx-freeform-label {
  padding: 2px 8px;
  color: var(--slidx-e-canvas);
  background: var(--slidx-e-accent);
  border-radius: var(--slidx-e-radius);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  line-height: 1.5;
  white-space: nowrap;
}

.slidx-freeform[data-manipulating="false"] .slidx-freeform-guide,
.slidx-freeform[data-manipulating="false"] .slidx-freeform-label {
  display: none;
}

.slidx-freeform-status {
  position: fixed;
  width: 1px;
  height: 1px;
  margin: -1px;
  overflow: hidden;
  clip-path: inset(50%);
}
`;

export function applyFreeformStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-freeform]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-freeform", "");
  style.textContent = FREEFORM_STYLESHEET;
  document.head.append(style);
}
