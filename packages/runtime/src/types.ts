/**
 * The deck model, as it crosses from Rust into the browser.
 *
 * These types mirror `slidx_core` exactly, field for field. They are written
 * by hand here only until the N-API type generation lands; after that this
 * file is generated and drift becomes a build failure rather than a bug.
 */

/** Whether an element is painted at a given stop. */
export type Visibility = "hidden" | "visible";

/** Which phase of an element's life an effect belongs to. */
export type EffectKind = "entrance" | "emphasis" | "exit";

/**
 * Every named animation, in the same order as `EffectPreset` in Rust.
 *
 * A runtime value rather than a bare type, so the CSS can be checked against
 * it — a preset the compiler can emit but the stylesheet has no rule for
 * would otherwise fail silently, on stage, as an element that never appears.
 */
export const EFFECT_PRESETS = [
  "none",
  "fade",
  "fly-in",
  "wipe",
  "zoom",
  "split",
  "grow",
  "float",
  "typewriter",
  "draw",
  "pulse",
  "shake",
  "spin",
  "color-pulse",
  "underline",
  "fade-out",
  "fly-out",
  "wipe-out",
  "zoom-out",
  "shrink",
] as const;

/** A named animation. One CSS keyframe set each. */
export type EffectPreset = (typeof EFFECT_PRESETS)[number];

export type Easing = "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out" | "spring";

export type Origin = "left" | "right" | "top" | "bottom" | "center";

/** A resolved animation, attached to one element on one frame. */
export interface Effect {
  kind: EffectKind;
  preset: EffectPreset;
  durationMs: number;
  delayMs: number;
  easing: Easing;
  origin?: Origin;
}

/**
 * One element's state within one frame.
 *
 * `effect` is present only on the frame that triggers it, which is what stops
 * an entrance from replaying every time the presenter steps past it.
 *
 * `content` and `properties` are how a step changes something already on
 * screen. Both are absolute rather than incremental — `content: undefined`
 * means "whatever the markup says", not "unchanged" — which is what lets the
 * runtime step backwards without remembering anything.
 */
export interface ElementState {
  target: string;
  visibility: Visibility;
  /** Text the element shows at this stop, overriding the markup. */
  content?: string;
  /** Data properties in force at this stop, as `data-slidx-<name>`. */
  properties?: Record<string, string>;
  effect?: Effect;
}

/** A complete description of the slide at one stop. */
export interface StepFrame {
  index: number;
  states: ElementState[];
}

/**
 * Every stop on a slide, in order.
 *
 * Snapshots, not deltas. Never empty: a slide with no steps is one frame.
 */
export interface StepTimeline {
  frames: StepFrame[];
}

/** One slide, as the runtime needs it. */
export interface SlideData {
  id: string;
  index: number;
  title: string | null;
  notes: string[];
  transition: string | null;
  budgetSeconds: number | null;
  optional: boolean;
  timeline: StepTimeline;
}

/** The deck, as the runtime needs it. */
export interface DeckData {
  title: string | null;
  durationSeconds: number | null;
  slides: SlideData[];
}
