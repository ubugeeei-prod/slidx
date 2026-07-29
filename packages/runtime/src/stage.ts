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
import { setContent, setProperties } from "./patch";
import type { Effect, ElementState, StepFrame, StepTimeline } from "./types";

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

  // The markup's own text is the value a frame means by "no override", so it
  // has to be captured before any step has had a chance to change it.
  const original = new Map<HTMLElement, string>();

  for (const element of elements.values()) {
    element.setAttribute(STAGED_ATTRIBUTE, "");
    original.set(element, element.textContent ?? "");
  }

  let current = 0;
  applyFrame(elements, original, frames[0]!, { animate: false });

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
      applyFrame(elements, original, frames[next]!, { animate: next > current });
      current = next;
      return next;
    },
    applyPrint() {
      applyFrame(elements, original, printFrame(frames), { animate: false });
      current = frames.length - 1;
    },
  };
}

/**
 * A stage with no slide under it.
 *
 * The presenter view has to drive the deck — a clicker sends its keys to
 * whichever window is focused, and that is the speaker's own screen, not the
 * projector. But the presenter page does not render the slide, so there is
 * nothing for a frame to be applied to.
 *
 * Rather than give the presenter its own idea of what "next" means, it gets a
 * stage that only counts. Every rule about clamping, about the last stop, and
 * about when a step becomes a slide change then lives in
 * [`createNavigator`](./navigate) once, where it is tested — instead of twice,
 * where the two copies would eventually disagree about the end of a slide.
 */
export function createStopCursor(stopCount: number): Stage {
  // A slide always has a resting frame, so a count of zero is a caller that
  // has not counted rather than a slide with nothing on it.
  const stops = Math.max(1, stopCount);
  let current = 0;

  return {
    get stopCount() {
      return stops;
    },
    get index() {
      return current;
    },
    apply(index: number): number {
      current = Math.max(0, Math.min(index, stops - 1));
      return current;
    },
    applyPrint() {
      current = stops - 1;
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
  original: Map<HTMLElement, string>,
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
    setContent(element, original, state.content);
    setProperties(element, state.properties);

    if (animate && state.effect) {
      setEffect(element, state.effect);
    }
  }
}

/**
 * The union of every frame, for print and PDF export.
 *
 * Three different rules, one per kind of state, each matching how that kind is
 * used:
 *
 * - **Visibility accumulates.** Anything that was ever on screen is shown,
 *   because a handout that hides content the audience saw is worse than one
 *   that shows a little more.
 * - **Content replaces.** A changing value has one final answer; stacking the
 *   intermediate ones would be nonsense.
 * - **Properties accumulate.** They are independent switches, so the last
 *   value of each stands.
 *
 * Mirrors `StepTimeline::print_frame` in Rust. The two implementations are
 * checked against each other by the deck fixtures rather than assumed equal.
 */
function printFrame(frames: StepFrame[]): StepFrame {
  const states = new Map<string, ElementState>();

  for (const frame of frames) {
    for (const state of frame.states) {
      const existing = states.get(state.target);

      if (!existing) {
        states.set(state.target, {
          target: state.target,
          visibility: state.visibility,
          ...(state.content === undefined ? {} : { content: state.content }),
          ...(state.properties === undefined ? {} : { properties: { ...state.properties } }),
        });
        continue;
      }

      if (state.visibility === "visible") existing.visibility = "visible";
      if (state.content !== undefined) existing.content = state.content;
      if (state.properties) {
        existing.properties = { ...existing.properties, ...state.properties };
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
