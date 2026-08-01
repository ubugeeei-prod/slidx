/** The searchable route to deck actions and named slides. */

export const COMMAND_PALETTE_STYLESHEET = `
.slidx-command-trigger {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
  min-width: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-snug);
  border-color: transparent;
  color: var(--slidx-e-muted);
  font-size: 11px;
  white-space: nowrap;
}

.slidx-command-trigger:hover,
.slidx-command-trigger[aria-expanded="true"] {
  background: var(--slidx-e-surface);
  color: var(--slidx-e-text);
}

.slidx-command-trigger-icon {
  color: var(--slidx-e-accent);
  font-size: var(--slidx-e-lockup);
  line-height: 1;
}

.slidx-command-trigger kbd,
.slidx-command-footer kbd {
  padding: 0 var(--slidx-e-tight);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
  color: var(--slidx-e-muted);
  font: 11px/1.5 var(--slidx-e-font-mono);
}

.slidx-command-palette {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: grid;
  place-items: start center;
  padding-top: 12vh;
  background: color-mix(in srgb, var(--slidx-e-text) 20%, transparent);
}

.slidx-command-palette[hidden] { display: none; }

.slidx-command-dialog {
  display: flex;
  flex-direction: column;
  width: min(42rem, calc(100vw - var(--slidx-e-loose) - var(--slidx-e-loose)));
  max-height: min(42rem, 76vh);
  overflow: hidden;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
  color: var(--slidx-e-text);
}

.slidx-command-search {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: center;
  gap: var(--slidx-e-snug);
  padding-left: var(--slidx-e-gap);
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-command-search-mark {
  color: var(--slidx-e-accent);
  font-size: var(--slidx-e-lockup);
}

.slidx-command-input {
  min-height: 3.25rem;
  padding: 0 var(--slidx-e-gap) 0 0;
  border: 0;
  border-radius: 0;
  background: transparent;
  font-size: 1rem;
}

.slidx-command-input:focus-visible {
  outline-offset: calc(var(--slidx-e-hairline) * -1);
}

.slidx-command-results {
  min-height: var(--slidx-e-hit);
  overflow-y: auto;
}

.slidx-command-item {
  display: grid;
  grid-template-columns: 2.5rem minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--slidx-e-snug);
  width: 100%;
  min-height: 3rem;
  padding: var(--slidx-e-snug) var(--slidx-e-gap);
  border: 0;
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: 0;
  text-align: left;
}

.slidx-command-item[aria-selected="true"] {
  border-left: 2px solid var(--slidx-e-accent);
  background: var(--slidx-e-surface);
}

.slidx-command-item[aria-disabled="true"] {
  cursor: default;
  opacity: 0.42;
}

.slidx-command-item[aria-current="true"] {
  cursor: default;
  opacity: 1;
}

.slidx-command-kind {
  color: var(--slidx-e-accent);
  font: 12px/1 var(--slidx-e-font-mono);
  text-align: center;
}

.slidx-command-item[data-command-tone="theme"] .slidx-command-kind {
  color: var(--slidx-e-text);
}

.slidx-command-item[data-command-tone="muted"] .slidx-command-kind {
  color: var(--slidx-e-muted);
}

.slidx-command-item[data-command-tone="danger"] .slidx-command-kind {
  color: var(--slidx-e-error);
}

.slidx-command-item[data-command-tone="success"] .slidx-command-kind {
  color: var(--slidx-e-success);
}

.slidx-command-copy { min-width: 0; }

.slidx-command-title {
  display: block;
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-command-hint {
  display: block;
  overflow: hidden;
  color: var(--slidx-e-muted);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-command-type {
  color: var(--slidx-e-muted);
  font-size: 11px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.slidx-command-empty {
  margin: 0;
  padding: var(--slidx-e-loose);
  color: var(--slidx-e-muted);
  text-align: center;
}

.slidx-command-empty[hidden] { display: none; }

.slidx-command-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--slidx-e-gap);
  padding: var(--slidx-e-snug) var(--slidx-e-gap);
  color: var(--slidx-e-muted);
  font-size: 11px;
}

.slidx-command-footer span {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-tight);
}

@media (max-width: 75em) {
  .slidx-command-trigger-label { display: none; }
}

@media (max-width: 47.5em) {
  .slidx-command-trigger kbd { display: none; }
  .slidx-command-footer { justify-content: space-between; }
}
`;

export function applyCommandPaletteStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-command-palette]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-command-palette", "");
  style.textContent = COMMAND_PALETTE_STYLESHEET;
  document.head.append(style);
}
