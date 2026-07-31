/** Keyboard reference and its quiet, always reachable launcher. */
export const SHORTCUT_STYLES = `
.slidx-shortcuts {
  position: fixed;
  right: var(--slidx-e-gap);
  bottom: var(--slidx-e-gap);
  z-index: 40;
}

.slidx-shortcuts-open,
.slidx-shortcuts-close {
  min-width: var(--slidx-e-hit);
  min-height: var(--slidx-e-hit);
  padding: var(--slidx-e-tight) var(--slidx-e-snug);
  color: var(--slidx-e-muted);
  background: var(--slidx-e-surface);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
}

.slidx-shortcuts-open:hover,
.slidx-shortcuts-close:hover {
  color: var(--slidx-e-text);
  border-color: var(--slidx-e-muted);
}

.slidx-shortcuts-dialog {
  position: fixed;
  inset: 50% auto auto 50%;
  width: min(38rem, calc(100vw - var(--slidx-e-loose) - var(--slidx-e-loose)));
  max-height: calc(100vh - var(--slidx-e-loose) - var(--slidx-e-loose));
  overflow: auto;
  transform: translate(-50%, -50%);
  color: var(--slidx-e-text);
  background: var(--slidx-e-canvas);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
}

.slidx-shortcuts-dialog[hidden] { display: none; }

.slidx-shortcuts-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--slidx-e-gap) var(--slidx-e-loose);
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-shortcuts-head h2 { margin: 0; font-size: 1rem; }

.slidx-shortcut-groups {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--slidx-e-loose);
  padding: var(--slidx-e-gap) var(--slidx-e-loose) var(--slidx-e-loose);
}

.slidx-shortcut-group h3 {
  margin: 0;
  padding-bottom: var(--slidx-e-tight);
  color: var(--slidx-e-text);
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
  font-size: .75rem;
  letter-spacing: .08em;
  text-transform: uppercase;
}

.slidx-shortcuts-list { margin: 0; }

.slidx-shortcut {
  display: grid;
  grid-template-columns: minmax(7rem, auto) 1fr;
  gap: var(--slidx-e-gap);
  align-items: center;
  min-height: calc(var(--slidx-e-hit) + var(--slidx-e-snug));
  border-bottom: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-shortcut dt,
.slidx-shortcut dd { margin: 0; }

.slidx-shortcut dd { color: var(--slidx-e-muted); }

.slidx-shortcut kbd {
  display: inline-block;
  min-width: 1.5rem;
  margin-right: var(--slidx-e-tight);
  padding: 2px var(--slidx-e-tight);
  color: var(--slidx-e-text);
  background: var(--slidx-e-surface);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  font: 12px/1.4 var(--slidx-e-font-mono);
  text-align: center;
}

@media (max-width: 40rem) {
  .slidx-shortcut-groups { grid-template-columns: 1fr; }
}
`;
