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
.slidx-freeform-label,
.slidx-freeform-color {
  position: fixed;
}

.slidx-freeform-frame,
.slidx-freeform-guide,
.slidx-freeform-label { pointer-events: none; }

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

.slidx-freeform-color {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
  min-height: var(--slidx-e-hit);
  padding: var(--slidx-e-tight) var(--slidx-e-snug);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
  color: var(--slidx-e-text);
  pointer-events: auto;
}

.slidx-freeform-color-name {
  color: var(--slidx-e-muted);
  font-size: 11px;
}

.slidx-freeform-color-input {
  width: var(--slidx-e-hit);
  height: var(--slidx-e-hit);
  padding: 2px;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: transparent;
  cursor: pointer;
}

.slidx-freeform-color-input::-webkit-color-swatch-wrapper { padding: 0; }
.slidx-freeform-color-input::-webkit-color-swatch {
  border: 0;
  border-radius: calc(var(--slidx-e-radius) - 1px);
}
.slidx-freeform-color-input::-moz-color-swatch {
  border: 0;
  border-radius: calc(var(--slidx-e-radius) - 1px);
}

.slidx-freeform-color-reset {
  min-height: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-snug);
  border: 0;
  background: transparent;
  color: var(--slidx-e-muted);
  cursor: pointer;
}

.slidx-freeform-color-reset:hover,
.slidx-freeform-color-reset:focus-visible { color: var(--slidx-e-text); }
.slidx-freeform-color-reset:disabled { opacity: 0.42; cursor: default; }
.slidx-freeform[data-manipulating="true"] .slidx-freeform-color { display: none; }

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
