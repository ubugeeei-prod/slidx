/** Contextual inspector chrome, kept separate from the editor grid. */

export const INSPECTOR_STYLESHEET = `
.slidx-inspector-head {
  min-height: 68px;
}

.slidx-inspector-eyebrow {
  display: block;
  margin-bottom: 2px;
  color: var(--slidx-e-accent);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.14em;
  text-transform: uppercase;
}

.slidx-inspector-tabs {
  display: flex;
  min-height: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-gap);
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
  overflow-x: auto;
}

.slidx-inspector-tab {
  position: relative;
  min-width: 0;
  min-height: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-snug);
  border: 0;
  background: transparent;
  color: var(--slidx-e-muted);
  font-size: 11px;
  cursor: pointer;
}

.slidx-inspector-tab::after {
  content: "";
  position: absolute;
  inset: auto var(--slidx-e-snug) -1px;
  height: 2px;
  background: transparent;
}

.slidx-inspector-tab[aria-selected="true"] {
  color: var(--slidx-e-text);
}

.slidx-inspector-tab[aria-selected="true"]::after {
  background: var(--slidx-e-accent);
}

.slidx-inspector-panels > [hidden] { display: none; }

.slidx-text-context {
  display: grid;
  grid-template-columns: 36px minmax(0, 1fr);
  gap: var(--slidx-e-snug);
  align-items: center;
  margin-bottom: var(--slidx-e-gap);
  padding: var(--slidx-e-snug);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-surface);
}

.slidx-text-context-mark {
  display: grid;
  width: 36px;
  height: 36px;
  place-items: center;
  border: var(--slidx-e-hairline) solid var(--slidx-e-accent);
  border-radius: var(--slidx-e-radius);
  color: var(--slidx-e-accent);
  font-family: var(--slidx-e-font-sans);
  font-size: 13px;
  font-weight: 650;
}

.slidx-text-context-label {
  display: block;
  margin-bottom: 2px;
  color: var(--slidx-e-muted);
  font-size: 9px;
  font-weight: 650;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.slidx-text-context .slidx-selected {
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: transparent;
  color: var(--slidx-e-text);
  line-height: 1.4;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-text-tones {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--slidx-e-tight);
}

.slidx-text-tone,
.slidx-text-segment {
  min-width: 0;
  min-height: var(--slidx-e-hit);
  border-color: var(--slidx-e-line);
  background: var(--slidx-e-canvas);
  color: var(--slidx-e-muted);
  cursor: pointer;
}

.slidx-text-tone {
  display: grid;
  grid-template-columns: 8px minmax(0, 1fr);
  gap: var(--slidx-e-tight);
  align-items: center;
  padding-inline: 6px;
  font-size: 9px;
  text-align: left;
}

.slidx-text-tone-swatch {
  width: 8px;
  height: 8px;
  border: var(--slidx-e-hairline) solid currentColor;
  border-radius: 50%;
  background: var(--slidx-e-text);
}

.slidx-text-tone[data-tone="accent"] .slidx-text-tone-swatch {
  background: var(--slidx-e-accent);
}

.slidx-text-tone[data-tone="muted"] .slidx-text-tone-swatch {
  background: var(--slidx-e-muted);
}

.slidx-text-tone[data-tone="danger"] .slidx-text-tone-swatch {
  background: var(--slidx-e-error);
}

.slidx-text-tone[data-tone="success"] .slidx-text-tone-swatch {
  background: var(--slidx-e-success);
}

.slidx-text-segments {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--slidx-e-tight);
}

.slidx-text-segment {
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr);
  gap: var(--slidx-e-snug);
  align-items: center;
  padding: 0 var(--slidx-e-snug);
  text-align: left;
}

.slidx-text-segment > span {
  display: grid;
  height: 20px;
  place-items: center;
  border-right: var(--slidx-e-hairline) solid var(--slidx-e-line);
  color: var(--slidx-e-text);
  font-family: var(--slidx-e-font-sans);
  font-size: 10px;
}

.slidx-text-segment[data-value="bold"] > span { font-weight: 750; }
.slidx-text-segment[data-value="code"],
.slidx-text-segment[data-value="code"] > span { font-family: var(--slidx-e-font-mono); }

.slidx-text-tone[aria-pressed="true"],
.slidx-text-segment[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
  background: color-mix(in srgb, var(--slidx-e-accent) 8%, var(--slidx-e-canvas));
  color: var(--slidx-e-text);
}

.slidx-text-tone:disabled,
.slidx-text-segment:disabled {
  opacity: 0.42;
  cursor: default;
}

.slidx-text-advanced {
  margin-top: var(--slidx-e-gap);
  padding-top: var(--slidx-e-snug);
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-text-advanced summary {
  display: grid;
  min-height: var(--slidx-e-hit);
  grid-template-columns: minmax(0, 1fr) auto auto;
  gap: var(--slidx-e-snug);
  align-items: center;
  color: var(--slidx-e-text);
  cursor: pointer;
  list-style: none;
}

.slidx-text-advanced summary::-webkit-details-marker { display: none; }
.slidx-text-advanced summary::after { content: "+"; color: var(--slidx-e-accent); }
.slidx-text-advanced[open] summary::after { content: "−"; }

.slidx-text-advanced-hint {
  color: var(--slidx-e-muted);
  font-size: 9px;
}

.slidx-text-advanced-body { padding-top: var(--slidx-e-snug); }

.slidx-text-advanced-actions {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--slidx-e-tight);
}

.slidx-text-advanced-actions button {
  min-height: var(--slidx-e-hit);
  border-color: var(--slidx-e-line);
}

.slidx-text-advanced-actions .slidx-add {
  border-color: var(--slidx-e-accent);
  color: var(--slidx-e-accent);
}

.slidx-text-advanced-actions .slidx-remove-mark {
  margin: 0;
  color: var(--slidx-e-error);
}

.slidx-text-advanced-actions .slidx-remove-mark:disabled {
  color: var(--slidx-e-muted);
  opacity: 0.42;
}

.slidx-block-context {
  display: grid;
  grid-template-columns: var(--slidx-e-hit) minmax(0, 1fr);
  gap: var(--slidx-e-gap);
  align-items: center;
  margin-bottom: var(--slidx-e-gap);
  padding: var(--slidx-e-gap);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-surface);
}

.slidx-block-number {
  display: grid;
  width: var(--slidx-e-hit);
  height: var(--slidx-e-hit);
  place-items: center;
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-accent);
  color: var(--slidx-e-canvas);
  font-family: var(--slidx-e-font-mono);
  font-size: 11px;
  font-weight: 700;
}

.slidx-block-source {
  margin: 0;
  overflow: hidden;
  color: var(--slidx-e-text);
  font-family: var(--slidx-e-font-mono);
  font-size: 11px;
  line-height: 1.45;
}

.slidx-inspector-section {
  padding: var(--slidx-e-gap) 0;
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-inspector-section h4 {
  margin: 0 0 var(--slidx-e-snug);
  color: var(--slidx-e-muted);
  font-size: 10px;
  font-weight: 650;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.slidx-region-choices {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--slidx-e-snug);
}

.slidx-region-choice {
  display: flex;
  min-width: 0;
  min-height: var(--slidx-e-hit);
  align-items: center;
  gap: var(--slidx-e-snug);
  padding: var(--slidx-e-tight) var(--slidx-e-snug);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
  color: var(--slidx-e-muted);
  cursor: pointer;
}

.slidx-region-choice span {
  display: grid;
  width: 24px;
  height: 24px;
  flex: 0 0 auto;
  place-items: center;
  border: var(--slidx-e-hairline) solid currentColor;
  border-radius: 2px;
  font-family: var(--slidx-e-font-mono);
  font-size: 8px;
}

.slidx-region-choice[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
  color: var(--slidx-e-accent);
}

.slidx-frame-position {
  margin-top: var(--slidx-e-snug);
  padding-top: var(--slidx-e-snug);
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-frame-position-head {
  display: flex;
  min-height: var(--slidx-e-hit);
  align-items: center;
  justify-content: space-between;
  gap: var(--slidx-e-snug);
}

.slidx-frame-position-state {
  color: var(--slidx-e-muted);
  font-size: 11px;
}

.slidx-frame-position-state[data-pinned="true"] {
  color: var(--slidx-e-accent);
}

.slidx-frame-reset {
  min-height: var(--slidx-e-hit);
  padding-inline: var(--slidx-e-snug);
  border-color: var(--slidx-e-line);
  color: var(--slidx-e-muted);
  font-size: 11px;
}

.slidx-frame-reset:disabled {
  opacity: 0.42;
  cursor: default;
}

.slidx-frame-anchors {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--slidx-e-tight);
  margin-top: var(--slidx-e-snug);
  padding: var(--slidx-e-snug);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-surface);
}

.slidx-frame-anchor {
  display: grid;
  min-width: 0;
  min-height: var(--slidx-e-hit);
  place-items: center;
  padding: 0;
  border-color: transparent;
  background: var(--slidx-e-canvas);
}

.slidx-frame-anchor span {
  width: 6px;
  height: 6px;
  border: var(--slidx-e-hairline) solid var(--slidx-e-muted);
  border-radius: 2px;
}

.slidx-frame-anchor[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
  background: color-mix(in srgb, var(--slidx-e-accent) 9%, var(--slidx-e-canvas));
}

.slidx-frame-anchor[aria-pressed="true"] span {
  border-color: var(--slidx-e-accent);
  background: var(--slidx-e-accent);
}

.slidx-frame-anchor:disabled {
  opacity: 0.42;
  cursor: default;
}

.slidx-frame-position-hint {
  margin: var(--slidx-e-snug) 0 0;
  color: var(--slidx-e-muted);
  font-size: 11px;
}

.slidx-width-choices {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--slidx-e-snug);
}

.slidx-width-choice {
  min-width: 0;
  min-height: var(--slidx-e-hit);
  padding: var(--slidx-e-snug);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
  cursor: pointer;
}

.slidx-width-track {
  display: flex;
  width: 100%;
  height: 12px;
  align-items: center;
  justify-content: center;
  border-inline: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-width-bar {
  height: 4px;
  border-radius: 2px;
  background: var(--slidx-e-muted);
}

.slidx-width-choice[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
}

.slidx-width-choice[aria-pressed="true"] .slidx-width-bar {
  background: var(--slidx-e-accent);
}

.slidx-block-palette {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--slidx-e-snug);
}

.slidx-block-color-choice {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: var(--slidx-e-snug);
  padding: var(--slidx-e-tight) var(--slidx-e-snug);
  border-color: var(--slidx-e-line);
  color: var(--slidx-e-muted);
  text-align: left;
}

.slidx-block-color-choice[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
  color: var(--slidx-e-text);
}

.slidx-block-color-choice:disabled { opacity: 0.42; cursor: default; }

.slidx-block-color-swatch {
  display: flex;
  width: 24px;
  height: 24px;
  flex: 0 0 auto;
  overflow: hidden;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: 2px;
  background: var(--slidx-e-canvas);
}

.slidx-block-color-swatch > span { flex: 1; }

.slidx-block-color-hint {
  margin: var(--slidx-e-snug) 0 0;
  color: var(--slidx-e-muted);
  font-size: 11px;
}

.slidx-block-color-custom {
  margin-top: var(--slidx-e-snug);
  padding-top: var(--slidx-e-snug);
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-block-color-custom summary {
  display: flex;
  min-height: var(--slidx-e-hit);
  align-items: center;
  justify-content: space-between;
  color: var(--slidx-e-muted);
  cursor: pointer;
  list-style: none;
}

.slidx-block-color-custom summary::-webkit-details-marker { display: none; }
.slidx-block-color-custom summary::after { content: "+"; color: var(--slidx-e-accent); }
.slidx-block-color-custom[open] summary::after { content: "−"; }

.slidx-block-color {
  display: grid;
  grid-template-columns: var(--slidx-e-hit) minmax(0, 1fr);
  gap: var(--slidx-e-snug);
  align-items: center;
  padding-top: var(--slidx-e-snug);
}

.slidx-block-color-input {
  width: var(--slidx-e-hit);
  height: var(--slidx-e-hit);
  padding: 2px;
  cursor: pointer;
}

.slidx-block-color-input::-webkit-color-swatch-wrapper { padding: 0; }
.slidx-block-color-input::-webkit-color-swatch { border: 0; border-radius: 2px; }
.slidx-block-color-input::-moz-color-swatch { border: 0; border-radius: 2px; }

.slidx-block-color-value {
  color: var(--slidx-e-muted);
  font-family: var(--slidx-e-font-mono);
  font-size: 11px;
}

.slidx-block-attributes,
.slidx-block-actions button {
  min-height: var(--slidx-e-hit);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
  color: var(--slidx-e-text);
  cursor: pointer;
}

.slidx-block-identity .slidx-field {
  grid-template-columns: 68px minmax(0, 1fr);
}

.slidx-block-attributes {
  width: 100%;
  border-color: var(--slidx-e-accent);
  color: var(--slidx-e-accent);
}

.slidx-block-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--slidx-e-snug);
  padding-top: var(--slidx-e-gap);
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-block-actions .slidx-block-delete {
  color: var(--slidx-e-error);
}

.slidx-region-choice:hover,
.slidx-width-choice:hover,
.slidx-frame-anchor:not(:disabled):hover,
.slidx-block-color-choice:not(:disabled):hover,
.slidx-text-tone:not(:disabled):hover,
.slidx-text-segment:not(:disabled):hover,
.slidx-block-actions button:hover {
  border-color: var(--slidx-e-accent);
}
`;

export function applyInspectorStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-inspector]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-inspector", "");
  style.textContent = INSPECTOR_STYLESHEET;
  document.head.append(style);
}
