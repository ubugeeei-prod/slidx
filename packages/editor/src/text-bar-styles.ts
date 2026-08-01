/** Selection-local text tools that live in the canvas header. */

export const TEXT_BAR_STYLESHEET = `
.slidx-text-bar[hidden] { display: none; }

.slidx-canvas .slidx-panel-head[data-text-tools="true"] .slidx-canvas-tools {
  display: none;
}

.slidx-text-bar,
.slidx-text-bar-tones,
.slidx-text-bar-actions {
  display: flex;
  align-items: center;
}

.slidx-text-bar { gap: var(--slidx-e-snug); }
.slidx-text-bar-tones,
.slidx-text-bar-actions { gap: var(--slidx-e-tight); }

.slidx-text-bar-tones {
  padding-right: var(--slidx-e-snug);
  border-right: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-text-bar-tone,
.slidx-text-bar-toggle {
  display: grid;
  width: var(--slidx-e-hit);
  min-width: var(--slidx-e-hit);
  height: var(--slidx-e-hit);
  min-height: var(--slidx-e-hit);
  padding: 0;
  place-items: center;
  border-color: transparent;
  color: var(--slidx-e-muted);
}

.slidx-text-bar-swatch {
  width: 10px;
  height: 10px;
  border: var(--slidx-e-hairline) solid currentColor;
  border-radius: 50%;
  background: var(--slidx-e-text);
}

.slidx-text-bar-tone[data-tone="accent"] .slidx-text-bar-swatch {
  background: var(--slidx-e-accent);
}

.slidx-text-bar-tone[data-tone="muted"] .slidx-text-bar-swatch {
  background: var(--slidx-e-muted);
}

.slidx-text-bar-tone[data-tone="danger"] .slidx-text-bar-swatch {
  background: var(--slidx-e-error);
}

.slidx-text-bar-tone[data-tone="success"] .slidx-text-bar-swatch {
  background: var(--slidx-e-success);
}

.slidx-text-bar-toggle[data-style="bold"] { font-weight: 750; }
.slidx-text-bar-toggle[data-style="code"] { font-family: var(--slidx-e-font-mono); }

.slidx-text-bar-tone[aria-pressed="true"],
.slidx-text-bar-toggle[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
  background: color-mix(in srgb, var(--slidx-e-accent) 8%, var(--slidx-e-canvas));
  color: var(--slidx-e-text);
}

.slidx-text-bar-tone:not(:disabled):hover,
.slidx-text-bar-toggle:not(:disabled):hover {
  border-color: var(--slidx-e-accent);
}

.slidx-text-bar-tone:disabled,
.slidx-text-bar-toggle:disabled {
  opacity: 0.42;
  cursor: default;
}

.slidx-text-bar-done {
  min-height: var(--slidx-e-hit);
  border-color: transparent;
  color: var(--slidx-e-accent);
  font-size: 11px;
  font-weight: 650;
}

@media (max-width: 48em) {
  .slidx-text-bar { gap: var(--slidx-e-tight); }
  .slidx-text-bar-tones { padding-right: var(--slidx-e-tight); }
}
`;

export function applyTextBarStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-text-bar]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-text-bar", "");
  style.textContent = TEXT_BAR_STYLESHEET;
  document.head.append(style);
}
