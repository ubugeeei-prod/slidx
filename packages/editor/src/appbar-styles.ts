/** Product lockup and the commands that apply to the whole deck. */

export const APPBAR_STYLESHEET = `
.slidx-appbar {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-gap);
  min-width: 0;
  padding: 0 80px 0 var(--slidx-e-gap);
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-appbar-lockup {
  flex: none;
  display: flex;
  align-items: center;
  gap: calc(var(--slidx-e-lockup) / 4);
  color: var(--slidx-e-text);
}

.slidx-appbar-mark {
  width: var(--slidx-e-lockup);
  height: var(--slidx-e-lockup);
}

.slidx-appbar-mark-document { fill: var(--slidx-e-text); }
.slidx-appbar-mark-page { fill: var(--slidx-e-accent); }

.slidx-appbar-wordmark {
  font-size: var(--slidx-e-lockup);
  font-weight: 650;
  letter-spacing: -0.02em;
  line-height: 1;
}

.slidx-appbar-context {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--slidx-e-snug);
}

.slidx-appbar-title {
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-appbar-position {
  flex: none;
  padding-left: var(--slidx-e-snug);
  border-left: var(--slidx-e-hairline) solid var(--slidx-e-line);
  color: var(--slidx-e-muted);
  font-variant-numeric: tabular-nums;
}

.slidx-appbar-commands {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--slidx-e-tight);
}

.slidx-appbar-status {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
  margin-right: var(--slidx-e-tight);
  color: var(--slidx-e-muted);
  font-size: 11px;
  white-space: nowrap;
}

.slidx-appbar-status::before {
  content: "";
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
}

.slidx-appbar-status[data-state="opening"],
.slidx-appbar-status[data-state="writing"] { color: var(--slidx-e-accent); }
.slidx-appbar-status[data-state="warning"] { color: var(--slidx-e-warning); }
.slidx-appbar-status[data-state="error"] { color: var(--slidx-e-error); }
.slidx-appbar-status[data-state="readonly"] { color: var(--slidx-e-accent); }

.slidx-appbar-command-rule {
  width: var(--slidx-e-hairline);
  height: 16px;
  margin: 0 var(--slidx-e-tight);
  background: var(--slidx-e-line);
}

.slidx-appbar-command,
.slidx-appbar-present,
.slidx-appbar-delivery-toggle {
  min-width: var(--slidx-e-hit);
  min-height: var(--slidx-e-hit);
  border-color: transparent;
}

.slidx-appbar-command {
  padding: 0 var(--slidx-e-tight);
  font-size: 16px;
  line-height: 1;
}

.slidx-appbar-present {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
  padding: 0 var(--slidx-e-snug);
  border-color: var(--slidx-e-accent);
  border-radius: var(--slidx-e-radius) 0 0 var(--slidx-e-radius);
  background: var(--slidx-e-accent);
  color: var(--slidx-e-canvas);
  font-size: 11px;
  font-weight: 650;
}

.slidx-appbar-delivery {
  position: relative;
  display: flex;
  margin-left: var(--slidx-e-tight);
}

.slidx-appbar-delivery-toggle {
  width: var(--slidx-e-hit);
  padding: 0;
  border-color: var(--slidx-e-accent);
  border-left-color: color-mix(in srgb, var(--slidx-e-canvas) 30%, transparent);
  border-radius: 0 var(--slidx-e-radius) var(--slidx-e-radius) 0;
  background: var(--slidx-e-accent);
  color: var(--slidx-e-canvas);
  font-size: 11px;
}

.slidx-appbar-present:hover,
.slidx-appbar-delivery-toggle:hover {
  background: color-mix(in srgb, var(--slidx-e-accent) 88%, var(--slidx-e-canvas));
}

.slidx-appbar-command:disabled,
.slidx-appbar-present:disabled,
.slidx-appbar-delivery-toggle:disabled {
  opacity: 0.38;
  cursor: default;
}

.slidx-appbar-delivery-menu[hidden] { display: none; }

.slidx-appbar-delivery-menu {
  position: absolute;
  z-index: 20;
  top: calc(100% + var(--slidx-e-tight));
  right: 0;
  display: grid;
  min-width: 296px;
  padding: var(--slidx-e-tight);
  background: var(--slidx-e-canvas);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
}

.slidx-appbar-delivery-option {
  display: grid;
  grid-template-columns: var(--slidx-e-hit) minmax(0, 1fr);
  align-items: center;
  min-height: calc(var(--slidx-e-hit) + var(--slidx-e-gap));
  padding: var(--slidx-e-snug);
  text-align: left;
  border-color: transparent;
}

.slidx-appbar-delivery-option:hover,
.slidx-appbar-delivery-option:focus-visible { background: var(--slidx-e-surface); }

.slidx-appbar-delivery-mark {
  color: var(--slidx-e-accent);
  font-size: 16px;
}

.slidx-appbar-delivery-copy {
  display: grid;
  min-width: 0;
}

.slidx-appbar-delivery-label { font-weight: 650; }
.slidx-appbar-delivery-hint { color: var(--slidx-e-muted); font-size: 11px; }

@media (max-width: 60em) {
  .slidx-appbar-status[data-state="saved"],
  .slidx-appbar-command-rule { display: none; }
}

@media (max-width: 47.5em) {
  .slidx-appbar-present-label { display: none; }
  .slidx-appbar-present { min-width: var(--slidx-e-hit); padding: 0 var(--slidx-e-tight); }
}
`;

export function applyAppbarStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-appbar]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-appbar", "");
  style.textContent = APPBAR_STYLESHEET;
  document.head.append(style);
}
