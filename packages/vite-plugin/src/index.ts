/**
 * `@slidx/vite-plugin` — decks in a Vite project.
 *
 * The whole install-to-deploy path is meant to be two commands:
 *
 * ```sh
 * vp add -D @slidx/vite-plugin
 * vp build
 * ```
 *
 * so `slidx()` with no options finds `./slides`, serves them in dev, and
 * emits static HTML on build. Anyone who has to read a reference before their
 * first slide renders has already been let down.
 *
 * The built deck is ordinary multi-page HTML: one document per slide, no
 * router, and no client runtime on a slide that has no steps. Navigation is
 * the browser's job, which is why a slide URL can be shared, bookmarked,
 * crawled, and opened on a phone with no JavaScript at all.
 */

import type { Plugin, ViteDevServer } from "vite";

import { readAssetSizes } from "./assets";
import { readDeck } from "./deck";
import { exportPdf, rasteriseCards, reportOverflow } from "./artifacts";
import { frameRequested, renderSlideDocuments, renderStopImages } from "./frames";
import {
  ogFileBase,
  presenterFileName,
  printFileName,
  resolveOptions,
  runtimeFileName,
  ROBOTS_FILE_NAME,
  sitemapFileName,
  slideFileName,
  slideRoute,
  snippetFileName,
  withPdf,
  type SlidxOptions,
} from "./options";
import { build as buildDeck } from "./pipeline";
import { blockingSummary, formatReport, groupFindings } from "./report";
import { emptyDeckMessage, slideRequestFor } from "./routes";
import { EDITOR_PAGE } from "./editor";
import { createEditSession } from "./session";

export type { SlidxOptions } from "./options";
export { slideRequestFor, type SlideRequest } from "./routes";
export { EDITOR_ROUTE_PREFIX } from "./session";
export { EDITOR_PAGE } from "./editor";

/** A virtual module so a deck-only project needs no entry of its own. */
const ENTRY_ID = "virtual:slidx-entry";
const RESOLVED_ENTRY_ID = `\0${ENTRY_ID}`;

/** The runtime, resolved as a module rather than served as a file. */
const RESOLVED_RUNTIME_ID = "\0virtual:slidx-runtime";

