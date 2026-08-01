/** Collaboration in the command bar, with the full roster revealed on demand. */

export const PRESENCE_STYLESHEET = `
.slidx-presence {
  position: relative;
  flex: none;
}

.slidx-presence[data-empty="true"] { display: none; }

.slidx-presence-toggle {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-snug);
  min-width: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-snug);
  border-color: transparent;
  color: var(--slidx-e-muted);
  font-size: 11px;
}

.slidx-presence-toggle:hover,
.slidx-presence[data-open="true"] .slidx-presence-toggle {
  background: var(--slidx-e-surface);
  color: var(--slidx-e-text);
}

.slidx-presence-avatars {
  display: flex;
  align-items: center;
}

.slidx-presence-avatar {
  display: grid;
  place-items: center;
  width: var(--slidx-e-lockup);
  height: var(--slidx-e-lockup);
  margin-left: calc(var(--slidx-e-tight) * -1);
  border: var(--slidx-e-hairline) solid var(--slidx-e-canvas);
  border-radius: 50%;
  background: var(--slidx-e-accent);
  color: var(--slidx-e-canvas);
  font-size: 11px;
  font-weight: 650;
  line-height: 1;
}

.slidx-presence-avatar:first-child { margin-left: 0; }
.slidx-presence-count { font-variant-numeric: tabular-nums; white-space: nowrap; }

.slidx-presence-popover {
  position: absolute;
  top: calc(100% + var(--slidx-e-tight));
  right: 0;
  z-index: 10;
  width: min(22rem, calc(100vw - var(--slidx-e-gap) - var(--slidx-e-gap)));
  max-height: min(70vh, 30rem);
  overflow-y: auto;
  padding: var(--slidx-e-gap);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
}

.slidx-presence-popover[hidden] { display: none; }

.slidx-presence-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--slidx-e-gap);
  padding: var(--slidx-e-snug);
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-presence-label {
  font-size: 11px;
  font-weight: 650;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.slidx-presence-total { color: var(--slidx-e-muted); font-size: 11px; }

.slidx-presence-list {
  display: flex;
  flex-direction: column;
  gap: var(--slidx-e-tight);
  margin: var(--slidx-e-snug) 0 0;
  padding: 0;
  list-style: none;
}

.slidx-presence-who { min-width: 0; }

.slidx-presence-seat {
  display: flex;
  align-items: baseline;
  gap: var(--slidx-e-snug);
  width: 100%;
  min-height: var(--slidx-e-hit);
  margin: 0;
  padding: 0 var(--slidx-e-snug);
  border: var(--slidx-e-hairline) solid transparent;
  border-radius: var(--slidx-e-radius);
  background: transparent;
  color: inherit;
  font: inherit;
}

button.slidx-presence-seat { cursor: pointer; }
button.slidx-presence-seat:hover { background: var(--slidx-e-surface); }
.slidx-presence-seat[aria-pressed="true"] {
  border-color: var(--slidx-e-accent);
  background: var(--slidx-e-accent);
  color: var(--slidx-e-canvas);
}

.slidx-presence-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-presence-seat[aria-pressed="true"] .slidx-presence-where,
.slidx-presence-seat[aria-pressed="true"] .slidx-presence-role { color: inherit; }
.slidx-presence-seat[data-local="true"] .slidx-presence-name { color: var(--slidx-e-accent); }
.slidx-presence-where { flex: none; color: var(--slidx-e-muted); }
.slidx-presence-role { flex: none; color: var(--slidx-e-muted); font-size: 11px; }

@media (max-width: 47.5em) {
  .slidx-presence-count-suffix { display: none; }
}
`;

export function applyPresenceStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-presence]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-presence", "");
  style.textContent = PRESENCE_STYLESHEET;
  document.head.append(style);
}
