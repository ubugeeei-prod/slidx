/**
 * Applying a compiled frame to a slide.
 *
 * The compiler emits complete state snapshots rather than deltas, so this
 * module is deliberately simple: `apply(n)` writes the whole slide's state
 * from frame `n`. There is no history to unwind and no delta to invert, which
 * is why stepping forward, stepping back, jumping to a deep link, and printing
 * are the same operation with a different argument — and therefore cannot
 * disagree with each other.
 *
 * Animation is expressed as attributes and custom properties, never as
 * scripted motion. Keeping it in CSS is what lets the compositor run it, which
 * is the difference between smooth and juddering on venue hardware.
 */

import { findAnchors, resolveAnchor } from "./anchor";
import type { Effect, StepFrame, StepTimeline } from "./types";

/** Present on every element the pipeline controls. */
export const STAGED_ATTRIBUTE = "data-slidx-staged";

/** Present while an element is not yet revealed. */
export const HIDDEN_ATTRIBUTE = "data-slidx-hidden";

const EFFECT_ATTRIBUTE = "data-slidx-effect";
const EFFECT_KIND_ATTRIBUTE = "data-slidx-effect-kind";

/** A slide's steps, bound to the DOM. */
export interface Stage {
  /** Number of stops, including the resting frame. Always at least one. */
  readonly stopCount: number;
  /** The stop currently shown. */
  readonly index: number;
  /** Shows a stop, clamping out-of-range indices. Returns the stop shown. */
  apply(index: number): number;
  /** Shows every element that was ever revealed, with no animation. */
  applyPrint(): void;
}

/**
 * Binds a timeline to a slide.
 *
 * Anchors are resolved once, here, rather than on every step: resolution
 * mutates the DOM (it removes the wrapper in the block-marker case), so
 * repeating it would give different answers each time.
 */
export function createStage(root: HTMLElement, timeline: StepTimeline): Stage {
  const elements = bind(root, timeline);
  const frames = timeline.frames.length > 0 ? timeline.frames : [{ index: 0, states: [] }];

  for (const element of elements.values()) {
    element.setAttribute(STAGED_ATTRIBUTE, "");
  }

  let current = 0;
  applyFrame(elements, frames[0]!, { animate: false });

  return {
    get stopCount() {
      return frames.length;
    },
    get index() {
      return current;
    },
    apply(index: number): number {
      const next = Math.max(0, Math.min(index, frames.length - 1));

      // Re-applying the stop already shown must not touch the DOM. Mirrored
      // windows, resize handlers, and URL syncs all re-assert the current
      // position, and clearing the effect attributes would cut short an
      // animation that is still playing.
      if (next === current) return next;

      // Effects belong to the moment a stop is reached going forward. Playing
      // them on the way back would re-run an animation the audience already
      // watched, which reads as a mistake rather than as emphasis.
      applyFrame(elements, frames[next]!, { animate: next > current });
      current = next;
      return next;
    },
    applyPrint() {
      applyFrame(elements, printFrame(frames), { animate: false });
      current = frames.length - 1;
    },
  };
}

/**
 * Maps every selector the timeline mentions to the element it stages.
 *
 * Anchor selectors are resolved through the anchor contract; anything else is
 * an author-written selector and is queried directly. A selector that matches
 * nothing is dropped rather than throwing — a stale `steps:` entry should cost
 * an animation, not the slide.
 */
function bind(root: HTMLElement, timeline: StepTimeline): Map<string, HTMLElement> {
  const elements = new Map<string, HTMLElement>();

  for (const anchor of findAnchors(root)) {
    const selector = `[data-slidx-step="${anchor.getAttribute("data-slidx-step")}"]`;
    const staged = resolveAnchor(root, anchor);
    if (staged) elements.set(selector, staged);
  }

  for (const frame of timeline.frames) {
    for (const state of frame.states) {
      if (elements.has(state.target)) continue;

      const found = query(root, state.target);
      if (found) elements.set(state.target, found);
    }
  }

  return elements;
}

function query(root: HTMLElement, selector: string): HTMLElement | null {
  try {
    return root.querySelector<HTMLElement>(selector);
  } catch {
    // An invalid selector is an authoring mistake, reported by the linter.
    // At presentation time it must simply do nothing.
    return null;
  }
}

function applyFrame(
  elements: Map<string, HTMLElement>,
  frame: StepFrame,
  { animate }: { animate: boolean },
): void {
  for (const element of elements.values()) {
    clearEffect(element);
  }

  for (const state of frame.states) {
    const element = elements.get(state.target);
    if (!element) continue;

    element.toggleAttribute(HIDDEN_ATTRIBUTE, state.visibility === "hidden");

    if (animate && state.effect) {
      setEffect(element, state.effect);
    }
  }
}

/**
 * The union of every frame, for print and PDF export.
 *
 * Anything that was ever on screen is shown, because a handout that hides
 * content the audience saw is worse than one that shows a little more. This
 * mirrors `StepTimeline::print_frame` in Rust.
 */
function printFrame(frames: StepFrame[]): StepFrame {
  const states = new Map<string, { target: string; visibility: "hidden" | "visible" }>();

  for (const frame of frames) {
    for (const state of frame.states) {
      const existing = states.get(state.target);
      if (!existing) {
        states.set(state.target, { target: state.target, visibility: state.visibility });
      } else if (state.visibility === "visible") {
        existing.visibility = "visible";
      }
    }
  }

  return { index: frames.length - 1, states: Array.from(states.values()) };
}

function setEffect(element: HTMLElement, effect: Effect): void {
  element.setAttribute(EFFECT_ATTRIBUTE, effect.preset);
  element.setAttribute(EFFECT_KIND_ATTRIBUTE, effect.kind);
  element.style.setProperty("--slidx-effect-duration", `${effect.durationMs}ms`);
  element.style.setProperty("--slidx-effect-delay", `${effect.delayMs}ms`);
  element.style.setProperty("--slidx-effect-easing", `var(--slidx-easing-${effect.easing})`);

  if (effect.origin) {
    element.style.setProperty("--slidx-effect-origin", effect.origin);
  } else {
    element.style.removeProperty("--slidx-effect-origin");
  }
}

function clearEffect(element: HTMLElement): void {
  element.removeAttribute(EFFECT_ATTRIBUTE);
  element.removeAttribute(EFFECT_KIND_ATTRIBUTE);
  element.style.removeProperty("--slidx-effect-duration");
  element.style.removeProperty("--slidx-effect-delay");
  element.style.removeProperty("--slidx-effect-easing");
  element.style.removeProperty("--slidx-effect-origin");

  // An element with no inline properties left should not carry an empty
  // `style=""`, which would otherwise make `apply` non-idempotent in the DOM.
  if (element.getAttribute("style") === "") element.removeAttribute("style");
}
