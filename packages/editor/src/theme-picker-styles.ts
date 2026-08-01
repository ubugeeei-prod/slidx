/** Theme gallery chrome, separate so integrations can omit the picker cleanly. */

export const THEME_PICKER_STYLESHEET = `
.slidx-theme-field {
  display: grid;
  gap: var(--slidx-e-snug);
  margin-bottom: var(--slidx-e-gap);
}

.slidx-theme-picker {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(100px, 1fr));
  gap: var(--slidx-e-snug);
}

.slidx-theme-choice {
  display: grid;
  min-width: 0;
  height: auto;
  align-content: start;
  gap: var(--slidx-e-snug);
  padding: var(--slidx-e-tight);
  border-color: var(--slidx-e-line);
  color: var(--slidx-e-muted);
  text-align: left;
}

.slidx-theme-choice[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
  background: var(--slidx-e-surface);
  color: var(--slidx-e-text);
}

.slidx-theme-choice:disabled { cursor: default; opacity: 0.58; }

.slidx-theme-preview {
  position: relative;
  display: grid;
  width: 100%;
  aspect-ratio: 16 / 9;
  grid-template-columns: 1fr auto;
  grid-template-rows: 1fr auto auto;
  gap: 4px 6px;
  overflow: hidden;
  padding: 8px;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-theme-preview-accent {
  position: absolute;
  inset: 0 auto 0 0;
  width: 3px;
}

.slidx-theme-preview-heading {
  align-self: start;
  font-size: clamp(14px, 2vw, 20px);
  font-weight: 700;
  line-height: 1;
}

.slidx-theme-preview-line {
  width: 78%;
  height: 3px;
  align-self: center;
  opacity: 0.72;
}

.slidx-theme-preview-line[data-muted="true"] {
  width: 56%;
  opacity: 1;
}

.slidx-theme-preview-code {
  grid-column: 2;
  grid-row: 1 / span 3;
  align-self: end;
  padding: 3px 4px;
  font-size: 8px;
  line-height: 1;
}

.slidx-theme-copy {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.slidx-theme-copy strong {
  overflow: hidden;
  color: currentColor;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-theme-copy > span {
  display: -webkit-box;
  overflow: hidden;
  color: var(--slidx-e-muted);
  font-size: 9px;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.slidx-theme-notice {
  margin: 0;
  padding: var(--slidx-e-snug);
  border-left: 2px solid var(--slidx-e-accent);
  color: var(--slidx-e-muted);
  font-size: 10px;
  line-height: 1.45;
}

.slidx-theme-choice:not(:disabled):hover { border-color: var(--slidx-e-accent); }
`;

export function applyThemePickerStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-theme-picker]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-theme-picker", "");
  style.textContent = THEME_PICKER_STYLESHEET;
  document.head.append(style);
}
