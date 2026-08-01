/** Adaptive workspace rails and canvas focus mode. */

export const WORKSPACE_FOCUS_STYLESHEET = `
.slidx-workspace-controls {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-tight);
}

.slidx-workspace-focus {
  display: flex;
  align-items: center;
  gap: var(--slidx-e-tight);
  min-width: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-snug);
  border-color: transparent;
  color: var(--slidx-e-muted);
  white-space: nowrap;
}

.slidx-workspace-focus:hover,
.slidx-workspace-focus[data-active="true"] { color: var(--slidx-e-text); }

.slidx-workspace-focus[data-active="true"] {
  border-color: var(--slidx-e-accent);
  background: var(--slidx-e-surface);
}

.slidx-workspace-focus-icon {
  width: 12px;
  height: 12px;
  border: var(--slidx-e-hairline) solid currentColor;
  border-radius: 2px;
}

.slidx-workspace-panel {
  display: none;
  align-items: center;
  gap: var(--slidx-e-tight);
  min-width: var(--slidx-e-hit);
  padding: 0 var(--slidx-e-snug);
  border-color: transparent;
  color: var(--slidx-e-muted);
  white-space: nowrap;
}

.slidx-workspace-panel:hover,
.slidx-workspace-panel[data-active="true"] { color: var(--slidx-e-text); }

.slidx-workspace-panel[data-active="true"] {
  border-color: var(--slidx-e-line);
  background: var(--slidx-e-surface);
}

.slidx-workspace-panel-icon {
  position: relative;
  width: 13px;
  height: 12px;
  border: var(--slidx-e-hairline) solid currentColor;
  border-radius: 2px;
}

.slidx-workspace-panel-icon::after {
  content: "";
  position: absolute;
  top: 0;
  bottom: 0;
  border-left: var(--slidx-e-hairline) solid currentColor;
}

.slidx-workspace-panel-icon[data-panel="outline"]::after { left: var(--slidx-e-tight); }
.slidx-workspace-panel-icon[data-panel="inspector"]::after { right: var(--slidx-e-tight); }

/*
 * More specific than the panel-owned grid rules loaded after the base chrome.
 * Timeline owns an extra row; focus mode owns the temporary absence of it.
 */
.slidx-editor.slidx-editor[data-canvas-focus="true"] {
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: 34px minmax(0, 1fr);
  grid-template-areas: "appbar" "canvas";
}

.slidx-editor[data-canvas-focus="true"] > .slidx-outline,
.slidx-editor[data-canvas-focus="true"] > .slidx-inspector,
.slidx-editor[data-canvas-focus="true"] > .slidx-timeline,
.slidx-editor[data-canvas-focus="true"] > .slidx-diagnostics { display: none; }

.slidx-editor[data-canvas-focus="true"] > .slidx-canvas { min-width: 0; }

@media (max-width: 75em) {
  .slidx-workspace-focus-label { display: none; }
  .slidx-workspace-focus { padding: 0 var(--slidx-e-snug); }
}

/*
 * Below a full desktop width, one side panel is a choice rather than a tax on
 * the slide. The selected rail stays beside the real canvas, and the two appbar
 * controls make the hidden rail one click away. This is a grid reflow, not an
 * overlay: nothing obscures the content an author is actively changing.
 */
@media (max-width: 64em) {
  .slidx-workspace-panel { display: flex; }
  .slidx-workspace-panel-label { display: none; }

  .slidx-editor.slidx-editor[data-workspace-panel="outline"]:not([data-canvas-focus="true"]) {
    grid-template-columns: clamp(9.5rem, 22vw, 11rem) minmax(0, 1fr);
    grid-template-areas:
      "appbar appbar"
      "outline canvas"
      "timeline timeline"
      "findings findings";
  }

  .slidx-editor.slidx-editor[data-workspace-panel="inspector"]:not([data-canvas-focus="true"]) {
    grid-template-columns: minmax(0, 1fr) clamp(12rem, 28vw, 15rem);
    grid-template-areas:
      "appbar appbar"
      "canvas inspector"
      "timeline timeline"
      "findings findings";
  }

  .slidx-editor.slidx-editor[data-workspace-panel="canvas"]:not([data-canvas-focus="true"]) {
    grid-template-columns: minmax(0, 1fr);
    grid-template-areas: "appbar" "canvas" "timeline" "findings";
  }

  .slidx-editor:not([data-workspace-panel="outline"]) > .slidx-outline,
  .slidx-editor:not([data-workspace-panel="inspector"]) > .slidx-inspector { display: none; }

  .slidx-editor[data-access="read"] .slidx-workspace-panel[data-panel="inspector"] {
    display: none;
  }
}
`;

export function applyWorkspaceFocusStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-workspace-focus]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-workspace-focus", "");
  style.textContent = WORKSPACE_FOCUS_STYLESHEET;
  document.head.append(style);
}
