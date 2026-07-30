/**
 * The storyboard's own chrome.
 *
 * Its own stylesheet rather than a section of the editor's, for the same reason
 * the surface is its own module: this is the one part of the tool whose colour
 * carries meaning. Everywhere else the chrome recedes so the deck can be the
 * colourful thing on the screen; here the accent *is* the talk, and the muted
 * tone *is* the slack, so those two tokens are load-bearing and belong next to
 * the code that depends on them.
 *
 * Every colour comes from the editor's custom properties, so both schemes work
 * without a second palette.
 */

export const STORYBOARD_STYLESHEET = `
/*
 * Fixed rather than a fourth column in the editor's grid.
 *
 * Working out where a talk's time goes is a whole-deck job, and the panel that
 * would fit next to a canvas is not wide enough to draw twenty minutes in.
 */
.slidx-storyboard {
  position: fixed;
  inset: 0;
  z-index: 4;
  pointer-events: none;
}

.slidx-storyboard > * { pointer-events: auto; }

/*
 * In the one row of chrome the editor already has, at the height every panel
 * head shares. The outline's row has "Add slide" and the canvas's has
 * "Markdown"; this is the third of the same thing, and a control floating over
 * the deck would be the only part of the tool that ignored the layout.
 */
.slidx-sb-launch {
  position: absolute;
  top: 6px;
  right: var(--slidx-e-gap);
}

.slidx-sb-sheet {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  background: var(--slidx-e-canvas);
}

.slidx-sb-sheet[hidden] { display: none; }
.slidx-sb-sheet:focus { outline: none; }

.slidx-sb-plan {
  padding: 14px var(--slidx-e-gap) 16px;
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

/* The number the whole surface exists to state, so it is the largest thing. */
.slidx-sb-summary {
  margin: 0 0 14px;
  font-size: 15px;
  font-variant-numeric: tabular-nums;
}

.slidx-sb-slack, .slidx-sb-untimed {
  margin: 10px 0 0;
  color: var(--slidx-e-muted);
}

/* Nothing to say, so nothing said — rather than a line of empty chrome. */
.slidx-sb-slack:empty, .slidx-sb-untimed:empty { display: none; }

.slidx-sb-hint { margin: 0; color: var(--slidx-e-muted); }

/* The bar. */

.slidx-sb-bar {
  position: relative;
  height: 30px;
  /* Room under it for the label on the slot's end. */
  margin-bottom: 20px;
}

/*
 * The slot, as a band. The slides are drawn on top of it, so a deck that runs
 * long has segments sitting past the band's right edge with nothing under them,
 * which is the overrun and needs no second way of saying so.
 */
.slidx-sb-track {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 0;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-surface);
}

.slidx-sb-bar[data-slot="none"] .slidx-sb-track { border-style: dashed; }

.slidx-sb-segments {
  position: absolute;
  inset: 0;
  display: flex;
}

.slidx-sb-segment {
  height: 100%;
  min-width: 0;
  padding: 0;
  border: 0;
  border-radius: 2px;
  background: var(--slidx-e-accent);
  /* Drawn inside the segment, so a separator never widens one. */
  box-shadow: inset -1px 0 0 var(--slidx-e-canvas);
}

.slidx-sb-segment:hover { background: var(--slidx-e-accent); filter: brightness(1.15); }
.slidx-sb-segment[aria-current="true"] { outline: 2px solid var(--slidx-e-text); outline-offset: -2px; }

/* Slack, in the colour of everything else that is not the talk itself. */
.slidx-sb-segment[data-optional="true"] { background: var(--slidx-e-muted); }

/*
 * Striped where the width came from an estimate rather than a budget the author
 * set. A provisional number that looked identical to a decided one would make
 * the bar more confident than the deck is.
 */
.slidx-sb-segment[data-source="estimate"] {
  background-image: repeating-linear-gradient(
    135deg,
    rgb(255 255 255 / 0.34) 0 3px,
    rgb(255 255 255 / 0) 3px 7px
  );
}

.slidx-sb-slot {
  position: absolute;
  top: 0;
  height: calc(100% + 14px);
  border-left: 1px solid var(--slidx-e-text);
}

.slidx-sb-slot span {
  position: absolute;
  right: 5px;
  bottom: -4px;
  /* Shrink-to-fit inside a zero-width line, so without this it wraps per word. */
  white-space: nowrap;
  color: var(--slidx-e-muted);
  font-variant-numeric: tabular-nums;
}

/* One row per slide. */

.slidx-sb-rows {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 2px var(--slidx-e-gap) 28px;
}

.slidx-sb-slide {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) 170px;
  gap: 12px;
  padding: 8px;
  border-radius: var(--slidx-e-radius);
}

.slidx-sb-slide[aria-current="true"] { background: var(--slidx-e-surface); }
.slidx-sb-slide[data-optional="true"] { box-shadow: inset 2px 0 0 var(--slidx-e-muted); }

.slidx-sb-grip {
  color: var(--slidx-e-muted);
  cursor: grab;
  user-select: none;
  line-height: 1.2;
}

.slidx-sb-main { min-width: 0; }

.slidx-sb-jump {
  display: flex;
  align-items: baseline;
  gap: 8px;
  width: 100%;
  padding: 0 0 5px;
  border: 0;
  background: transparent;
  text-align: left;
}

.slidx-sb-number {
  min-width: 1.6em;
  color: var(--slidx-e-muted);
  font-variant-numeric: tabular-nums;
  text-align: right;
}

.slidx-sb-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 600;
}

.slidx-sb-message { min-height: 3.6em; }

.slidx-sb-unwritten {
  margin: 0 0 5px;
  color: var(--slidx-e-muted);
}

.slidx-sb-time {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.slidx-sb-budget { font-variant-numeric: tabular-nums; }

.slidx-sb-length {
  margin: 0;
  color: var(--slidx-e-muted);
  font-variant-numeric: tabular-nums;
}

/* A number the author decided reads as decided. */
.slidx-sb-length[data-source="budget"] { color: var(--slidx-e-text); }

.slidx-sb-cut {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--slidx-e-muted);
  cursor: pointer;
}

/* The editor's inputs are full width, which would make a checkbox a bar. */
.slidx-sb-cut input {
  width: auto;
  padding: 0;
  cursor: pointer;
}

/*
 * The gap between two rows is the drop target, so "put it here" is aimed at the
 * place it will land rather than at the row above or below it.
 */
.slidx-sb-drop {
  height: 8px;
  border-radius: 2px;
}

.slidx-sb-drop[data-over="true"] { background: var(--slidx-e-accent); }

@media (prefers-reduced-motion: no-preference) {
  .slidx-sb-drop, .slidx-sb-segment { transition: background 90ms linear, filter 90ms linear; }
}
`;

/** Puts the storyboard's stylesheet into a document, once. */
export function applyStoryboardStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-storyboard]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-storyboard", "");
  style.textContent = STORYBOARD_STYLESHEET;
  document.head.append(style);
}
