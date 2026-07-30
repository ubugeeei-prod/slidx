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
  /**
   * Absolute URL the deck's root is deployed at, overriding its own `url:`.
   *
   * A canonical link, an `og:url` and a sitemap entry are absolute by
   * definition, and a build has no way of knowing the origin it will be served
   * from. So it is stated rather than guessed — usually once, as `url:` in the
   * deck's frontmatter, which is where an author already writes it for the QR
   * codes and the published description. This option is for when the
   * deployment knows better than the file does: a preview build of the same
   * deck is at a different address, and editing the deck per environment is
   * how the two get out of step.
   *
   * Left out, nothing absolute is emitted at all: no canonical, no sitemap.
   * That is deliberate. A guessed origin points a search engine at a page that
   * does not exist, which is worse than the relative links the pages still
   * carry — and it is the rule the QR codes already follow, where no URL means
   * no code rather than a code that scans to nothing.
   */
  deckUrl?: string;
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
  deckUrl: string | undefined;
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
    deckUrl: options.deckUrl?.trim() || undefined,
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

/** What the exported document is called when nobody chooses. */
export const DEFAULT_PDF_FILE_NAME = "deck.pdf";

/**
 * PDF export, normalised.
 *
 * `true` is the common case and has to mean something sensible without a file
 * name, so it becomes the deck's default name rather than a required field.
 */
function resolvePdf(pdf: SlidxOptions["pdf"]): false | { fileName: string } {
  if (!pdf) return false;
  if (pdf === true) return { fileName: DEFAULT_PDF_FILE_NAME };

  return { fileName: pdf.fileName?.trim() || DEFAULT_PDF_FILE_NAME };
}

/**
 * The same options with the PDF on, whatever the project chose.
 *
 * `slidx export --target pdf` is a person asking for the document on a command
 * line. Refusing because the config leaves `pdf` off would be answering a
 * direct question with a setting, and the setting exists to keep a browser
 * download out of every ordinary build rather than to forbid the export.
 */
export function withPdf(options: ResolvedOptions): ResolvedOptions {
  return options.pdf ? options : { ...options, pdf: { fileName: DEFAULT_PDF_FILE_NAME } };
}

/** Where a slide's social card is written, without an extension. */
export function ogFileBase(options: ResolvedOptions, index: number | "deck"): string {
  const name = index === "deck" ? "og" : `og-${index + 1}`;
  return options.base ? `${options.base}/${name}` : name;
}

/**
 * Where one shared snippet's page is written.
 *
 * The path comes from the renderer, which allocates keys across the whole deck
 * because a key is a URL and two files cannot have one name. This only puts it
 * under the deck's base.
 */
export function snippetFileName(options: ResolvedOptions, path: string): string {
  return options.base ? `${options.base}/${path}` : path;
}

/** Where the printable document is written. */
export function printFileName(options: ResolvedOptions): string {
  return options.base ? `${options.base}/print/index.html` : "print/index.html";
}

/**
 * Where the sitemap is written.
 *
 * Beside the slides rather than at the site root, because a sitemap may only
 * list URLs at or below its own directory — and a deck is usually one part of
 * somebody's site rather than the whole of it. `robots.txt` is what points at
 * it from the root.
 */
export function sitemapFileName(options: ResolvedOptions): string {
  return options.base ? `${options.base}/sitemap.xml` : "sitemap.xml";
}

/**
 * Where `robots.txt` is written, which a crawler gives no choice about.
 *
 * `/robots.txt` and nowhere else: a copy inside the deck's directory is a file
 * nothing ever asks for. It is therefore the one thing a deck emits outside its
 * own base, and the one that can collide with a file the project already owns.
 */
export const ROBOTS_FILE_NAME = "robots.txt";

/** Where the runtime module is written, and imported from. */
export function runtimeFileName(options: ResolvedOptions): string {
  return options.base ? `${options.base}/runtime.js` : "runtime.js";
}
