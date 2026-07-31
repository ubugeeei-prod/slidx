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
  pointer-events: none;
}

.slidx-freeform-frame {
  outline: 1px solid color-mix(in srgb, var(--slidx-e-accent) 46%, transparent);
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

.slidx-freeform:has(.slidx-freeform-move:focus-visible) .slidx-freeform-frame {
  outline-color: var(--slidx-e-accent);
}

.slidx-freeform-move {
  cursor: move;
}

.slidx-freeform-handle {
  display: grid;
  place-items: center;
}

.slidx-freeform-handle::before {
  content: "";
  width: 7px;
  height: 7px;
  border: 1px solid var(--slidx-e-accent);
  border-radius: 2px;
  background: var(--slidx-e-canvas);
}

.slidx-freeform-handle:focus-visible::before {
  border-color: var(--slidx-e-canvas);
  background: var(--slidx-e-accent);
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
