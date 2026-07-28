/**
 * The slidx client runtime.
 *
 * Small on purpose. Navigation between slides is the browser's job — a built
 * deck is ordinary multi-page HTML, one URL per slide — so this runtime is
 * responsible for one thing: showing the right *stop* on the slide that is
 * already loaded.
 *
 * A slide with no steps loads none of this.
 */

export { ANCHOR_ATTRIBUTE, findAnchors, resolveAnchor } from "./anchor";
export { createStage, HIDDEN_ATTRIBUTE, STAGED_ATTRIBUTE } from "./stage";
export type { Stage } from "./stage";
export type {
  DeckData,
  Easing,
  Effect,
  EffectKind,
  EffectPreset,
  ElementState,
  Origin,
  SlideData,
  StepFrame,
  StepTimeline,
  Visibility,
} from "./types";

/**
 * Attribute set on `<html>` as soon as the runtime loads.
 *
 * The staging CSS is gated on it, so a deck whose script never arrives — a
 * venue with no network, a blocked bundle — shows every element rather than a
 * slide that is mostly invisible.
 */
export const JS_ATTRIBUTE = "data-slidx-js";

/** Marks the document as script-enabled so staging CSS takes effect. */
export function markScriptEnabled(document: Document): void {
  document.documentElement.setAttribute(JS_ATTRIBUTE, "");
}
