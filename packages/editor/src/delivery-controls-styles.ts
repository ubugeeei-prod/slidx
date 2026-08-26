/** The Slide inspector's authored delivery decisions. */
export const DELIVERY_CONTROL_STYLESHEET = `
.slidx-delivery {
  display: grid;
  gap: var(--slidx-e-gap);
  margin-bottom: var(--slidx-e-loose);
}

.slidx-delivery-label,
.slidx-delivery-name {
  display: block;
  color: var(--slidx-e-muted);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.slidx-delivery-label {
  padding-bottom: var(--slidx-e-snug);
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-delivery-name { margin-bottom: var(--slidx-e-snug); }

.slidx-delivery-notice {
  margin: 0 0 var(--slidx-e-snug);
  color: var(--slidx-e-warning);
  font-size: 11px;
}

.slidx-transition-choices {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--slidx-e-tight);
}

.slidx-transition-choice {
  display: grid;
  grid-template-columns: 42px minmax(0, 1fr);
  align-items: center;
  gap: var(--slidx-e-snug);
  min-width: 0;
  min-height: 50px;
  padding: var(--slidx-e-tight);
  text-align: left;
}

.slidx-transition-choice[data-transition="inherit"] { grid-column: 1 / -1; }
.slidx-transition-choice[aria-pressed="true"] { border-color: var(--slidx-e-accent); }

.slidx-transition-copy {
  display: grid;
  min-width: 0;
  line-height: 1.35;
}

.slidx-transition-copy strong {
  overflow: hidden;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-transition-copy > span { display: none; }

.slidx-transition-detail {
  min-height: 1.45em;
  margin: var(--slidx-e-tight) 0 0;
  color: var(--slidx-e-muted);
  font-size: 10px;
  line-height: 1.45;
}

.slidx-transition-preview {
  position: relative;
  display: block;
  width: 42px;
  height: 32px;
  overflow: hidden;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: 2px;
  background: var(--slidx-e-canvas);
}

.slidx-transition-preview-from,
.slidx-transition-preview-to {
  position: absolute;
  inset: 3px;
  display: grid;
  align-content: center;
  gap: 3px;
  padding: 4px;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  background: var(--slidx-e-surface);
}

.slidx-transition-preview i {
  display: block;
  height: 1px;
  background: var(--slidx-e-muted);
}

.slidx-transition-preview-to {
  border-color: color-mix(in srgb, var(--slidx-e-accent) 50%, var(--slidx-e-line));
  background: color-mix(in srgb, var(--slidx-e-accent) 18%, var(--slidx-e-canvas));
}

.slidx-transition-preview[data-transition-preview="none"] .slidx-transition-preview-from {
  right: calc(50% + 1px);
}
.slidx-transition-preview[data-transition-preview="none"] .slidx-transition-preview-to {
  left: calc(50% + 1px);
}
.slidx-transition-preview[data-transition-preview="fade"] .slidx-transition-preview-from { opacity: 0.42; }
.slidx-transition-preview[data-transition-preview="fade"] .slidx-transition-preview-to { opacity: 0.66; }
.slidx-transition-preview[data-transition-preview="slide"] .slidx-transition-preview-to { left: 14px; }
.slidx-transition-preview[data-transition-preview="push"] .slidx-transition-preview-from { right: 19px; }
.slidx-transition-preview[data-transition-preview="push"] .slidx-transition-preview-to { left: 19px; }
.slidx-transition-preview[data-transition-preview="wipe"] .slidx-transition-preview-to { clip-path: inset(0 40% 0 0); }
.slidx-transition-preview[data-transition-preview="rise"] .slidx-transition-preview-from { bottom: 19px; }
.slidx-transition-preview[data-transition-preview="rise"] .slidx-transition-preview-to { top: 19px; }
.slidx-transition-preview[data-transition-preview="inherit"] .slidx-transition-preview-to {
  inset: 8px 4px 4px 12px;
  border-style: dashed;
}

.slidx-budget-choices {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--slidx-e-tight);
}

.slidx-budget-choice {
  min-width: 0;
  padding-inline: var(--slidx-e-tight);
  color: var(--slidx-e-muted);
  font-size: 10px;
}

.slidx-budget-choice[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
  color: var(--slidx-e-text);
}

.slidx-budget-entry {
  display: grid;
  grid-template-columns: 54px minmax(0, 1fr);
  align-items: center;
  gap: var(--slidx-e-snug);
  margin-top: var(--slidx-e-snug);
  color: var(--slidx-e-muted);
  font-size: 11px;
}

.slidx-budget-custom {
  padding-block: var(--slidx-e-tight);
  font-variant-numeric: tabular-nums;
}

.slidx-budget-status {
  margin: var(--slidx-e-tight) 0 0 62px;
  color: var(--slidx-e-muted);
  font-size: 10px;
  line-height: 1.45;
}

.slidx-budget-status[data-state="over"],
.slidx-budget-status[data-state="invalid"] { color: var(--slidx-e-warning); }

.slidx-optional-choice {
  display: grid;
  grid-template-columns: 42px minmax(0, 1fr);
  align-items: center;
  gap: var(--slidx-e-snug);
  min-height: 54px;
  padding: var(--slidx-e-snug);
  text-align: left;
}

.slidx-optional-choice[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
  background: color-mix(in srgb, var(--slidx-e-accent) 8%, transparent);
}

.slidx-optional-mark {
  color: var(--slidx-e-muted);
  font-family: var(--slidx-e-font-mono);
  font-size: 9px;
  letter-spacing: 0.08em;
  text-align: center;
}

.slidx-optional-choice[aria-pressed="true"] .slidx-optional-mark { color: var(--slidx-e-accent); }

.slidx-optional-copy { display: grid; min-width: 0; line-height: 1.35; }
.slidx-optional-copy strong { font-size: 11px; }
.slidx-optional-copy span { color: var(--slidx-e-muted); font-size: 9px; }

@media (max-width: 56em) {
  .slidx-transition-choices { grid-template-columns: 1fr; }
  .slidx-budget-choices { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
`;

export function applyDeliveryControlStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-delivery-controls]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-delivery-controls", "");
  style.textContent = DELIVERY_CONTROL_STYLESHEET;
  document.head.append(style);
}
