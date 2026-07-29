// The slidx deck, as it crosses out of Rust.
//
// Generated from the Rust types by `vp run generate:types`. Editing this file
// is pointless: `cargo test -p slidx_wasm` compares it against the types it
// came from, and the types win.

/**
 * What a caller can ask for when building a deck.
 */
export type BuildOptions = {
  /**
   * Theme name. Falls back to the deck's own `theme:`, then the default.
   */
  theme: string | null;
  /**
   * Separator for single-file decks.
   */
  separator: string | null;
  /**
   * Skip rendering and return only the model and diagnostics. The editor
   * uses this while typing, where the outline matters and the HTML does not.
   */
  parseOnly: boolean;
  /**
   * Also render the presenter view for each slide.
   *
   * Off by default: a deck that is only being built for the web does not
   * need it, and it doubles the rendering work.
   */
  presenter: boolean;
  /**
   * Also render the print shell — one document, one page per stop.
   */
  print: boolean;
  /**
   * Also draw a social card per slide, and one for the deck.
   */
  og: boolean;
  /**
   * Module URL the presenter view imports the runtime from.
   */
  runtimeSrc: string | null;
  /**
   * The runtime's source, inlined into the print shell.
   *
   * The print shell is opened over `file://` — by the PDF exporter, from a
   * USB stick, out of an email attachment — and a browser refuses to
   * resolve a module import from a null origin whatever the path says. So
   * that document carries its own script rather than referencing one.
   */
  printRuntime: string | null;
};

/**
 * Everything a build or a preview needs from one call.
 */
export type BuildResult = {
  title: string | null;
  description: string | null;
  slides: Array<BuiltSlide>;
  /**
   * Parse diagnostics and lint findings, in that order.
   */
  diagnostics: Array<Finding>;
  /**
   * True when something in `diagnostics` should stop a build.
   */
  hasBlocking: boolean;
  /**
   * The whole deck as one printable document. Absent unless `print` was set.
   */
  printHtml?: string;
  /**
   * The deck's own social card, as SVG. Absent unless `og` was set.
   */
  ogSvg?: string;
};

/**
 * One built slide.
 */
export type BuiltSlide = {
  id: string;
  index: number;
  title: string | null;
  notes: Array<string>;
  /**
   * Stops on this slide, including the resting frame. Always at least one.
   */
  stopCount: number;
  /**
   * The frontmatter keys the author wrote, whether or not slidx knows them.
   *
   * The editor's inspector shows these, so a key this version has never
   * heard of is still visible rather than quietly lost. The first slide's
   * block is the deck's, which is what the parser already believes.
   *
   * Declared by hand because it is genuinely open: whatever a deck's YAML
   * held. A generated shape would be a promise about keys slidx does not
   * define.
   */
  frontmatter?: Record<string, unknown>;
  /**
   * The complete HTML page. Absent when `parseOnly` was set.
   */
  html?: string;
  /**
   * This slide's social card, as SVG. Absent unless `og` was set.
   */
  ogSvg?: string;
  /**
   * The speaker's view of this slide. Absent unless `presenter` was set.
   */
  presenterHtml?: string;
};

/**
 * A diagnostic, flattened for the JavaScript side.
 */
export type Finding = {
  severity: string;
  code: string;
  message: string;
  help?: string;
  slideIndex?: number;
};

/**
 * Every stop on a slide, in order.
 */
export type StepTimeline = { frames: Array<StepFrame> };

/**
 * A complete description of the slide at one stop.
 */
export type StepFrame = { index: number; states: Array<ElementState> };

/**
 * One element's state within one frame.
 */
export type ElementState = {
  target: string;
  visibility: Visibility;
  /**
   * Text the element shows at this stop, when a step has changed it.
   *
   * `None` means "whatever is in the markup". Carrying the override in the
   * snapshot rather than as a diff is what lets a presenter step backwards
   * through a changing value and see the earlier one again, without the
   * runtime remembering anything.
   */
  content?: string;
  /**
   * Data properties in force at this stop. Accumulated, so a later patch
   * that changes colour does not clear an earlier one that changed weight.
   */
  properties?: { [key in string]: string };
  /**
   * Set only on the frame that triggers the animation, so scrubbing
   * backwards past an entrance does not replay it.
   */
  effect?: Effect;
};

/**
 * A resolved animation attached to one element in one frame.
 */
export type Effect = {
  kind: EffectKind;
  preset: EffectPreset;
  durationMs: number;
  delayMs: number;
  easing: Easing;
  /**
   * Omitted rather than written as `null` when unset. A timeline is
   * serialised into every page of every deck, and most presets have no
   * direction to travel from.
   */
  origin?: Origin;
};

/**
 * Which phase of an element's life an effect belongs to.
 */
export type EffectKind = "entrance" | "emphasis" | "exit";

/**
 * A named animation. Each preset maps to one CSS keyframe set in the runtime.
 */
export type EffectPreset =
  | "none"
  | "fade"
  | "fly-in"
  | "wipe"
  | "zoom"
  | "split"
  | "grow"
  | "float"
  | "typewriter"
  | "draw"
  | "pulse"
  | "shake"
  | "spin"
  | "color-pulse"
  | "underline"
  | "fade-out"
  | "fly-out"
  | "wipe-out"
  | "zoom-out"
  | "shrink";

/**
 * Timing curve for an effect.
 */
export type Easing = "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out" | "spring";

/**
 * Direction an effect travels from or towards.
 */
export type Origin = "left" | "right" | "top" | "bottom" | "center";

/**
 * Whether an element is painted in a given frame.
 */
export type Visibility = "hidden" | "visible";

/**
 * What `buildDeck` accepts.
 *
 * Every field may be left out — the Rust struct is `#[serde(default)]` — or
 * passed explicitly as `undefined`, which means the same thing.
 */
export type BuildDeckOptions = { [K in keyof BuildOptions]?: BuildOptions[K] | undefined };
