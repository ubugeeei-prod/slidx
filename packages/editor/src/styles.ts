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
 */

export const STYLESHEET = `
:root {
  --slidx-e-canvas: #ffffff;
  --slidx-e-surface: #f7f7f8;
  --slidx-e-text: #16181d;
  --slidx-e-muted: #6a6f7a;
  --slidx-e-line: #e2e3e7;
  --slidx-e-accent: #2f6feb;
  --slidx-e-error: #b42318;
  --slidx-e-warning: #9a6700;
  --slidx-e-hairline: 1px;
  --slidx-e-radius: 4px;
  --slidx-e-gap: 12px;

  color-scheme: light dark;
}

@media (prefers-color-scheme: dark) {
  :root {
    --slidx-e-canvas: #16181d;
    --slidx-e-surface: #1c1f26;
    --slidx-e-text: #e8eaed;
    --slidx-e-muted: #9aa0ac;
    --slidx-e-line: #2a2e37;
    --slidx-e-accent: #7aa2f7;
    --slidx-e-error: #f5776a;
    --slidx-e-warning: #d9a441;
  }
}

*, *::before, *::after { box-sizing: border-box; }

body {
  margin: 0;
  height: 100vh;
  background: var(--slidx-e-canvas);
  color: var(--slidx-e-text);
  font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  font-size: 13px;
  line-height: 1.5;
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
  padding: 2px 8px;
  cursor: pointer;
}

button:hover { background: var(--slidx-e-surface); }
button:focus-visible, [contenteditable]:focus-visible, input:focus-visible, textarea:focus-visible {
  outline: 2px solid var(--slidx-e-accent);
  outline-offset: 1px;
}

/* Outline. */

.slidx-outline-list {
  margin: 0;
  padding: 4px;
  list-style: none;
  overflow-y: auto;
}

.slidx-outline-row {
  display: flex;
  align-items: center;
  padding-right: 4px;
  border-radius: var(--slidx-e-radius);
}

.slidx-outline-open {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border: 0;
  border-radius: var(--slidx-e-radius);
  text-align: left;
}

.slidx-outline-open:hover { background: transparent; }

.slidx-outline-row:hover { background: var(--slidx-e-surface); }
.slidx-outline-row[aria-current="true"] { background: var(--slidx-e-surface); }
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

.slidx-outline-remove {
  border: 0;
  padding: 0 4px;
  color: var(--slidx-e-muted);
  opacity: 0;
}

.slidx-outline-row:hover .slidx-outline-remove { opacity: 1; }

/* Canvas. */

.slidx-canvas-stage {
  flex: 1;
  min-height: 0;
  display: grid;
  padding: var(--slidx-e-gap);
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
  padding: 12px;
  color: inherit;
  background: var(--slidx-e-surface);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace;
  font-size: 12px;
  tab-size: 2;
}

/* Inspector. */

.slidx-inspector-panels { padding: var(--slidx-e-gap); overflow-y: auto; }

.slidx-group + .slidx-group {
  margin-top: 18px;
  padding-top: 14px;
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-group h3 {
  margin: 0 0 8px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--slidx-e-muted);
}

.slidx-field { display: grid; grid-template-columns: 78px minmax(0, 1fr); align-items: center; gap: 8px; margin-bottom: 6px; }
.slidx-field-name { color: var(--slidx-e-muted); }

input, textarea {
  width: 100%;
  font: inherit;
  color: inherit;
  background: var(--slidx-e-canvas);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  padding: 3px 6px;
}

textarea { resize: vertical; font-size: 12px; }

.slidx-hint { margin: 0; color: var(--slidx-e-muted); }
.slidx-selected {
  margin: 0 0 8px;
  padding: 6px 8px;
  background: var(--slidx-e-surface);
  border-radius: var(--slidx-e-radius);
  font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace;
  font-size: 12px;
}

/* Diagnostics. */

.slidx-diagnostics { max-height: 26vh; overflow-y: auto; }
.slidx-diagnostics[data-empty="true"] { display: none; }

.slidx-findings { margin: 0; padding: 6px var(--slidx-e-gap); list-style: none; }

.slidx-finding {
  display: flex;
  gap: 10px;
  align-items: baseline;
  padding: 2px 0;
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
  .slidx-outline-row, button, .slidx-outline-remove { transition: background 90ms linear, opacity 90ms linear; }
}
`;

/** Puts the chrome's stylesheet into a document, once. */
export function applyStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-editor]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-editor", "");
  style.textContent = STYLESHEET;
  document.head.append(style);
}
