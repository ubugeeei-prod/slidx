/** The add-content menu, kept beside the small surface it styles. */
export const CONTENT_MENU_STYLESHEET = `
.slidx-canvas-tools {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
}

.slidx-content { position: relative; }

.slidx-content-toggle::before {
  content: "+";
  margin-right: var(--slidx-e-tight);
  color: var(--slidx-e-accent);
  font-weight: 650;
}

.slidx-content-toggle[aria-expanded="true"] {
  border-color: var(--slidx-e-accent);
  color: var(--slidx-e-text);
}

.slidx-content-toggle:disabled { cursor: default; opacity: 0.45; }

.slidx-content-menu {
  position: absolute;
  top: calc(100% + var(--slidx-e-tight));
  right: 0;
  z-index: 5;
  width: 232px;
  padding: var(--slidx-e-tight);
  background: var(--slidx-e-canvas);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
}

.slidx-content-menu[hidden] { display: none; }

.slidx-content-item {
  width: 100%;
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--slidx-e-gap);
  padding: var(--slidx-e-snug);
  border: 0;
  border-radius: 0;
  text-align: left;
}

.slidx-content-item + .slidx-content-item {
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-content-label { font-weight: 600; }
.slidx-content-hint { color: var(--slidx-e-muted); font-size: 12px; }
`;

export function applyContentMenuStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-content-menu]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-content-menu", "");
  style.textContent = CONTENT_MENU_STYLESHEET;
  document.head.append(style);
}
