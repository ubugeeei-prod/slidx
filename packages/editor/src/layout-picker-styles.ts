/**
 * The visual layout picker's stylesheet.
 *
 * The picker owns more space than an ordinary scalar field: a miniature needs
 * enough width for its named grid areas to remain recognisable. Keeping these
 * rules beside the surface also lets integrations omit the picker without
 * carrying its CSS.
 */

export const LAYOUT_PICKER_STYLESHEET = `
.slidx-layout-field {
  grid-template-columns: minmax(0, 1fr);
  align-items: stretch;
}

.slidx-layout-picker {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--slidx-e-tight);
}

.slidx-layout-choice {
  display: grid;
  gap: var(--slidx-e-tight);
  min-width: 0;
  height: auto;
  padding: var(--slidx-e-tight);
  text-align: left;
}

.slidx-layout-choice[aria-pressed="true"] {
  background: var(--slidx-e-surface);
  border-color: var(--slidx-e-accent);
}

.slidx-layout-preview {
  display: grid;
  width: 100%;
  aspect-ratio: 16 / 9;
  gap: 2px;
  padding: 2px;
  background: var(--slidx-e-canvas);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-layout-region {
  min-width: 0;
  min-height: 0;
  background: var(--slidx-e-line);
}

.slidx-layout-choice[aria-pressed="true"] .slidx-layout-region {
  background: var(--slidx-e-accent);
}

.slidx-layout-name {
  overflow: hidden;
  color: var(--slidx-e-muted);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-layout-choice[aria-pressed="true"] .slidx-layout-name {
  color: var(--slidx-e-text);
}
`;

/** Puts the layout picker's stylesheet into a document, once. */
export function applyLayoutPickerStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-layout-picker]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-layout-picker", "");
  style.textContent = LAYOUT_PICKER_STYLESHEET;
  document.head.append(style);
}
