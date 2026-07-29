/**
 * The slidx client runtime.
 *
 * Small on purpose, and split so a page loads only what it uses. Navigation
 * between slides is the browser's job — a built deck is ordinary multi-page
 * HTML, one URL per slide — so an audience slide is responsible for one thing:
 * showing the right *stop* on the slide already loaded. A slide with no steps
 * loads none of it.
 *
 * The presenter view needs more, and is the only thing that pays for it: a
 * clock, and a channel to keep the projector on the same slide.
 */

export { ANCHOR_ATTRIBUTE, findAnchors, resolveAnchor } from "./anchor";
export { createKeymap, DEFAULT_BINDINGS, formatBinding } from "./keymap";
export type { Binding, Command, Keymap, KeymapOptions } from "./keymap";
export { createMediaController, describeLevel, LOUDNESS_TARGET_DB } from "./media";
export type { LevelReport, Levels, LevelStatus, MediaController, MediaElementLike } from "./media";
export { createMirror } from "./mirror";
export { createNavigator, LAST_STEP } from "./navigate";
export { detectPlatform, enterPresentation, presentationChecklist } from "./presentation";
export type {
  ChecklistItem,
  Platform,
  PresentationEnvironment,
  PresentationSession,
} from "./presentation";
export type { Navigator, NavigatorOptions } from "./navigate";
export type { Mirror, MirrorMessage, MirrorTransport, Position } from "./mirror";
export { createStage, HIDDEN_ATTRIBUTE, STAGED_ATTRIBUTE } from "./stage";
export type { Stage } from "./stage";
export { createTimer, formatDuration } from "./timer";
export type { Timer, TimerState, TimerStatus } from "./timer";
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