export function slidx(userOptions: SlidxOptions = {}): Plugin {
  const options = resolveOptions(userOptions);
  let root = process.cwd();

  // Where the project's own static files come from, so a `robots.txt` it
  // already ships is not overwritten by one this plugin wrote.
  let publicDir: string | false = false;

  // Kept from the bundle so the measurement pass does not read and parse the
  // deck a second time. It runs against files, so it cannot run any earlier.
  let lastBuild: Awaited<ReturnType<typeof renderDeck>> | undefined;

  return {
    name: "slidx",

    configResolved(config) {
      root = config.root;
      publicDir = config.publicDir;
    },

    /**
     * A deck-only project has no `index.html` and no script to bundle.
     * Supplying an entry means `vite build` works in a directory that holds
     * nothing but Markdown, which is the point.
     *
     * The command comes from the hook's own argument rather than from state
     * set in `configResolved` — that hook runs *after* this one, so reading it
     * here would always see the initial value and never supply the entry.
     */
    config(_config, env) {
      if (env.command !== "build") return undefined;
      return { build: { rollupOptions: { input: ENTRY_ID } } };
    },

    /**
     * The runtime is a module Vite resolves, not a file the middleware serves.
     *
     * Serving it from middleware alone works in a browser and fails in Vite's
     * import analysis, which resolves a presenter page's imports against the
     * module graph and rejects what it cannot find. Resolving it here means
     * dev and build agree about what `/slides/runtime.js` is.
     */
    resolveId(id) {
      if (id === ENTRY_ID) return RESOLVED_ENTRY_ID;
      if (id === `/${runtimeFileName(options)}`) return RESOLVED_RUNTIME_ID;
      return undefined;
    },

    async load(id) {
      if (id === RESOLVED_ENTRY_ID) return "export default null;";
      if (id === RESOLVED_RUNTIME_ID) return readRuntime();
      return undefined;
    },

    /**
     * The editing routes exist here and nowhere else.
     *
     * They write to the author's slide files, so a built deck must have no way
     * to reach them. Registering them in `configureServer` alone is that
     * guarantee: `vite build` never calls this hook.
     */
    configureServer(server) {
      watchSlides(server, root, options.srcDir);
      announceEditor(server);

      const session = createEditSession(root, options);

      server.middlewares.use(async (request, response, next) => {
        const url = request.url ?? "/";

        try {
          if (await session.handle(request, response)) return;
        } catch (error) {
          next(error);
          return;
        }

        const asked = slideRequestFor(url, options.base);
        if (asked === null) return next();

        try {
          const built = await renderDeck(root, options, asked.presenter, asked.print ?? false);
          const slide = built.slides[asked.index];
          const html = asked.print
            ? built.printHtml
            : asked.presenter
              ? slide?.presenterHtml
              : slide?.html;

          if (!html) {
            response.statusCode = 404;
            response.setHeader("content-type", "text/plain; charset=utf-8");
            response.end(emptyDeckMessage(built.slides.length, options.srcDir));
            return;
          }

          report(server, built.diagnostics, built.slides);

          response.setHeader("content-type", "text/html; charset=utf-8");
          // A deck is edited constantly; a cached slide is a slide that does
          // not change when its file does.
          response.setHeader("cache-control", "no-store");
          response.end(await server.transformIndexHtml(url, html));
        } catch (error) {
          next(error);
        }
      });
    },

    async generateBundle(_output, bundle) {
      // The virtual entry existed only to give rollup something to start
      // from. Leaving it emits an empty chunk into a deck that otherwise
      // ships no JavaScript at all, which is the property worth protecting.
      for (const [fileName, chunk] of Object.entries(bundle)) {
        if (chunk.type === "chunk" && chunk.facadeModuleId === RESOLVED_ENTRY_ID) {
          delete bundle[fileName];
        }
      }

      const built = await renderDeck(root, options, options.presenter, options.print, options.og);
      lastBuild = built;

      // No slide files at all is a different situation from a deck that
      // failed to parse: emitting a blank page would look like the deck built
      // and is worse than emitting nothing and saying so.
      if (built.fileCount === 0) {
        this.warn(emptyDeckMessage(0, options.srcDir));
        return;
      }

      const { blocking } = groupFindings(built.diagnostics);

      if (built.diagnostics.length > 0) {
        const titles = built.slides.map((slide) => slide.title);
        this.warn(`\n${formatReport(built.diagnostics, titles)}`);
      }

      // The decision comes from Rust and the count comes from the grouping.
      // Both were computed here before, which made "what blocks a build" a rule
      // written twice — once in the linter and once in a filter over its output.
      // `report.test.ts` pins that the two still agree.
      if (built.hasBlocking && options.failOnDiagnostics) {
        this.error(blockingSummary(blocking.length));
      }

      for (const [index, slide] of built.slides.entries()) {
        if (slide.html) {
          this.emitFile({
            type: "asset",
            fileName: slideFileName(options, index),
            source: slide.html,
          });
        }

        if (slide.presenterHtml) {
          this.emitFile({
            type: "asset",
            fileName: presenterFileName(options, index),
            source: slide.presenterHtml,
          });
        }
      }

      // Cards are emitted as SVG unconditionally and rasterised below, so a
      // build with no browser still produces something a platform can be
      // pointed at.
      if (options.og) {
        for (const [index, slide] of built.slides.entries()) {
          if (!slide.ogSvg) continue;
          this.emitFile({
            type: "asset",
            fileName: `${ogFileBase(options, index)}.svg`,
            source: slide.ogSvg,
          });
        }

        if (built.ogSvg) {
          this.emitFile({
            type: "asset",
            fileName: `${ogFileBase(options, "deck")}.svg`,
            source: built.ogSvg,
          });
        }
      }

      // A slide that shares a fence already draws a QR pointing at its page,
      // so an unwritten page is a code on a projector that resolves to
      // nothing. Empty for the decks that share nothing, which is most.
      for (const snippet of built.snippets) {
        this.emitFile({
          type: "asset",
          fileName: snippetFileName(options, snippet.path),
          source: snippet.html,
        });
      }

      if (built.printHtml) {
        this.emitFile({
          type: "asset",
          fileName: printFileName(options),
          source: built.printHtml,
        });
      }

      // The runtime is emitted once and shared by the presenter and print
      // pages. It is the only JavaScript a deck ships — the audience slides
      // stay at zero.
      if (options.presenter || options.print) {
        this.emitFile({
          type: "asset",
          fileName: runtimeFileName(options),
          source: await readRuntime(),
        });
      }

      // The list of pages, written by the thing that emitted them. Absent for a
      // deck nobody has given an address, because a `<loc>` is a full URL and a
      // relative one makes the file invalid rather than lenient.
      if (built.sitemap) {
        this.emitFile({
          type: "asset",
          fileName: sitemapFileName(options),
          source: built.sitemap,
        });
      }

      // `robots.txt` is the one file a deck writes outside its own base, and
      // therefore the one that can belong to somebody else. A project that
      // already has one keeps it: it is the whole site's file, this plugin owns
      // a directory of it, and silently replacing it could open a site up as
      // easily as close it down. The `noindex` on every page is what still
      // holds in that case, which is why a draft deck says it both ways.
      if (built.robots && !(await hasOwnRobots(publicDir))) {
        this.emitFile({
          type: "asset",
          fileName: ROBOTS_FILE_NAME,
          source: built.robots,
        });
      }
    },

    /**
     * The PDF is made after the files exist, not instead of them.
     *
     * `writeBundle` rather than `generateBundle`: the exporter opens the
     * emitted print shell over `file://`, so it has to be on disk. Printing
     * the artifact is also what guarantees the PDF matches what a person gets
     * by pressing Cmd-P on the same page.
     *
     * The frames are the same idea one step further out. `slidx export` starts
     * this build and asks it for what that export needs — see `frames.ts` —
     * because the browser is here and the print shell is here, and rendering
     * them anywhere else would be a second answer to what a slide looks like.
     */
    async writeBundle(output) {
      const directory = output.dir ?? "dist";
      const frame = frameRequested();

      if (options.og) await rasteriseCards(this, directory, options);
      await exportPdf(this, directory, frame === "pdf" ? withPdf(options) : options);

      if (frame === "pdf-slides") await renderSlideDocuments(this, directory, options);
      if (frame === "png") await renderStopImages(this, directory, options);

      // An empty deck emitted no pages, so there is nothing to open.
      if (lastBuild && lastBuild.fileCount > 0) {
        const titles = lastBuild.slides.map((slide) => slide.title);
        await reportOverflow(this, directory, options, lastBuild.source, titles);
      }
    },
  };
}

