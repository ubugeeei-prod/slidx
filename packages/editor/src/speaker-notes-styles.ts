/** The current slide's speaking surface, kept immediately under its canvas. */
export const SPEAKER_NOTES_STYLESHEET = `
.slidx-speaker-notes {
  flex: 0 0 auto;
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
  background: color-mix(in srgb, var(--slidx-e-surface) 58%, var(--slidx-e-canvas));
}

.slidx-speaker-notes-head {
  display: flex;
  align-items: center;
  min-height: 34px;
  padding-right: var(--slidx-e-gap);
}

.slidx-speaker-notes-toggle {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--slidx-e-snug);
  min-width: 0;
  padding-inline: var(--slidx-e-gap);
  border: 0;
  color: var(--slidx-e-text);
  text-align: left;
}

.slidx-speaker-notes-toggle::before {
  content: "";
  width: 2px;
  height: 12px;
  background: var(--slidx-e-accent);
}

.slidx-speaker-notes-toggle::after {
  content: "⌃";
  margin-left: auto;
  color: var(--slidx-e-muted);
  font-size: 11px;
}

.slidx-speaker-notes[data-open="false"] .slidx-speaker-notes-toggle::after { content: "⌄"; }

.slidx-speaker-notes-title {
  overflow: hidden;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-overflow: ellipsis;
  text-transform: uppercase;
  white-space: nowrap;
}

.slidx-speaker-notes-key {
  color: var(--slidx-e-muted);
  font-family: var(--slidx-e-font-mono);
  font-size: 10px;
}

.slidx-speaker-notes-state {
  flex: 0 0 auto;
  color: var(--slidx-e-muted);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.slidx-speaker-notes-state[data-state="dirty"] { color: var(--slidx-e-warning); }
.slidx-speaker-notes-state[data-state="problem"] { color: var(--slidx-e-error); }

.slidx-speaker-notes-body {
  padding: 0 var(--slidx-e-loose) var(--slidx-e-gap);
}

.slidx-speaker-notes[data-open="false"] .slidx-speaker-notes-body { display: none; }

.slidx-speaker-notes-input {
  display: block;
  min-height: 76px;
  max-height: 24vh;
  resize: vertical;
  background: var(--slidx-e-canvas);
  line-height: 1.65;
}

@media (max-width: 56em) {
  .slidx-speaker-notes-body { padding-inline: var(--slidx-e-gap); }
  .slidx-speaker-notes-key { display: none; }
}

@media (max-height: 44em) {
  .slidx-speaker-notes-body { padding-bottom: var(--slidx-e-snug); }
  .slidx-speaker-notes-input { min-height: 56px; max-height: 18vh; }
}
`;

export function applySpeakerNotesStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-speaker-notes]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-speaker-notes", "");
  style.textContent = SPEAKER_NOTES_STYLESHEET;
  document.head.append(style);
}
