import { SHORTCUT_STYLES } from "./shortcut-styles";

/**
 * The chrome around the deck.
 *
 * The deck is the colourful thing on the screen. This sits next to the author's
 * editor and their terminal all day, so it recedes: the system font stack, one
 * hairline instead of a border, flat surfaces, no gradient, and no shadow that
 * is decoration rather than depth. The only motion is the kind that says a
 * state changed, and it is off for anyone who asked for less of it.
 *
 * Both colour schemes come from one set of custom properties, because the room
 * an author works in is not knowable from here — the same reason the built-in
 * deck themes ship both.
 *
 * The accent is the brand's `signal`, copied from `assets/brand/tokens.json` and
 * checked against it by a test rather than trusted. It used to be a code host's
 * brand blue in light and an editor theme's in dark, which is the same borrowed
 * palette problem `scripts/check-borrowed.mjs` now fails the build over.
 */

export const STYLESHEET = `
:root {
  --slidx-e-canvas: #ffffff;
  --slidx-e-surface: #f7f7f8;
  --slidx-e-text: #16181d;
  --slidx-e-muted: #6a6f7a;
  --slidx-e-line: #e2e3e7;
  --slidx-e-accent: #01489f;
  --slidx-e-error: #b42318;
  --slidx-e-warning: #9a6700;
  --slidx-e-hairline: 1px;
  --slidx-e-radius: 4px;

  /*
   * Both stacks name a CJK family, which the deck themes have been held to all
   * along and the chrome was not. Without one a Japanese slide title in the
   * outline falls back to a Latin-only face and renders as tofu — in the panel
   * whose whole job is telling an author which slide they are on.
   */
  --slidx-e-font-sans: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto,
    "Hiragino Sans", "Noto Sans JP", "Yu Gothic UI", sans-serif;
  --slidx-e-font-mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas,
    "Noto Sans Mono CJK JP", monospace;

  /*
   * One rhythm, in multiples of a 4px step.
   *
   * The chrome used to reach for 4, 5, 6, 8, 10, 12, 14 and 18 pixels in
   * different places, which is what makes an interface feel assembled rather
   * than designed: nothing lines up with anything, and no gap means the same as
   * any other gap. Four stops cover every distance this tool actually needs.
   */
  --slidx-e-tight: 4px;
  --slidx-e-snug: 8px;
  --slidx-e-gap: 16px;
  --slidx-e-loose: 24px;

  /*
   * The smallest a control may be.
   *
   * Not the 44px touch guideline: this is a dense desktop tool driven by a
   * mouse and a keyboard, and 44px rows would put a third as much of the deck
   * on screen. 28 is the smallest a pointer reliably hits without the row
   * becoming a target you have to aim at, and every interactive element here
   * meets it whether or not its ink fills it.
   */
  --slidx-e-hit: 28px;

  color-scheme: light dark;
}

@media (prefers-color-scheme: dark) {
  :root {
    --slidx-e-canvas: #16181d;
    --slidx-e-surface: #1c1f26;
    --slidx-e-text: #e8eaed;
    --slidx-e-muted: #9aa0ac;
    --slidx-e-line: #2a2e37;
    --slidx-e-accent: #a5c9ff;
    --slidx-e-error: #f5776a;
    --slidx-e-warning: #d9a441;
  }
}

*, *::before, *::after { box-sizing: border-box; }

/*
 * Selection and scrollbars, chosen rather than left to the browser.
 *
 * A default selection is the operating system's accent, which is whatever the
 * author last picked in their settings — so the one moment the tool asserts a
 * colour of its own is a colour it did not choose. The accent at low alpha keeps
 * the selected text legible, which a solid fill does not.
 */
::selection {
  background: color-mix(in srgb, var(--slidx-e-accent) 26%, transparent);
}

* {
  scrollbar-width: thin;
  scrollbar-color: var(--slidx-e-line) transparent;
}

body {
  margin: 0;
  height: 100vh;
  background: var(--slidx-e-canvas);
  color: var(--slidx-e-text);
  font-family: var(--slidx-e-font-sans);
  font-size: 13px;
  /*
   * Looser than a Latin-only interface would need.
   *
   * This editor is used to write Japanese decks. CJK glyphs fill their em box
   * where Latin lowercase fills about half of it, so lines set at 1.5 that look
   * airy in English look cramped in Japanese — and an outline of slide titles
   * is the one place a whole panel is set in it. 1.65 is the smallest that
   * reads as comfortable in both.
   */
  line-height: 1.65;
}

.slidx-editor {
  display: grid;
  height: 100vh;
  grid-template-columns: 232px minmax(0, 1fr) 296px;
  grid-template-rows: minmax(0, 1fr) auto;
  grid-template-areas: "outline canvas inspector" "findings findings findings";
}

.slidx-outline { grid-area: outline; border-right: var(--slidx-e-hairline) solid var(--slidx-e-line); }
.slidx-canvas { grid-area: canvas; }
.slidx-inspector { grid-area: inspector; border-left: var(--slidx-e-hairline) solid var(--slidx-e-line); }
.slidx-diagnostics { grid-area: findings; border-top: var(--slidx-e-hairline) solid var(--slidx-e-line); }

.slidx-outline, .slidx-canvas, .slidx-inspector {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

/*
 * One row of chrome per panel, at one height.
 *
 * Panels whose headers disagree by a pixel read as three tools bolted together
 * rather than as one.
 */
.slidx-panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--slidx-e-gap);
  height: 34px;
  padding: 0 var(--slidx-e-gap);
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-panel-head h2 {
  margin: 0;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--slidx-e-muted);
}

button {
  font: inherit;
  color: var(--slidx-e-text);
  background: transparent;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  /* Ink can be small; the target cannot. */
  min-height: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-snug);
  cursor: pointer;
}

button:hover { background: var(--slidx-e-surface); }

/*
 * The focus ring.
 *
 * Deliberate and identical everywhere, because a keyboard user's only way of
 * knowing where they are is this ring. Two pixels rather than one — a hairline
 * ring disappears against a hairline border — and offset by two so it reads as a
 * ring around the control rather than as a thicker edge on it.
 *
 * :focus-visible rather than :focus, so clicking a button does not leave a
 * ring behind that a mouse user has to wonder about.
 */
button:focus-visible,
[contenteditable]:focus-visible,
input:focus-visible,
textarea:focus-visible,
[tabindex]:focus-visible {
  outline: 2px solid var(--slidx-e-accent);
  outline-offset: 2px;
  /* Above a neighbouring row's background, which would otherwise clip it. */
  position: relative;
  z-index: 1;
}

/* Outline. */

.slidx-outline-list {
  margin: 0;
  padding: var(--slidx-e-snug);
  list-style: none;
  overflow-y: auto;
}

.slidx-outline-row {
  display: flex;
  align-items: center;
  min-height: var(--slidx-e-hit);
  padding-right: var(--slidx-e-tight);
  border-radius: var(--slidx-e-radius);
}

.slidx-outline-open {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
  padding: 0 var(--slidx-e-snug);
  border: 0;
  border-radius: var(--slidx-e-radius);
  text-align: left;
}

.slidx-outline-open:hover { background: transparent; }

.slidx-outline-row:hover { background: var(--slidx-e-surface); }

/*
 * The current slide, said twice.
 *
 * Hover and selection used to be the same background, so an author moving the
 * mouse through the outline could not tell which slide they were actually on.
 * The rule on the leading edge is the part that does not move under a pointer.
 */
.slidx-outline-row[aria-current="true"] {
  background: var(--slidx-e-surface);
  border-left: 2px solid var(--slidx-e-accent);
  padding-left: 0;
}

.slidx-outline-row[aria-current="true"] .slidx-outline-open { padding-left: 6px; }
.slidx-outline-row[aria-current="true"] .slidx-outline-title { font-weight: 600; }

.slidx-outline-number {
  min-width: 1.6em;
  color: var(--slidx-e-muted);
  font-variant-numeric: tabular-nums;
  text-align: right;
}

.slidx-outline-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--slidx-e-warning);
}

.slidx-outline-row[data-severity="error"] .slidx-dot { background: var(--slidx-e-error); }

/*
 * Revealed by hover *or* focus, and never removed from the layout.
 *
 * It used to be opacity: 0 with only a hover rule to bring it back, which
 * makes it unreachable by keyboard in the literal sense: tabbing to it moved
 * focus to something invisible, and the focus ring was drawn on a control nobody
 * could see. Opacity rather than display: none for the same reason the demo
 * switch uses display: none and this does not — here the row must not change
 * width when a pointer enters it.
 */
.slidx-outline-duplicate,
.slidx-outline-remove {
  border: 0;
  min-width: var(--slidx-e-hit);
  min-height: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-tight);
  color: var(--slidx-e-muted);
  opacity: 0;
}

.slidx-outline-row:hover .slidx-outline-duplicate,
.slidx-outline-row:hover .slidx-outline-remove,
.slidx-outline-duplicate:focus-visible,
.slidx-outline-remove:focus-visible {
  opacity: 1;
}

/* Canvas. */

.slidx-canvas-stage {
  flex: 1;
  min-height: 0;
  display: grid;
  padding: var(--slidx-e-loose);
}

.slidx-canvas-stage > * { grid-area: 1 / 1; }
.slidx-canvas-stage[data-editing="true"] .slidx-canvas-frame { visibility: hidden; }
.slidx-canvas-stage:not([data-editing="true"]) .slidx-canvas-source { visibility: hidden; }

.slidx-canvas-frame {
  width: 100%;
  height: 100%;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
}

.slidx-canvas-source {
  width: 100%;
  height: 100%;
  resize: none;
  padding: var(--slidx-e-loose);
  color: inherit;
  background: var(--slidx-e-surface);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  font-family: var(--slidx-e-font-mono);
  font-size: 12px;
  tab-size: 2;
}

/* Inspector. */

.slidx-inspector-panels { padding: var(--slidx-e-loose); overflow-y: auto; }

.slidx-group + .slidx-group {
  margin-top: var(--slidx-e-loose);
  padding-top: var(--slidx-e-gap);
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-group h3 {
  margin: 0 0 var(--slidx-e-snug);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--slidx-e-muted);
}

.slidx-field {
  display: grid;
  grid-template-columns: 80px minmax(0, 1fr);
  align-items: center;
  gap: var(--slidx-e-gap);
  margin-bottom: var(--slidx-e-snug);
}
.slidx-field-name { color: var(--slidx-e-muted); }

input, textarea {
  width: 100%;
  min-height: var(--slidx-e-hit);
  font: inherit;
  color: inherit;
  background: var(--slidx-e-canvas);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  padding: var(--slidx-e-snug) var(--slidx-e-gap);
}

/*
 * A field the author has not filled says what belongs in it, quietly.
 *
 * Browsers set placeholder text at the same weight as a value, which makes an
 * empty field look filled in at a glance.
 */
::placeholder { color: var(--slidx-e-muted); opacity: 1; }

textarea { resize: vertical; font-size: 12px; }

.slidx-remove-mark {
  margin-top: var(--slidx-e-tight);
  color: var(--slidx-e-muted);
}

.slidx-hint { margin: 0; color: var(--slidx-e-muted); }
.slidx-selected {
  margin: 0 0 var(--slidx-e-snug);
  padding: var(--slidx-e-gap);
  background: var(--slidx-e-surface);
  border-radius: var(--slidx-e-radius);
  font-family: var(--slidx-e-font-mono);
  font-size: 12px;
}

/* Diagnostics. */

.slidx-diagnostics { max-height: 26vh; overflow-y: auto; }

/*
 * Nothing wrong is a state, not an absence.
 *
 * The panel used to vanish, which moves every other panel by its height the
 * moment an author fixes the last diagnostic — the layout jumping is how they
 * find out. It now keeps one row and says so, and the row is the calm one: a
 * linter that is quiet should look quiet rather than look broken.
 */
.slidx-diagnostics[data-empty="true"] .slidx-findings { display: none; }

.slidx-diagnostics[data-empty="true"]::after {
  content: attr(data-empty-label);
  display: block;
  min-height: var(--slidx-e-hit);
  padding: var(--slidx-e-snug) var(--slidx-e-gap);
  color: var(--slidx-e-muted);
}

.slidx-findings { margin: 0; padding: var(--slidx-e-snug) var(--slidx-e-gap); list-style: none; }

.slidx-finding {
  display: flex;
  gap: var(--slidx-e-snug);
  align-items: baseline;
  padding: var(--slidx-e-tight) 0;
}

.slidx-finding[role="button"] { cursor: default; }
.slidx-finding-where { min-width: 5.5em; color: var(--slidx-e-muted); }
.slidx-finding-message { flex: 1; }
.slidx-finding-code { color: var(--slidx-e-muted); font-size: 12px; }
.slidx-finding[data-severity="error"] .slidx-finding-where { color: var(--slidx-e-error); }
.slidx-finding[data-severity="warning"] .slidx-finding-where { color: var(--slidx-e-warning); }

/*
 * The only motion in the tool, and it is reporting a state rather than
 * decorating one: a row that just changed, and a control that just took focus.
 */
@media (prefers-reduced-motion: no-preference) {
  .slidx-outline-row, button, .slidx-outline-remove {
    transition: background 90ms linear, opacity 90ms linear;
  }
}

/*
 * The focus ring never animates, at any setting.
 *
 * It is the answer to "where am I", and an answer that fades in arrives after
 * the question. Excluded from the transition above rather than switched off
 * here, but stated so nobody adds it back.
 */

/*
 * Presence, and why it floats rather than taking a row of the grid.
 *
 * An author working alone is the normal case and must see no new chrome at all,
 * so this is hidden until somebody else connects — and a panel that appears and
 * disappears from the layout would move the canvas under the author's cursor.
 * Floating costs nothing when it is not there.
 */
.slidx-presence {
  position: fixed;
  top: 6px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  gap: 8px;
  align-items: center;
  max-width: 60vw;
  padding: 2px 8px;
  background: var(--slidx-e-surface);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  font-size: 12px;
}

.slidx-presence[data-empty="true"] { display: none; }
.slidx-presence-label { color: var(--slidx-e-muted); }
.slidx-presence-list { display: flex; gap: 8px; margin: 0; padding: 0; list-style: none; overflow-x: auto; }
.slidx-presence-who { display: flex; gap: 4px; align-items: baseline; white-space: nowrap; }
.slidx-presence-who[data-local="true"] .slidx-presence-name { color: var(--slidx-e-accent); }
.slidx-presence-where { color: var(--slidx-e-muted); }
.slidx-presence-role { color: var(--slidx-e-muted); font-size: 11px; }

${SHORTCUT_STYLES}
`;

/** Puts the chrome's stylesheet into a document, once. */
export function applyStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-editor]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-editor", "");
  style.textContent = STYLESHEET;
  document.head.append(style);
}
