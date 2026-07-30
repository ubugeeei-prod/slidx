/**
 * The history panel's own stylesheet.
 *
 * Its own rather than a block appended to [`styles`](./styles), for the reason
 * the timeline's is: a panel that claims space should be the thing that says
 * so. This one claims a **rail** rather than a row — 30 pixels down the right
 * edge, taken out of the editor's padding rather than out of its grid, so it
 * composes with any panel that does claim a row instead of fighting it for the
 * same declaration.
 *
 * It opens over the inspector rather than beside it. History is something an
 * author consults and closes, not something they work next to all day, and a
 * fourth permanent column would take width from the canvas — the one surface
 * that is showing the actual talk.
 */

export const REVISIONS_STYLESHEET = `
/*
 * The rail the panel lives on. Padding rather than a grid column: the grid is
 * where panels that need a row of their own go, and two panels editing the
 * same \`grid-template-areas\` would silently drop whichever applied first.
 */
.slidx-editor { padding-right: 30px; }

.slidx-revisions {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  width: 30px;
  display: flex;
  background: var(--slidx-e-canvas);
  border-left: var(--slidx-e-hairline) solid var(--slidx-e-line);
  z-index: 2;
}

.slidx-revisions[data-open="true"] { width: min(400px, 92vw); }

/* The rail itself: the one thing visible when the panel is closed. */
.slidx-revisions-tab {
  flex: none;
  width: 30px;
  padding: 10px 0;
  border: 0;
  border-radius: 0;
  color: var(--slidx-e-muted);
  writing-mode: vertical-rl;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.slidx-revisions[data-open="true"] .slidx-revisions-tab {
  color: var(--slidx-e-text);
  background: var(--slidx-e-surface);
}

.slidx-revisions-panel {
  flex: 1;
  min-width: 0;
  display: none;
  flex-direction: column;
  border-left: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-revisions[data-open="true"] .slidx-revisions-panel { display: flex; }

.slidx-revisions-body { flex: 1; min-height: 0; overflow-y: auto; }

/*
 * What to say when there is no history. A deck in a directory nobody ran
 * \`git init\` in is ordinary, so this is prose in the panel's own voice rather
 * than an error state with a colour.
 */
.slidx-revisions-note { margin: 0; padding: var(--slidx-e-gap); color: var(--slidx-e-muted); }

.slidx-revisions-list { margin: 0; padding: 4px; list-style: none; }

.slidx-revision + .slidx-revision {
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
}

.slidx-revision-open {
  display: block;
  width: 100%;
  padding: 7px 8px;
  border: 0;
  border-radius: var(--slidx-e-radius);
  text-align: left;
}

.slidx-revision-open:hover { background: var(--slidx-e-surface); }
.slidx-revision[aria-current="true"] .slidx-revision-open { background: var(--slidx-e-surface); }

.slidx-revision-subject {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slidx-revision-meta {
  display: block;
  margin-top: 1px;
  color: var(--slidx-e-muted);
  font-size: 12px;
}

/*
 * What the deck says the commit did, under what the author called it. Indented
 * to the subject above it, because it is an answer about that row.
 */
.slidx-revision-change { padding: 0 8px 10px 8px; }

.slidx-revision-headline {
  margin: 0;
  font-weight: 600;
}

.slidx-revision-lines {
  margin: 4px 0 0;
  padding-left: 16px;
  color: var(--slidx-e-muted);
}

.slidx-revision-lines li { margin-top: 1px; }

.slidx-revision-quiet { margin: 0; color: var(--slidx-e-muted); }

/*
 * The deck as the selected commit had it.
 *
 * Below the list rather than inside the row it belongs to, for two reasons:
 * moving an iframe between parents reloads it in some browsers, and a preview
 * that stayed put while the list scrolled is easier to compare against than one
 * that moves with the row.
 */
.slidx-revisions-preview {
  flex: none;
  border-top: var(--slidx-e-hairline) solid var(--slidx-e-line);
  padding: var(--slidx-e-gap);
}

.slidx-revisions-preview[data-showing="false"] { display: none; }

.slidx-revision-frame {
  display: block;
  width: 100%;
  aspect-ratio: 16 / 9;
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
}

.slidx-revision-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

/*
 * Restoring is not the panel's headline. Reading history is what an author came
 * for, and going back is what they might then decide to do — so it reads as a
 * button rather than as the thing the panel is for.
 */
.slidx-revision-restore, .slidx-revision-undo { flex: none; }

.slidx-revision-said {
  flex: 1 1 100%;
  margin: 0;
  color: var(--slidx-e-muted);
}

@media (prefers-reduced-motion: no-preference) {
  .slidx-revisions { transition: width 120ms ease-out; }
}
`;

/** Puts the history panel's stylesheet into a document, once. */
export function applyRevisionsStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-revisions]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-revisions", "");
  style.textContent = REVISIONS_STYLESHEET;
  document.head.append(style);
}
