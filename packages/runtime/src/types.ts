/**
 * The deck model, as it crosses from Rust into the browser.
 *
 * The shapes the pipeline produces are re-exported rather than restated. They
 * are generated from the Rust types into `@slidxjs/wasm`, which is a build-time
 * dependency here and nothing more: `export type` erases, so the runtime an
 * audience downloads is not one byte larger for knowing what a frame is, and
 * `vp pack --dts` inlines the declarations into this package's own.
 *
 * What is written out below is the part with no Rust counterpart — the shape
 * the presenter view hands over, which is a projection of the deck rather than
 * a copy of it.
 */

import type { StepTimeline } from "@slidxjs/wasm";

export type {
  Easing,
  Effect,
  EffectKind,
  EffectPreset,
  ElementState,
  Origin,
  StepFrame,
  StepTimeline,
  Visibility,
} from "@slidxjs/wasm";

/**
 * A live demo and the recording that stands in for it.
 *
 * `live` is expected to be remote — that is what live means — and `fallback` is
 * expected not to be, because it has to work on the day the network does not.
 * `slidx_lint` reports a deck that gets either of those backwards.
 */
export interface DemoDeclaration {
  live: string;
  fallback: string | null;
  poster: string | null;
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
  /** Absent on a slide that declares no demo. */
  demo?: DemoDeclaration;
  timeline: StepTimeline;
}

/** The deck, as the runtime needs it. */
export interface DeckData {
  title: string | null;
  durationSeconds: number | null;
  slides: SlideData[];
}
