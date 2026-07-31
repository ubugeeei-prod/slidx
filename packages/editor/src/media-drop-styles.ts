/** The quiet, generous target shown while image or video files cross the canvas. */

export const MEDIA_DROP_STYLESHEET = `
.slidx-media-drop {
  position: fixed;
  z-index: 9;
  display: grid;
  place-items: center;
  padding: var(--slidx-e-loose);
  border: var(--slidx-e-hairline) solid transparent;
  pointer-events: none;
}

/*
 * The scrim belongs to the active state and nothing else.
 *
 * It used to be on the rule above, which paints whether or not anything is
 * being dragged — and this element is sized to cover the canvas, so a 72%
 * backdrop sat over every slide, always. Only its *children* were hidden when
 * inactive, so the overlay looked switched off while dimming the one thing in
 * the editor an author is trying to judge: the slide's own colours.
 *
 * It was reported as "not black, just dark, like something is on top of it",
 * which is exactly what it was.
 */
.slidx-media-drop[data-active="true"] {
  background: color-mix(in srgb, var(--slidx-e-canvas) 72%, transparent);
  border-color: color-mix(in srgb, var(--slidx-e-accent) 44%, transparent);
}

.slidx-media-drop[data-active="false"] > :not(.slidx-media-drop-status) {
  display: none;
}

.slidx-media-drop-region,
.slidx-media-drop-line {
  position: fixed;
  pointer-events: none;
}

.slidx-media-drop-region {
  border: var(--slidx-e-hairline) solid
    color-mix(in srgb, var(--slidx-e-accent) 34%, transparent);
}

.slidx-media-drop-line {
  height: 2px;
  background: var(--slidx-e-accent);
}

.slidx-media-drop[data-target=""] .slidx-media-drop-region,
.slidx-media-drop[data-target=""] .slidx-media-drop-line {
  display: none;
}

.slidx-media-drop-message {
  position: relative;
  min-width: min(280px, 100%);
  max-width: 36rem;
  min-height: calc(var(--slidx-e-hit) + var(--slidx-e-snug));
  padding: var(--slidx-e-gap) var(--slidx-e-loose);
  border: var(--slidx-e-hairline) solid var(--slidx-e-line);
  border-radius: var(--slidx-e-radius);
  background: var(--slidx-e-canvas);
  color: var(--slidx-e-text);
  font: inherit;
  line-height: 1.5;
  text-align: center;
}

.slidx-media-drop[data-target=""] .slidx-media-drop-message {
  color: var(--slidx-e-muted);
}

.slidx-media-drop[data-error="true"] .slidx-media-drop-message {
  border-color: color-mix(in srgb, var(--slidx-e-error) 54%, var(--slidx-e-line));
  color: var(--slidx-e-error);
  cursor: pointer;
  pointer-events: auto;
}

.slidx-media-drop[data-busy="true"] .slidx-media-drop-message {
  color: var(--slidx-e-accent);
}

.slidx-media-drop-status {
  position: fixed;
  width: 1px;
  height: 1px;
  margin: -1px;
  overflow: hidden;
  clip-path: inset(50%);
}
`;

export function applyMediaDropStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-media-drop]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-media-drop", "");
  style.textContent = MEDIA_DROP_STYLESHEET;
  document.head.append(style);
}
