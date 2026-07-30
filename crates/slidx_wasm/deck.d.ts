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
   * Absolute URL of the deck's root, overriding the deck's own `url:`.
   *
   * A canonical link, an `og:url` and a sitemap entry are absolute by
   * definition, and a build has no way to know the origin it will be deployed
   * to. So the origin is something someone states: usually `url:` in the
   * frontmatter, which is where authors already write it for the QR codes,
   * and this when the deployment knows better than the file does — a preview
   * build of the same deck is at a different address, and the file cannot say
   * so without being edited per environment.
   *
   * Absent means nothing absolute is emitted at all. A guessed origin sends a
   * search engine to a page that does not exist.
   */
  deckUrl: string | null;
  /**
   * Where the deck is mounted in the site, root-relative. Defaults to `/`.
   *
   * Only `robots.txt` needs it: that file lives at the site root and has to
   * name the deck from there, so it is the one artefact that cannot be
   * written relative to the deck itself.
   */
  deckPath: string | null;
  /**
   * Module URL the presenter view imports the runtime from.
   */
  runtimeSrc: string | null;
  /**
   * Image sizes the caller already read, keyed by the path a slide writes.
   *
   * There is no filesystem on this side of the boundary, so the resolution
   * rules cannot open `./logo.png` themselves. A caller that can — the Vite
   * plugin — reads each header, passes it through [`probe_image`], and hands
   * the answers back here. Absent means those rules stay silent, which is
   * the editor mid-keystroke.
   */
  assets: Array<AssetSize>;
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
 * One image, as the caller measured it.
 */
export type AssetSize = {
  /**
   * As a slide writes it, minus any query or fragment.
   */
  path: string;
  width: number;
  height: number;
  /**
   * True for a format with no resolution to run out of, which is SVG.
   */
  scalable: boolean;
};

/**
 * Everything a build or a preview needs from one call.
 */
export type BuildResult = {
  title: string | null;
  description: string | null;
  /**
   * Length of the speaking slot, from `duration:`.
   *
   * What the per-slide budgets are laid against. Absent for a deck whose
   * author never had a slot, and absent means nothing can be said about
   * whether the talk fits — which is silence rather than a guess.
   */
  durationSeconds?: number;
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
  /**
   * A page per shared code fence, for the caller to write.
   *
   * Composed here and written by whoever asked, because this side of the
   * boundary has no filesystem. Empty when the deck shares nothing, which
   * is most decks.
   */
  snippets: Array<SnippetFile>;
  /**
   * `sitemap.xml` for the deck, for the caller to write beside the slides.
   *
   * Absent when nobody has said where the deck is deployed: `<loc>` is
   * defined as a full URL, so a sitemap without an origin is an invalid file
   * rather than a relative one.
   */
  sitemap?: string;
  /**
   * `robots.txt` for the site the deck is deployed into.
   *
   * Every directive in it is root-relative, so unlike the sitemap it is
   * always something. Absent only when nothing was rendered at all.
   */
  robots?: string;
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
   * This slide's steps as rows and stops, for the editor's timeline.
   *
   * Carried in the same answer as everything else rather than fetched on its
   * own, because a grid drawn from a second call would be a second snapshot
   * and could describe a deck the rest of the payload no longer agrees with.
   */
  steps: StepGrid;
  /**
   * Seconds the author budgeted this slide, from `budget:`.
   *
   * Resolved here rather than left as the text a slide wrote, because
   * `budget:` accepts `90`, `90s`, `1m30s` and `1:30`. A caller that drew a
   * width from the text would be the project's second duration parser, and
   * the one that disagreed with the linter.
   */
  budgetSeconds?: number;
  /**
   * Roughly how long this slide's notes take to say aloud.
   *
   * The only number available before a rehearsal exists for a slide with no
   * budget, which is most slides while a talk is being written. An estimate
   * rather than a measurement, and the same one the linter reasons about.
   */
  estimatedSeconds: number;
  /**
   * Safe to skip when running behind, from `optional:`.
   */
  optional: boolean;
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
 * One shared snippet, as a file waiting to be written.
 */
export type SnippetFile = {
  /**
   * Relative to the deck's own output root, separators already `/`.
   */
  path: string;
  html: string;
};

/**
 * What changed between two versions of a deck.
 */
export type DeckSummary = {
  /**
   * True when there was nothing to compare against — the deck arriving,
   * rather than the deck changing.
   */
  first: boolean;
  /**
   * How many slides the newer of the two decks has.
   */
  slides: number;
  /**
   * The one line, in the deck's own vocabulary.
   *
   * Empty when the deck did not change at all, which is an ordinary commit
   * that touched something else in the repository.
   */
  subject: string;
  /**
   * The rest of the changes, one sentence each.
   *
   * Empty when the subject already said the only thing that happened, so a
   * consumer can render both without repeating one of them.
   */
  changes: Array<string>;
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
 * A slide's steps, as a timeline shows them.
 */
export type StepGrid = {
  rows: Array<StepRow>;
  actions: Array<StepPlacement>;
  /**
   * Stops on this slide, including the resting frame. Always at least one.
   */
  stops: number;
  /**
   * True when the author wrote `steps:`.
   *
   * The one field a timeline must not guess: a generated stop has no line in
   * the file to change, so a cell is editable in place exactly when this is
   * true.
   */
  declared: boolean;
  /**
   * The `autoSteps:` mode in force, whether or not it generated the stops.
   *
   * It stays set after the stops are written out, because it is what puts
   * the anchors in the markup that the written-out steps name.
   */
  auto?: AutoSteps;
};

/**
 * One thing a slide's steps can name.
 */
export type StepRow = {
  /**
   * The selector a step targets.
   */
  target: string;
  /**
   * What to call the row in front of an author.
   */
  label: string;
  /**
   * The mark's `#key`, when the author gave this row a name.
   *
   * Absent for a row staged by `autoSteps:` or a `<!-- step -->` marker,
   * which have no name in the source — the reason those rows cannot be
   * pointed at by hand and the reason a timeline has to show them.
   */
  key?: string;
  /**
   * Whether the row is painted at each stop, one entry per stop.
   *
   * Read straight off the compiled frames, which is the whole reason a
   * timeline over this model is cheap: the state at every stop is already
   * computed, so a caller draws the bar instead of folding the actions
   * forward. Folding them would be a second copy of the rule about what a
   * stop means, and the two would eventually disagree about a `hide`.
   */
  visible: Array<boolean>;
};

/**
 * One authored action, placed on the grid.
 */
export type StepPlacement = {
  /**
   * Position in the slide's action list, which is what an operation names.
   */
  index: number;
  kind: StepKind;
  /**
   * The stop this action lands on.
   */
  stop: number;
  /**
   * Every row it touches, group members included.
   */
  targets: Array<string>;
  /**
   * True when it plays on a timer rather than on a press, and therefore
   * shares the stop before it instead of adding one.
   */
  timed: boolean;
  /**
   * Canonical `steps:` source, without the leading `- `.
   */
  source: string;
};

/**
 * What one authored action does to its targets.
 */
export type StepKind = "reveal" | "hide" | "emphasize" | "set" | "group";

/**
 * Automatic staging derived from slide structure rather than explicit actions.
 */
export type AutoSteps = "list" | "block" | "row";

/**
 * What `buildDeck` accepts.
 *
 * Every field may be left out — the Rust struct is `#[serde(default)]` — or
 * passed explicitly as `undefined`, which means the same thing.
 */
export type BuildDeckOptions = { [K in keyof BuildOptions]?: BuildOptions[K] | undefined };