async function renderDeck(
  root: string,
  options: ReturnType<typeof resolveOptions>,
  presenter: boolean,
  print = false,
  og = false,
) {
  const { files, source } = await readDeck(
    root,
    options.srcDir,
    options.extensions,
    options.separator,
  );

  // Read here because the rules cannot: they run inside WebAssembly, which
  // has no filesystem. Cheap — the head of each image, and nothing at all for
  // a deck with no images, which is most of the ones being edited right now.
  const assets = await readAssetSizes(root, options.srcDir);

  const built = await buildDeck(source, {
    theme: options.theme,
    separator: options.separator,
    assets,
    presenter,
    print,
    og,
    deckUrl: options.deckUrl,
    // Where the deck sits in the site, which only `robots.txt` needs: it lives
    // at the site root and has to name the deck from there. Everything else a
    // page says is either relative to that page or absolute from the deck's own
    // URL.
    deckPath: slideRoute(options, 0),
    runtimeSrc: runtimeSrcFor(options),
    // The print shell carries the runtime rather than importing it, so the
    // one document a speaker falls back to opens from anywhere.
    printRuntime: print ? await readRuntime() : undefined,
  });

  return { ...built, source, fileCount: files.length };
}

/**
 * Where a presenter page imports the runtime from.
 *
 * Absolute, because a presenter page can be one or two directories deep
 * depending on the slide, and a relative path would have to differ per page.
 */
function runtimeSrcFor(options: ReturnType<typeof resolveOptions>): string {
  return `/${runtimeFileName(options)}`;
}

/** Whether the project already ships a `robots.txt` of its own. */
async function hasOwnRobots(publicDir: string | false): Promise<boolean> {
  if (publicDir === false) return false;

  const { access } = await import("node:fs/promises");
  const { join } = await import("node:path");

  return access(join(publicDir, ROBOTS_FILE_NAME)).then(
    () => true,
    () => false,
  );
}

/** The built runtime, read once. */
let runtime: Promise<string> | undefined;

function readRuntime(): Promise<string> {
  runtime ??= (async () => {
    const { createRequire } = await import("node:module");
    const { readFile } = await import("node:fs/promises");
    const require = createRequire(import.meta.url);

    return readFile(require.resolve("@slidx/runtime"), "utf8");
  })();

  return runtime;
}

/**
 * Watches the slide directory.
 *
 * A full reload rather than HMR: a slide is a whole document, so there is no
 * module boundary to swap. Reloading is also what proves the built page works,
 * since it is the same path the browser takes in production.
 */
function watchSlides(server: ViteDevServer, root: string, srcDir: string): void {
  const directory = `${root}/${srcDir}`;
  server.watcher.add(directory);

  const reload = (path: string) => {
    if (!path.startsWith(directory)) return;
    server.ws.send({ type: "full-reload", path: "*" });
  };

  server.watcher.on("add", reload);
  server.watcher.on("change", reload);
  server.watcher.on("unlink", reload);
}

/**
 * Says where the editor is, once, next to the URLs Vite prints.
 *
 * An editor nobody knows is running is an editor nobody uses. It costs one
 * line, and it goes through Vite's own URL printing so it appears with the
 * others rather than scrolling past before the server is up.
 */
function announceEditor(server: ViteDevServer): void {
  const printUrls = server.printUrls.bind(server);

  server.printUrls = () => {
    printUrls();
    const local = server.resolvedUrls?.local[0];
    if (local) server.config.logger.info(`  ➜  Editor:  ${local.replace(/\/$/, "")}${EDITOR_PAGE}`);
  };
}

/** Sends findings to the terminal and the browser overlay at once. */
function report(
  server: ViteDevServer,
  findings: Parameters<typeof groupFindings>[0],
  slides: readonly { title: string | null }[],
): void {
  if (findings.length === 0) return;

  const titles = slides.map((slide) => slide.title);
  server.config.logger.warn(`\n${formatReport(findings, titles)}`);
}

export default slidx;
