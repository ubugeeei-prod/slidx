/** The local author's handoff sheet in the deck command bar. */

export const SHARE_CONTROL_STYLESHEET = `
.slidx-share {
  position: relative;
  flex: none;
}

.slidx-share[hidden],
.slidx-share-popover[hidden] { display: none; }

.slidx-share-toggle {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
  min-width: var(--slidx-e-hit);
  min-height: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-snug);
  border-color: transparent;
  color: var(--slidx-e-muted);
  font-size: 11px;
  font-weight: 600;
}

.slidx-share-toggle:hover,
.slidx-share[data-open="true"] .slidx-share-toggle {
  background: var(--slidx-e-surface);
  color: var(--slidx-e-text);
}

.slidx-share-toggle-mark {
  color: var(--slidx-e-accent);
  font-size: 14px;
  line-height: 1;
}

.slidx-share-popover {
  position: absolute;
  z-index: 30;
  top: calc(100% + var(--slidx-e-tight));
  right: 0;
  display: grid;
  width: min(25rem, calc(100vw - var(--slidx-e-gap) - var(--slidx-e-gap)));
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
}

.slidx-share-head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) var(--slidx-e-hit);
  gap: var(--slidx-e-snug);
  padding: var(--slidx-e-gap);
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-share-heading { min-width: 0; }

.slidx-share-eyebrow {
  display: block;
  margin-bottom: var(--slidx-e-tight);
  color: var(--slidx-e-accent);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.slidx-share-title {
  margin: 0;
  color: var(--slidx-e-text);
  font-size: 14px;
  line-height: 1.25;
}

.slidx-share-close {
  width: var(--slidx-e-hit);
  height: var(--slidx-e-hit);
  padding: 0;
  border-color: transparent;
  color: var(--slidx-e-muted);
  font-size: 16px;
  line-height: 1;
}

.slidx-share-close:hover { background: var(--slidx-e-surface); color: var(--slidx-e-text); }

.slidx-share-body {
  display: grid;
  gap: var(--slidx-e-snug);
  padding: var(--slidx-e-gap);
}

.slidx-share-intro,
.slidx-share-foot,
.slidx-share-status {
  margin: 0;
  color: var(--slidx-e-muted);
  font-size: 11px;
  line-height: 1.45;
}

.slidx-share-links {
  display: grid;
  gap: var(--slidx-e-tight);
}

.slidx-share-link {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--slidx-e-gap);
  min-height: calc(var(--slidx-e-hit) + var(--slidx-e-gap));
  padding: var(--slidx-e-snug) var(--slidx-e-gap);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-surface);
}

.slidx-share-link[data-kind="edit"] { border-left-color: var(--slidx-e-warning); }

.slidx-share-link-copy { min-width: 0; }
.slidx-share-link-label { display: block; color: var(--slidx-e-text); font-weight: 650; }
.slidx-share-link-detail {
  display: block;
  overflow: hidden;
  color: var(--slidx-e-muted);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-share-copy {
  min-height: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-snug);
  border-color: var(--slidx-e-accent);
  background: var(--slidx-e-canvas);
  color: var(--slidx-e-accent);
  font-size: 11px;
  font-weight: 650;
  white-space: nowrap;
}

.slidx-share-copy:hover,
.slidx-share-copy[data-copied="true"] {
  background: var(--slidx-e-accent);
  color: var(--slidx-e-canvas);
}

.slidx-share-command {
  display: block;
  padding: var(--slidx-e-snug) var(--slidx-e-gap);
  border-left: 2px solid var(--slidx-e-accent);
  background: var(--slidx-e-surface);
  color: var(--slidx-e-text);
  font: 11px/1.45 var(--slidx-e-mono);
  user-select: all;
}

.slidx-share-status:not(:empty) {
  padding-top: var(--slidx-e-snug);
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
  color: var(--slidx-e-accent);
}

@media (max-width: 47.5em) {
  .slidx-share-toggle-label { display: none; }
}
`;

export function applyShareControlStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-share-control]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-share-control", "");
  style.textContent = SHARE_CONTROL_STYLESHEET;
  document.head.append(style);
}
