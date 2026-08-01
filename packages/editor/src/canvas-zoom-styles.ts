/** The compact viewport control and the scrollable, still-real slide frame. */

export const CANVAS_ZOOM_STYLESHEET = `
.slidx-canvas-zoom {
  display: flex;
  align-items: center;
}

.slidx-canvas-zoom[hidden] {
  display: none;
}

.slidx-canvas-zoom button {
  min-width: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-tight);
  border-color: var(--slidx-e-line);
  border-radius: 0;
  color: var(--slidx-e-muted);
  font-variant-numeric: tabular-nums;
}

.slidx-canvas-zoom .slidx-canvas-zoom-out { border-radius: var(--slidx-e-radius) 0 0 var(--slidx-e-radius); }
.slidx-canvas-zoom .slidx-canvas-zoom-in { border-radius: 0 var(--slidx-e-radius) var(--slidx-e-radius) 0; }

.slidx-canvas-zoom .slidx-canvas-zoom-value {
  min-width: calc(var(--slidx-e-hit) + var(--slidx-e-gap));
  margin-inline: calc(var(--slidx-e-hairline) * -1);
  color: var(--slidx-e-text);
  font-size: 11px;
}

.slidx-canvas-zoom button:hover:not(:disabled) {
  position: relative;
  z-index: 1;
  border-color: var(--slidx-e-muted);
}

.slidx-canvas-zoom button:disabled {
  color: color-mix(in srgb, var(--slidx-e-muted) 42%, transparent);
  cursor: default;
}

.slidx-canvas-stage {
  overflow: hidden;
  overscroll-behavior: contain;
}

.slidx-canvas-stage:not([data-zoom="fit"]) {
  overflow: auto;
  scrollbar-gutter: stable both-edges;
}

.slidx-canvas-stage .slidx-canvas-frame {
  place-self: start;
  width: var(--slidx-e-canvas-zoom, 100%);
  height: var(--slidx-e-canvas-zoom, 100%);
  max-width: none;
  max-height: none;
}

/*
 * Canvas overlays live in the editor document, not in the deck iframe. Clip
 * their full-window drawing layers to the visible viewport so a handle on a
 * panned-off block never appears over the outline, appbar, or notes.
 */
.slidx-editor > :is(.slidx-arrange, .slidx-resize, .slidx-freeform, .slidx-beacons) {
  clip-path: inset(
    var(--slidx-e-viewport-top, 0)
    var(--slidx-e-viewport-right, 0)
    var(--slidx-e-viewport-bottom, 0)
    var(--slidx-e-viewport-left, 0)
  );
}

@media (max-width: 48em) {
  .slidx-canvas-zoom .slidx-canvas-zoom-value {
    min-width: var(--slidx-e-hit);
    font-size: 10px;
  }
}
`;

export function applyCanvasZoomStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-canvas-zoom]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-canvas-zoom", "");
  style.textContent = CANVAS_ZOOM_STYLESHEET;
  document.head.append(style);
}
