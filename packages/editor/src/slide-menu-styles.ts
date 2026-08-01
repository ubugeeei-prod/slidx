/** The new-slide picker: small compositions, not decorative thumbnails. */
export const SLIDE_MENU_STYLESHEET = `
.slidx-slide-add { position: relative; }

.slidx-slide-add-toggle::before {
  content: "+";
  margin-right: var(--slidx-e-tight);
  color: var(--slidx-e-accent);
  font-weight: 650;
}

.slidx-slide-add-toggle[aria-expanded="true"] {
  border-color: var(--slidx-e-accent);
}

.slidx-slide-menu {
  position: absolute;
  top: calc(100% + var(--slidx-e-tight));
  right: 0;
  z-index: 12;
  width: 216px;
  padding: var(--slidx-e-tight);
  background: var(--slidx-e-canvas);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
}

.slidx-slide-menu[hidden] { display: none; }

.slidx-slide-choice {
  width: 100%;
  min-height: 58px;
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
  padding: var(--slidx-e-snug);
  border: 0;
  border-radius: 0;
  text-align: left;
}

.slidx-slide-choice + .slidx-slide-choice {
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-slide-choice-preview {
  flex: none;
  width: 56px;
  aspect-ratio: 16 / 9;
  display: grid;
  gap: 3px;
  padding: 4px;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  background: var(--slidx-e-surface);
}

.slidx-slide-choice-preview i {
  display: block;
  min-width: 0;
  min-height: 2px;
  background: color-mix(in srgb, var(--slidx-e-accent) 48%, var(--slidx-e-line));
}

[data-slide-kind="title-body"] .slidx-slide-choice-preview,
[data-slide-kind="points"] .slidx-slide-choice-preview {
  grid-template-rows: auto 1fr;
}

[data-slide-kind="title-body"] [data-part="title"],
[data-slide-kind="points"] [data-part="title"] {
  width: 68%;
  height: 3px;
}

[data-slide-kind="title-body"] [data-part="body"] {
  align-self: center;
  width: 88%;
  height: 4px;
}

[data-slide-kind="statement"] .slidx-slide-choice-preview {
  place-items: center;
}

[data-slide-kind="statement"] [data-part="statement"] {
  width: 78%;
  height: 5px;
}

[data-slide-kind="comparison"] .slidx-slide-choice-preview {
  grid-template-columns: 1fr 1fr;
}

[data-slide-kind="comparison"] .slidx-slide-choice-preview i {
  align-self: stretch;
}

[data-slide-kind="points"] .slidx-slide-choice-preview {
  grid-template-columns: 1fr;
  grid-template-rows: repeat(4, 1fr);
}

[data-slide-kind="points"] [data-part="point"] {
  width: 88%;
  height: 2px;
}

.slidx-slide-choice-copy {
  min-width: 0;
  display: grid;
  line-height: 1.35;
}

.slidx-slide-choice-copy strong { font-weight: 600; }
.slidx-slide-choice-copy span { color: var(--slidx-e-muted); font-size: 11px; }
`;

export function applySlideMenuStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-slide-menu]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-slide-menu", "");
  style.textContent = SLIDE_MENU_STYLESHEET;
  document.head.append(style);
}
