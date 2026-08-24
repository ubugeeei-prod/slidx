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
 * The paper, ink, muted, signal, and line colours are the brand palette, copied
 * from `assets/brand/tokens.json` and checked against it rather than trusted.
 * The editor used to carry a separate neutral palette around the brand's signal,
 * which made its identity disappear everywhere except a selected control.
 */

export const STYLESHEET = `
:root {
  --slidx-e-canvas: #f7faff;
  --slidx-e-text: #161b22;
  --slidx-e-muted: #5f656e;
  --slidx-e-line: #d3dae4;
  --slidx-e-accent: #01489f;
  --slidx-e-surface: color-mix(in srgb, var(--slidx-e-line) 24%, var(--slidx-e-canvas));
  --slidx-e-error: #b42318;
  --slidx-e-warning: #9a6700;
  --slidx-e-success: #067647;
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
  /* The lockup's published minimum: below this its set wordmark stops reading. */
  --slidx-e-lockup: 17px;

  color-scheme: light dark;
}

@media (prefers-color-scheme: dark) {
  :root {
    --slidx-e-canvas: #13171e;
    --slidx-e-text: #eff6ff;
    --slidx-e-muted: #979da7;
    --slidx-e-line: #2a2f37;
    --slidx-e-accent: #a5c9ff;
    --slidx-e-error: #f5776a;
    --slidx-e-warning: #d9a441;
    --slidx-e-success: #57c99d;
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
  --slidx-e-outline-width: 232px;
  --slidx-e-inspector-width: 296px;
  display: grid;
  position: relative;
  height: 100vh;
  grid-template-columns: var(--slidx-e-outline-width) minmax(0, 1fr) var(--slidx-e-inspector-width);
  grid-template-rows: 34px minmax(0, 1fr) auto;
  grid-template-areas: "appbar appbar appbar" "outline canvas inspector" "findings findings findings";
}

/*
 * A read capability is a review workspace, not an editor that fails late.
 *
 * The outline and the real rendered canvas remain; authoring-only panels and
 * gestures leave the stage entirely. The server still guards every write, and
 * the session refuses one before transport, so this is an honest affordance
 * rather than the security boundary.
 */
.slidx-editor[data-access="read"] {
  grid-template-columns: 232px minmax(0, 1fr);
  grid-template-areas: "appbar appbar" "outline canvas" "findings findings";
}

.slidx-editor[data-access="read"] > :is(
  .slidx-inspector,
  .slidx-timeline,
  .slidx-storyboard,
  .slidx-revisions,
  .slidx-arrange,
  .slidx-resize,
  .slidx-freeform,
  .slidx-media-drop
),
.slidx-editor[data-access="read"] .slidx-slide-add,
.slidx-editor[data-access="read"] .slidx-outline-actions,
.slidx-editor[data-access="read"] .slidx-content {
  display: none;
}

.slidx-editor[data-access="read"] :is(textarea, input)[readonly] {
  color: var(--slidx-e-muted);
  background: var(--slidx-e-surface);
  cursor: text;
}

/*
 * Preserve the editing field before preserving generous side-panel widths.
 *
 * The panels remain present — hiding one would make a narrower window change
 * what the product can do — but their fixed desktop measures yield in two
 * deliberate steps. This also keeps all three canvas commands on their row,
 * instead of clipping the last one behind the inspector.
 */
@media (max-width: 75em) {
  .slidx-editor { grid-template-columns: 12.5rem minmax(0, 1fr) 16.5rem; }
}

@media (max-width: 56em) {
  .slidx-editor { grid-template-columns: 11rem minmax(0, 1fr) 14rem; }
  .slidx-outline .slidx-panel-head {
    gap: var(--slidx-e-snug);
    padding-inline: var(--slidx-e-snug);
  }
}

.slidx-appbar { grid-area: appbar; }
.slidx-outline { grid-area: outline; border-right: var(--slidx-e-hairline) solid var(--slidx-e-line); }
.slidx-canvas { grid-area: canvas; }
.slidx-inspector { grid-area: inspector; border-left: var(--slidx-e-hairline) solid var(--slidx-e-line); }
.slidx-diagnostics { grid-area: findings; border-top: var(--slidx-e-hairline) solid var(--slidx-e-line); }

/*
 * Side-panel separators.
 *
 * The eight-pixel target straddles a one-pixel rule. It overlays the existing
 * three-column grid rather than becoming two more columns, because the timeline
 * owns the editor's grid areas and both surfaces must keep one definition.
 */
.slidx-panel-resizers {
  grid-column: 1 / -1;
  grid-row: 1;
  position: relative;
  min-width: 0;
  min-height: 0;
  z-index: 8;
  pointer-events: none;
}

.slidx-panel-resizer {
  position: absolute;
  top: 0;
  bottom: 0;
  width: var(--slidx-e-snug);
  touch-action: none;
  cursor: col-resize;
  pointer-events: auto;
}

.slidx-panel-resizer[data-panel="outline"] {
  left: calc(var(--slidx-e-outline-width) - var(--slidx-e-tight));
}

.slidx-panel-resizer[data-panel="inspector"] {
  right: calc(var(--slidx-e-inspector-width) - var(--slidx-e-tight));
}

.slidx-panel-resizer::before {
  content: "";
  position: absolute;
  inset-block: 0;
  left: calc(50% - var(--slidx-e-hairline));
  width: var(--slidx-e-hairline);
  background: transparent;
}

.slidx-panel-resizer:hover::before,
.slidx-panel-resizer:focus-visible::before,
.slidx-panel-resizer[data-dragging="true"]::before {
  background: var(--slidx-e-accent);
}

/*
 * The shared focus ring sets position: relative on anything focusable, which
 * would drop a separator into the flow of its overlay and away from the panel
 * edge at the moment keyboard resizing begins.
 */
.slidx-panel-resizer[data-panel]:focus-visible {
  position: absolute;
}

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

.slidx-slide-add-toggle { white-space: nowrap; }
.slidx-content-toggle { white-space: nowrap; }
.slidx-canvas-scheme { white-space: nowrap; }
.slidx-canvas-toggle { white-space: nowrap; }

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
  position: relative;
  display: grid;
  gap: var(--slidx-e-tight);
  min-height: var(--slidx-e-hit);
  margin-bottom: var(--slidx-e-snug);
  padding: var(--slidx-e-tight);
  border-left: 2px solid transparent;
  border-radius: var(--slidx-e-radius);
}

.slidx-outline-thumbnail {
  display: block;
  min-width: 0;
  overflow: hidden;
  aspect-ratio: 16 / 9;
  background: var(--slidx-e-surface);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
}

.slidx-outline-thumbnail[data-empty="true"]::after {
  content: "";
  display: block;
  width: 100%;
  height: 100%;
  background: color-mix(in srgb, var(--slidx-e-line) 18%, transparent);
}

.slidx-outline-frame {
  display: block;
  width: 100%;
  height: 100%;
  border: 0;
  pointer-events: none;
}

.slidx-outline-caption {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
  min-width: 0;
  min-height: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-tight);
}

.slidx-outline-open {
  position: absolute;
  inset: 0;
  z-index: 1;
  width: 100%;
  height: 100%;
  padding: 0;
  border: 0;
  border-radius: var(--slidx-e-radius);
  cursor: grab;
}

.slidx-outline-open:hover,
.slidx-outline-open:active { background: transparent; }

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
}

.slidx-outline-row[aria-current="true"] .slidx-outline-thumbnail {
  border-color: var(--slidx-e-accent);
}
.slidx-outline-row[aria-current="true"] .slidx-outline-title { font-weight: 600; }

.slidx-outline-number {
  align-self: start;
  grid-column: 1;
  grid-row: 1;
  color: var(--slidx-e-muted);
  font-variant-numeric: tabular-nums;
  text-align: center;
}

.slidx-outline-preview {
  position: relative;
  z-index: 0;
  grid-column: 2;
  grid-row: 1;
  display: block;
  width: 100%;
  aspect-ratio: 16 / 9;
  overflow: hidden;
  background: var(--slidx-e-canvas);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
}

.slidx-outline-frame {
  display: block;
  width: 100%;
  height: 100%;
  border: 0;
  pointer-events: none;
}

.slidx-outline-caption {
  grid-column: 2;
  grid-row: 2;
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
  min-width: 0;
}

.slidx-outline-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-dot {
  flex: none;
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
.slidx-outline-actions {
  position: absolute;
  z-index: 2;
  top: var(--slidx-e-snug);
  right: var(--slidx-e-snug);
  display: flex;
  gap: var(--slidx-e-tight);
}

.slidx-outline-actions button {
  min-width: var(--slidx-e-hit);
  min-height: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-tight);
  background: color-mix(in srgb, var(--slidx-e-canvas) 88%, transparent);
  border-color: color-mix(in srgb, var(--slidx-e-line) 72%, transparent);
  color: var(--slidx-e-muted);
  background: var(--slidx-e-canvas);
  opacity: 0;
  pointer-events: none;
}

.slidx-outline-row:hover .slidx-outline-actions button,
.slidx-outline-duplicate:focus-visible,
.slidx-outline-remove:focus-visible {
  opacity: 1;
  pointer-events: auto;
}

.slidx-outline-remove:hover { color: var(--slidx-e-error); }

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

.slidx-inspector-tabs {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--slidx-e-tight);
  padding: var(--slidx-e-snug) var(--slidx-e-gap) 0;
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-inspector-tab {
  min-width: 0;
  padding: 0 var(--slidx-e-tight);
  overflow: hidden;
  border-color: transparent;
  border-bottom: 2px solid transparent;
  border-radius: var(--slidx-e-radius) var(--slidx-e-radius) 0 0;
  color: var(--slidx-e-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-inspector-tab:hover { background: var(--slidx-e-surface); }

.slidx-inspector-tab[aria-selected="true"] {
  color: var(--slidx-e-text);
  background: var(--slidx-e-surface);
  border-bottom-color: var(--slidx-e-accent);
  font-weight: 600;
}

.slidx-inspector-panels {
  flex: 1;
  min-height: 0;
  padding: var(--slidx-e-loose);
  overflow-y: auto;
}

.slidx-inspector-panels > .slidx-group.slidx-inspector-panel {
  margin-top: 0;
  padding-top: 0;
  border-top: 0;
}

.slidx-inspector-panel[hidden] { display: none; }
.slidx-inspector-panel > h3 { display: none; }

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
 * The scheme control sits beside the Markdown toggle and is the same shape as
 * it: a quiet button in the panel head, not a switch that draws the eye away
 * from the slide it is about.
 */
.slidx-canvas-scheme {
  padding: 2px 8px;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: transparent;
  color: var(--slidx-e-muted);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.slidx-canvas-scheme:hover,
.slidx-canvas-scheme:focus-visible { color: var(--slidx-e-text); }

/* Auto has no forced palette; light and dark are deliberate overrides. */
.slidx-canvas-scheme[data-scheme="light"],
.slidx-canvas-scheme[data-scheme="dark"] {
  border-color: var(--slidx-e-accent);
  color: var(--slidx-e-text);
}

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
