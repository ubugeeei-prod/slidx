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

import { readDeck } from "./deck";
import {
  presenterFileName,
  resolveOptions,
  runtimeFileName,
  slideFileName,
  type SlidxOptions,
} from "./options";
import { build as buildDeck } from "./pipeline";
import { blockingSummary, formatReport, groupFindings } from "./report";

export type { SlidxOptions } from "./options";

/** A virtual module so a deck-only project needs no entry of its own. */
const ENTRY_ID = "virtual:slidx-entry";
const RESOLVED_ENTRY_ID = `\0${ENTRY_ID}`;

/** The runtime, resolved as a module rather than served as a file. */
const RESOLVED_RUNTIME_ID = "\0virtual:slidx-runtime";

export function slidx(userOptions: SlidxOptions = {}): Plugin {
  const options = resolveOptions(userOptions);
  let root = process.cwd();

  return {
    name: "slidx",

    configResolved(config) {
      root = config.root;
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

    configureServer(server) {
      watchSlides(server, root, options.srcDir);
      server.middlewares.use(async (request, response, next) => {
        const url = request.url ?? "/";

        const asked = slideRequestFor(url, options.base);
        if (asked === null) return next();

        try {
          const built = await renderDeck(root, options, asked.presenter);
          const slide = built.slides[asked.index];
          const html = asked.presenter ? slide?.presenterHtml : slide?.html;

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

      const built = await renderDeck(root, options, options.presenter);

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

      if (blocking.length > 0 && options.failOnDiagnostics) {
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

      // The runtime is emitted once and shared by every presenter page. It is
      // the only JavaScript a deck ships, and only when the presenter view is
      // built — the audience slides stay at zero.
      if (options.presenter) {
        this.emitFile({
          type: "asset",
          fileName: runtimeFileName(options),
          source: await readRuntime(),
        });
      }
    },
  };
}

async function renderDeck(
  root: string,
  options: ReturnType<typeof resolveOptions>,
  presenter: boolean,
) {
  const { files, source } = await readDeck(
    root,
    options.srcDir,
    options.extensions,
    options.separator,
  );

  const built = await buildDeck(source, {
    theme: options.theme,
    separator: options.separator,
    presenter,
    runtimeSrc: runtimeSrcFor(options),
  });

  return { ...built, fileCount: files.length };
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

/** A slide, and which of its two views was asked for. */
export interface SlideRequest {
  index: number;
  presenter: boolean;
}

/**
 * Which slide a URL asks for, or `null` when the URL is not ours.
 *
 * Returning `null` rather than a 404 lets everything else in the project —
 * assets, other plugins, the dev client — keep working alongside a deck.
 */
export function slideRequestFor(url: string, base: string): SlideRequest | null {
  const path = url.split("?")[0]!.replace(/\/+$/, "");
  const prefix = base ? `/${base}` : "";

  if (!path.startsWith(prefix)) return null;

  let rest = path
    .slice(prefix.length)
    .replace(/^\//, "")
    .replace(/\/index\.html$/, "");
  if (rest === "index.html") rest = "";

  const presenter = rest === "presenter" || rest.endsWith("/presenter");
  if (presenter) rest = rest.replace(/\/?presenter$/, "");

  if (rest === "") return { index: 0, presenter };

  const match = /^(\d+)$/.exec(rest);
  if (!match) return null;

  // Slides are one-based in a URL because that is how a person counts them.
  const number = Number(match[1]);
  return number >= 2 ? { index: number - 1, presenter } : null;
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

/** Sends findings to the terminal and the browser overlay at once. */
function report(
  server: ViteDevServer,
  findings: Parameters<typeof groupFindings>[0],
  slides: { title?: string | undefined }[],
): void {
  if (findings.length === 0) return;

  const titles = slides.map((slide) => slide.title);
  server.config.logger.warn(`\n${formatReport(findings, titles)}`);
}

/**
 * What to say when there is nothing to show.
 *
 * An empty deck is the state every new project starts in, so this is the first
 * thing many people will see. It says what to do next rather than what went
 * wrong.
 */
function emptyDeckMessage(count: number, srcDir: string): string {
  if (count > 0) return "No slide at this number.";

  return (
    `No slides found in ./${srcDir}.\n\n` +
    `Create ./${srcDir}/0001.md and this page will reload:\n\n` +
    "  # My first slide\n\n" +
    "  - a point\n"
  );
}

export default slidx;
