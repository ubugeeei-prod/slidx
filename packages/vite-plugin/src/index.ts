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
import { readDeck, type DeckSource } from "./deck";
import { exportPdf, rasteriseCards, reportOverflow } from "./artifacts";
import { emitCrawlerFiles } from "./crawler";
import { frameRequested, renderSlideDocuments, renderStopImages } from "./frames";
import {
  ogFileBase,
  effectsFileName,
  presenterFileName,
  printFileName,
  rehearsalFileName,
  resolveOptions,
  runtimeFileName,
  slideFileName,
  slideRoute,
  snippetFileName,
  withPdf,
  type SlidxOptions,
} from "./options";
import { build as buildDeck } from "./pipeline";
import { blockingSummary, formatReport, groupFindings } from "./report";
import { emptyDeckMessage, slideRequestFor } from "./routes";
import { readThemePackages } from "./themes";
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
/** Rehearsal is a separate module so audience slides never pay for it. */
const RESOLVED_REHEARSAL_ID = "\0virtual:slidx-rehearsal";

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
      if (id === `/${rehearsalFileName(options)}`) return RESOLVED_REHEARSAL_ID;
      return undefined;
    },

    async load(id) {
      if (id === RESOLVED_ENTRY_ID) return "export default null;";
      if (id === RESOLVED_RUNTIME_ID) return readRuntime();
      if (id === RESOLVED_REHEARSAL_ID) return readRehearsal();
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
      const session = createEditSession(root, options);

      // The session is told before the browser is: a slide the author saved in
      // their own text editor has to reach the shared document, or the next
      // operation from a co-presenter would be planned against bytes that are
      // no longer on disk.
      watchSlides(server, root, options.srcDir, () => session.refresh());
      announceEditor(server);
      server.httpServer?.on("close", () => session.close());

      server.middlewares.use(async (request, response, next) => {
        const url = request.url ?? "/";

        try {
          if (await session.handle(request, response)) return;
        } catch (error) {
          next(error);
          return;
        }

        if (url.split("?")[0] === `/${effectsFileName(options)}`) {
          response.statusCode = 200;
          response.setHeader("content-type", "text/css; charset=utf-8");
          response.setHeader("cache-control", "no-store");
          response.end(await readEffects());
          return;
        }

        const asked = slideRequestFor(url, options.base);
        if (asked === null) return next();

        try {
          // A `?rev=` asks for the deck as a commit had it. Only the *source*
          // changes: the same render, the same shell, the same theme, the same
          // WebAssembly module, so the page for an old commit is the real page
          // rather than a picture of one. A second rendering path here would
          // be a second answer about layout.
          const rev = revisionAsked(url);
          // `null` only ever comes from a revision that was asked for and not
          // found, so the narrowing is the guard: no revision is `undefined`,
          // which reads the working copy.
          const past = rev ? await session.deckAt(rev) : undefined;

          if (past === null) {
            response.statusCode = 404;
            response.setHeader("content-type", "text/plain; charset=utf-8");
            // Never a quiet fall back to the working copy: a page that looked
            // like history and was not would be the worst possible answer.
            response.end("No such revision in this repository.");
            return;
          }

          const built = await renderDeck(
            root,
            options,
            asked.presenter,
            asked.print ?? false,
            false,
            past,
          );
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

          // Findings are about the deck the author is working on. Reporting
          // them for a version they are only looking at would fill a terminal
          // with complaints about a slide they already fixed.
          if (!past) report(server, built.diagnostics, built.slides);

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
      // pages. Audience slides still stay at zero JavaScript unless they have
      // staged content.
      if (options.presenter || options.print) {
        this.emitFile({
          type: "asset",
          fileName: runtimeFileName(options),
          source: await readRuntime(),
        });
      }

      // Rehearsal recording is presenter-only and large enough to keep out of
      // the shared runtime. A print-only build and every audience page pay
      // nothing for it.
      if (options.presenter) {
        this.emitFile({
          type: "asset",
          fileName: rehearsalFileName(options),
          source: await readRehearsal(),
        });
      }

      // Loaded from the runtime only on a staged audience slide. One cacheable
      // file for the whole deck, and no bytes at all for the common one-stop
      // slide — inlining it into every page would charge seven kilobytes per
      // slide for an effect most slides never run.
      if (built.slides.some((slide) => slide.stopCount >= 2)) {
        this.emitFile({
          type: "asset",
          fileName: effectsFileName(options),
          source: await readEffects(),
        });
      }

      // The sitemap and the robots file, which are the deck describing itself
      // to something that is not a person.
      await emitCrawlerFiles(this, built, options, publicDir);
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
  /**
   * The deck as some commit had it, when a preview asked for one.
   *
   * The only thing a revision changes. Everything below this line is the path
   * a build and the working copy already take, which is what makes the page
   * for an old commit the page that commit would have produced.
   */
  past?: DeckSource,
) {
  const { files, source } =
    past ?? (await readDeck(root, options.srcDir, options.extensions, options.separator));

  // Read here because the rules cannot: they run inside WebAssembly, which
  // has no filesystem. Cheap — the head of each image, and nothing at all for
  // a deck with no images, which is most of the ones being edited right now.
  const assets = await readAssetSizes(root, options.srcDir);

  // And the same reason again, for the same boundary: the token documents of
  // whatever theme packages the project depends on. Installing one is the
  // whole configuration — nothing here is imported or registered.
  const themePackages = await readThemePackages(root);

  const built = await buildDeck(source, {
    theme: options.theme,
    themePackages,
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
    rehearsalSrc: rehearsalSrcFor(options),
    // The print shell carries the runtime rather than importing it, so the
    // one document a speaker falls back to opens from anywhere.
    printRuntime: print ? await readRuntime() : undefined,
  });

  return { ...built, source, fileCount: files.length };
}

/**
 * The commit a slide URL asks to be shown as of, if any.
 *
 * `rev` rather than `at`, because the editor's canvas already puts a timestamp
 * in `at` to defeat caching — and a cache-buster read as a revision would send
 * a clock reading to git.
 */
function revisionAsked(url: string): string | undefined {
  const query = url.indexOf("?");
  if (query === -1) return undefined;

  return new URLSearchParams(url.slice(query + 1)).get("rev") ?? undefined;
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

/** The presenter-only module sits beside the shared runtime. */
function rehearsalSrcFor(options: ReturnType<typeof resolveOptions>): string {
  return `/${rehearsalFileName(options)}`;
}

/** The built runtime, read once. */
let runtime: Promise<string> | undefined;
let rehearsal: Promise<string> | undefined;
let effects: Promise<string> | undefined;

function readRuntime(): Promise<string> {
  runtime ??= (async () => {
    const { createRequire } = await import("node:module");
    const { readFile } = await import("node:fs/promises");
    const require = createRequire(import.meta.url);

    return readFile(require.resolve("@slidx/runtime"), "utf8");
  })();

  return runtime;
}

/** The built presenter-only rehearsal runtime, read once. */
function readRehearsal(): Promise<string> {
  rehearsal ??= (async () => {
    const { createRequire } = await import("node:module");
    const { readFile } = await import("node:fs/promises");
    const require = createRequire(import.meta.url);

    return readFile(require.resolve("@slidx/rehearsal"), "utf8");
  })();

  return rehearsal;
}

/** The stylesheet the runtime resolves beside itself, read once. */
function readEffects(): Promise<string> {
  effects ??= (async () => {
    const { createRequire } = await import("node:module");
    const { readFile } = await import("node:fs/promises");
    const require = createRequire(import.meta.url);

    return readFile(require.resolve("@slidx/runtime/effects.css"), "utf8");
  })();

  return effects;
}

/**
 * Watches the slide directory.
 *
 * A full reload rather than HMR: a slide is a whole document, so there is no
 * module boundary to swap. Reloading is also what proves the built page works,
 * since it is the same path the browser takes in production.
 */
function watchSlides(
  server: ViteDevServer,
  root: string,
  srcDir: string,
  changed: () => void | Promise<void>,
): void {
  const directory = `${root}/${srcDir}`;
  server.watcher.add(directory);

  const reload = async (path: string) => {
    if (!path.startsWith(directory)) return;

    try {
      // Awaited, because the reload is what makes every browser ask for the
      // deck again. Telling them first would race the read this is waiting for,
      // and the answer they got back would be the bytes from before the save.
      await changed();
    } catch {
      // A save caught half-written reads as a failure here, and the reload
      // below is still the right thing to do: it sends every browser back to
      // the deck route, which reads the files itself and has its own answer
      // for a deck it cannot read. Swallowed rather than thrown, because a
      // watcher callback has nobody to throw to but the process.
    }

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
