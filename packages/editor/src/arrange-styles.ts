/**
 * The arrange overlay's own stylesheet.
 *
 * Its own rather than a block appended to [`styles`](./styles) for the reason
 * the timeline's is: a surface that claims part of the window should be the
 * thing that says so. The custom properties are the ones `styles` already
 * defines, so the two agree about colour without either importing the other.
 *
 * # Why it is fixed rather than a panel
 *
 * Everything here is drawn over the canvas frame in the editor's own document,
 * at coordinates read from inside it. The deck page is left exactly as the
 * build emits it — no handle, no outline, not one attribute — because the whole
 * claim the canvas makes is that it is the page, and an editor that decorated
 * it would be previewing something nobody will ever see.
 *
 * # Why almost nothing is drawn until a drag starts
 *
 * A grip on every block and a box around every region, permanently, is a canvas
 * an author cannot read their own slide on. So the grips are small and quiet
 * until they are hovered or focused, and the regions appear only while
 * something is moving between them.
 */

export const ARRANGE_STYLESHEET = `
.slidx-arrange {
  position: fixed;
  inset: 0;
  z-index: 5;
  /* The layer is a drawing, not a target. Only the grips answer a pointer. */
  pointer-events: none;
}

.slidx-arrange[data-freeform-selection="true"] .slidx-arrange-grips {
  display: none;
}

.slidx-arrange-region, .slidx-arrange-safe, .slidx-arrange-drop, .slidx-arrange-ghost {
  position: fixed;
  opacity: 0;
}

.slidx-arrange[data-dragging="true"] .slidx-arrange-region,
.slidx-arrange[data-dragging="true"] .slidx-arrange-safe,
.slidx-arrange[data-dragging="true"] .slidx-arrange-drop,
.slidx-arrange[data-dragging="true"] .slidx-arrange-ghost {
  opacity: 1;
}

/*
 * A region, while something is being dragged between them.
 *
 * The boundary is the information — this is the box the block will be measured
 * against once it lands — so it is a hairline and a name, and no fill.
 */
.slidx-arrange-region {
  border: var(--slidx-e-hairline) dashed var(--slidx-e-line);
}

.slidx-arrange-region[data-over="true"] { border-color: var(--slidx-e-accent); }

.slidx-arrange-name {
  position: absolute;
  top: 2px;
  left: var(--slidx-e-tight);
  color: var(--slidx-e-muted);
  font-size: 11px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.slidx-arrange-region[data-over="true"] .slidx-arrange-name { color: var(--slidx-e-accent); }

/* The safe area: the edge the room takes and the deck never gets back. */
.slidx-arrange-safe { border: var(--slidx-e-hairline) solid var(--slidx-e-warning); }

/* Where the block will go. The one solid line on the overlay. */
.slidx-arrange-drop {
  height: 2px;
  background: var(--slidx-e-accent);
}

.slidx-arrange-ghost { border: var(--slidx-e-hairline) solid var(--slidx-e-accent); }

/*
 * An alignment the block currently agrees with, drawn the full length of the
 * slide so it reads as a line rather than as an edge of something.
 */
.slidx-arrange-guide {
  position: fixed;
  background: var(--slidx-e-accent);
  opacity: 0.6;
}

.slidx-arrange-guide[data-kind="safe"] { background: var(--slidx-e-warning); }
.slidx-arrange-guide[data-axis="x"] { width: var(--slidx-e-hairline); }
.slidx-arrange-guide[data-axis="y"] { height: var(--slidx-e-hairline); }

/*
 * The grip: the whole gesture, in one control per block.
 *
 * A button rather than a bare element, so the tab order reaches every block on
 * the slide and the arrow keys can move one without a pointer ever being
 * involved. The target is the chrome's own hit size while the mark inside it
 * stays small, because a handle quiet enough not to cover a slide still has to
 * be easy to hit — those are two different measurements and only the visible
 * one has to recede.
 *
 * It straddles the block's top-left corner rather than sitting inside it: a
 * target laid over the first word of a heading is a target that stops the
 * heading being clicked, and the heading is edited in place.
 */
.slidx-arrange-grip {
  position: fixed;
  display: grid;
  place-items: center;
  width: var(--slidx-e-hit);
  height: var(--slidx-e-hit);
  min-height: var(--slidx-e-hit);
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--slidx-e-muted);
  font-size: 11px;
  line-height: 1;
  cursor: grab;
  opacity: 0.14;
  pointer-events: auto;
}

.slidx-arrange-grip > span { pointer-events: none; }

.slidx-arrange-grip:hover, .slidx-arrange-grip:focus-visible { opacity: 1; }
.slidx-arrange-grip:hover { background: transparent; }

.slidx-arrange-grip[data-moving="true"] {
  opacity: 1;
  color: var(--slidx-e-accent);
  cursor: grabbing;
}

/* The one state change worth animating: a grip coming forward under a cursor. */
@media (prefers-reduced-motion: no-preference) {
  .slidx-arrange-grip { transition: opacity 90ms linear; }
}

/*
 * What just happened, for a reader who cannot see the ghost.
 *
 * Off screen rather than hidden: display none and visibility hidden are both
 * removed from the accessibility tree, so a live region spelled either way
 * announces nothing at all.
 */
.slidx-arrange-status {
  position: fixed;
  width: 1px;
  height: 1px;
  margin: -1px;
  padding: 0;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}
`;

/** Puts the overlay's stylesheet into a document, once. */
export function applyArrangeStyles(document: Document): void {
  if (document.querySelector("style[data-slidx-arrange]")) return;

  const style = document.createElement("style");
  style.setAttribute("data-slidx-arrange", "");
  style.textContent = ARRANGE_STYLESHEET;
  document.head.append(style);
}
