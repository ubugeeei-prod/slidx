/**
 * The timeline panel's own stylesheet.
 *
 * Its own rather than a block appended to [`styles`](./styles), because this
 * panel is the only surface that needs a row of the editor's grid to itself —
 * and because a panel that claims one should be the thing that says so. The
 * custom properties are the ones `styles` already defines, so the two agree
 * about colour and spacing without either importing the other.
 *
 * A cell is 20 pixels of colour and nothing else. The information in this panel
 * is *where* the marks are relative to each other; a cell with a border, a
 * shadow, and a label would say the same thing at four times the width, and a
 * timeline that does not fit its slide on one screen is a timeline nobody reads.
 */

export const TIMELINE_STYLESHEET = `
.slidx-editor {
  grid-template-rows: minmax(0, 1fr) auto auto;
  grid-template-areas: "outline canvas inspector" "timeline timeline timeline" "findings findings findings";
}

.slidx-timeline {
  grid-area: timeline;
  display: flex;
  flex-direction: column;
  max-height: 34vh;
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-timeline .slidx-panel-head { border-bottom: none; }
.slidx-timeline-where { color: var(--slidx-e-muted); font-variant-numeric: tabular-nums; }

/*
 * The playhead. A range input rather than a drawn handle: dragging, the arrow
 * keys, Home and End all work without this file having an opinion, and every
 * one of them lands on a stop that is already compiled.
 */
.slidx-timeline-scrub {
  flex: 1;
  max-width: 320px;
  margin: 0 auto 0 0;
  accent-color: var(--slidx-e-accent);
}

.slidx-timeline-body { padding: 0 var(--slidx-e-gap) var(--slidx-e-gap); overflow: auto; }

.slidx-timeline-generated {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-gap);
  margin: 0 0 8px;
  padding: 6px 8px;
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-surface);
  color: var(--slidx-e-muted);
}

.slidx-timeline-adopt { flex: none; color: var(--slidx-e-text); }

.slidx-timeline-grid { display: flex; flex-direction: column; gap: 2px; }

.slidx-timeline-row, .slidx-timeline-stops {
  display: grid;
  grid-template-columns: 168px repeat(var(--slidx-t-columns), minmax(20px, 40px));
  gap: 2px;
  align-items: center;
}

.slidx-timeline-label {
  overflow: hidden;
  padding-right: 8px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* A row with no name in the source cannot be pointed at by hand, and says so. */
.slidx-timeline-label[data-named="false"] { color: var(--slidx-e-muted); font-style: italic; }

.slidx-timeline-stop {
  padding: 0;
  height: 18px;
  border: none;
  color: var(--slidx-e-muted);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}

.slidx-timeline-stop[aria-current="true"] { color: var(--slidx-e-accent); font-weight: 600; }

.slidx-timeline-cell {
  height: 20px;
  padding: 0;
  border: none;
  border-radius: 2px;
  /*
   * Off screen at this stop. Flat rather than absent, so the grid reads as a
   * grid with a gap in it instead of as rows of different lengths.
   */
  background: var(--slidx-e-surface);
  color: var(--slidx-e-canvas);
  font-size: 11px;
  line-height: 1;
}

/*
 * On screen at this stop. The bar is the point: it comes straight off the
 * compiled frames, so what it shows is what the deck shows.
 */
.slidx-timeline-cell[data-on="true"] { background: var(--slidx-e-line); }

/* Something happens here, which is the only place a colour is spent. */
.slidx-timeline-cell[data-kind] { background: var(--slidx-e-accent); }
.slidx-timeline-cell[data-kind="hide"] { background: var(--slidx-e-muted); }
.slidx-timeline-cell:hover { outline: 1px solid var(--slidx-e-accent); }
.slidx-timeline-cell[data-current="true"] { outline: 2px solid var(--slidx-e-accent); }

/* A generated grid is shown and not edited, so it does not answer a hover. */
.slidx-timeline-grid[data-generated="true"] .slidx-timeline-cell { cursor: default; }
.slidx-timeline-grid[data-generated="true"] .slidx-timeline-cell:hover { outline: none; }

.slidx-timeline-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  padding-top: 8px;
}

.slidx-timeline-what {
  flex: 1;
  overflow: hidden;
  color: var(--slidx-e-muted);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-timeline-actions button[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
  color: var(--slidx-e-accent);
}

.slidx-timeline-actions button:disabled { opacity: 0.45; cursor: default; }
`;

/** Puts the timeline's stylesheet into a document, once. */
export function applyTimelineStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-timeline]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-timeline", "");
  style.textContent = TIMELINE_STYLESHEET;
  document.head.append(style);
}
