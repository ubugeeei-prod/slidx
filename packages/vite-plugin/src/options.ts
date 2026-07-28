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

/** Where the printable document is written. */
export function printFileName(options: ResolvedOptions): string {
  return options.base ? `${options.base}/print/index.html` : "print/index.html";
}

/** Where the runtime module is written, and imported from. */
export function runtimeFileName(options: ResolvedOptions): string {
  return options.base ? `${options.base}/runtime.js` : "runtime.js";
}
