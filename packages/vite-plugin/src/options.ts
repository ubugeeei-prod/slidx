/**
 * Turning what a user wrote into what the plugin needs.
 *
 * Every option has a default that works, because the shortest path has to be
 * `slidx()` with nothing in it. Anyone who has to read the reference before
 * their first slide renders has already been let down.
 */

/** What `slidx()` accepts. */
export interface SlidxOptions {
  /** Directory holding slide files. Default `"slides"`. */
  srcDir?: string;
  /** Public route the deck is served under. Default `"slides"`. */
  base?: string;
  /** Theme name, overriding the deck's own `theme:`. */
  theme?: string;
  /** Separator for single-file decks. Default `"---"`. */
  separator?: string;
  /** File extensions treated as slides. Default `[".md"]`. */
  extensions?: string[];
  /**
   * Build the speaker's view alongside each slide.
   *
   * On by default: the presenter view is the reason to use a deck tool rather
   * than a PDF, and someone who discovers it exists only after their talk has
   * been failed by the default.
   */
  presenter?: boolean;
  /**
   * Build the print shell — one document, one page per stop.
   *
   * On by default. It is the fallback a speaker reaches for when the venue
   * cannot drive their laptop, and it costs one page nobody has to visit.
   */
  print?: boolean;
  /**
   * Export the deck to PDF at the end of a build.
   *
   * Off by default, and deliberately: it needs Playwright and a browser
   * download, which is a strange price to put in front of every install for
   * something a deck may never need. `pdf: true` turns it on, and the error
   * when Playwright is absent says exactly what to run.
   */
  pdf?: boolean | { fileName?: string };
  /**
   * Draw a social card for each slide, and one for the deck.
   *
   * On by default. A deck is shared as a URL far more often than it is
   * presented, and a URL with no card is a grey rectangle in a timeline.
   *
   * Cards are emitted as SVG always, and converted to PNG when a browser is
   * available — most scrapers still refuse SVG.
   */
  og?: boolean;
  /**
   * Measure the built pages in a browser and report content that is clipped.
   *
   * On by default. Whether a slide fits its box depends on where the lines
   * break, which nothing at build time can work out, so this is the only check
   * that catches a slide with more on it than the frame holds — and the
   * clipping is invisible on the author's screen too, because they are looking
   * at the same clipped slide.
   *
   * It costs one browser launch on a build that is usually launching one
   * anyway for the social cards, and it is skipped without complaint when
   * Playwright is not installed.
   */
  overflow?: boolean;
  /**
   * Fail the build when the linter reports something blocking.
   *
   * On by default: a contrast failure that reaches a projector cannot be fixed
   * from the stage, and a build is the last place it is cheap to catch.
   */
  failOnDiagnostics?: boolean;
}

/** Options with every default filled in. */
export interface ResolvedOptions {
  srcDir: string;
  base: string;
  theme: string | undefined;
  separator: string;
  extensions: string[];
  presenter: boolean;
  print: boolean;
  og: boolean;
  pdf: false | { fileName: string };
  overflow: boolean;
  failOnDiagnostics: boolean;
}

export function resolveOptions(options: SlidxOptions = {}): ResolvedOptions {
  return {
    srcDir: trimSlashes(options.srcDir ?? "slides") || "slides",
    base: normaliseBase(options.base ?? "slides"),
    theme: options.theme,
    separator: options.separator ?? "---",
    extensions: normaliseExtensions(options.extensions ?? [".md"]),
    presenter: options.presenter ?? true,
    print: options.print ?? true,
    og: options.og ?? true,
    pdf: resolvePdf(options.pdf),
    overflow: options.overflow ?? true,
    failOnDiagnostics: options.failOnDiagnostics ?? true,
  };
}

/**
 * A base is stored without slashes and added back where needed.
 *
 * Users write `"slides"`, `"/slides"`, and `"slides/"` and mean the same
 * thing; a plugin that treats them differently produces a 404 that looks like
 * a bug in the deck.
 */
function normaliseBase(base: string): string {
  return trimSlashes(base);
}

function trimSlashes(value: string): string {
  return value.replace(/^\/+/, "").replace(/\/+$/, "");
}

/** Extensions are stored with the dot, however they were written. */
function normaliseExtensions(extensions: string[]): string[] {
  const normalised = extensions
    .map((extension) => extension.trim().toLowerCase())
    .filter(Boolean)
    .map((extension) => (extension.startsWith(".") ? extension : `.${extension}`));

  return normalised.length > 0 ? [...new Set(normalised)] : [".md"];
}

/** The public URL of one slide. */
export function slideRoute(options: ResolvedOptions, index: number): string {
  const path = index === 0 ? "" : `${index + 1}/`;
  return options.base ? `/${options.base}/${path}` : `/${path}`;
}

/** The file a built slide is written to. */
export function slideFileName(options: ResolvedOptions, index: number): string {
  const path = index === 0 ? "index.html" : `${index + 1}/index.html`;
  return options.base ? `${options.base}/${path}` : path;
}

/** The file a slide's presenter view is written to. */
export function presenterFileName(options: ResolvedOptions, index: number): string {
  const path = index === 0 ? "presenter/index.html" : `${index + 1}/presenter/index.html`;
  return options.base ? `${options.base}/${path}` : path;
}

/**
 * PDF export, normalised.
 *
 * `true` is the common case and has to mean something sensible without a file
 * name, so it becomes the deck's default name rather than a required field.
 */
function resolvePdf(pdf: SlidxOptions["pdf"]): false | { fileName: string } {
  if (!pdf) return false;
  if (pdf === true) return { fileName: "deck.pdf" };

  return { fileName: pdf.fileName?.trim() || "deck.pdf" };
}

/** Where a slide's social card is written, without an extension. */
export function ogFileBase(options: ResolvedOptions, index: number | "deck"): string {
  const name = index === "deck" ? "og" : `og-${index + 1}`;
  return options.base ? `${options.base}/${name}` : name;
}

/** Where the printable document is written. */
export function printFileName(options: ResolvedOptions): string {
  return options.base ? `${options.base}/print/index.html` : "print/index.html";
}

/** Where the runtime module is written, and imported from. */
export function runtimeFileName(options: ResolvedOptions): string {
  return options.base ? `${options.base}/runtime.js` : "runtime.js";
}
