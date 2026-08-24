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
 *
 * The demo switch is the one exception to that split — it has to run on the
 * audience slide, because the thing it swaps is what the audience is looking
 * at. It stays cheap by loading only where a demo was declared, and by doing
 * nothing at the moment it is used except write one attribute.
 *
 * The camera looks like a second exception and is not. What ships on the slide
 * is an empty tile; the module that fills it is reached only from
 * `enterPresentation`, so a deck opened from a link never runs a line of it and
 * is never asked for a webcam.
 */

export { ANCHOR_ATTRIBUTE, findAnchors, resolveAnchor } from "./anchor";
export {
  browserCameraEnvironment,
  CAMERA_ATTRIBUTE,
  CAMERA_STATE_ATTRIBUTE,
  startCamera,
} from "./camera";
export type { CameraEnvironment, CameraSession, CameraStatus, MediaStreamLike } from "./camera";
export { createDemoSwitch, DEMO_ATTRIBUTE } from "./demo";
export type { DemoSide, DemoSwitch } from "./demo";
export { loadEffects } from "./effects";
export { createKeymap, DEFAULT_BINDINGS, formatBinding } from "./keymap";
export type { Binding, Command, Keymap, KeymapOptions } from "./keymap";
export { createMediaController, describeLevel, LOUDNESS_TARGET_DB } from "./media";
export type { LevelReport, Levels, LevelStatus, MediaController, MediaElementLike } from "./media";
export { createMirror } from "./mirror";
export { createPairing, createRemoteTransport, pairingUrl, readPairing } from "./remote";
export type { Pairing, PairingOptions, RemoteOptions, RemoteSocket } from "./remote";
export { createNavigator, LAST_STEP } from "./navigate";
export { assessPace, describePace } from "./pace";
export type {
  Pace,
  PaceBasis,
  PaceInput,
  PaceOptions,
  PaceSlide,
  PaceState,
  SkippableSlide,
} from "./pace";
export {
  browserPresentationEnvironment,
  detectPlatform,
  enterPresentation,
  presentationChecklist,
} from "./presentation";
export type {
  ChecklistItem,
  Platform,
  PresentationEnvironment,
  PresentationSession,
} from "./presentation";
export type { Navigator, NavigatorOptions } from "./navigate";
export type { Mirror, MirrorMessage, MirrorTransport, Position } from "./mirror";
export { createStage, createStopCursor, HIDDEN_ATTRIBUTE, STAGED_ATTRIBUTE } from "./stage";
export type { Stage } from "./stage";
export { createTimer, formatDuration } from "./timer";
export type { Timer, TimerState, TimerStatus } from "./timer";
export type {
  DeckData,
  DemoDeclaration,
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

export { JS_ATTRIBUTE, markScriptEnabled } from "./enabled";
