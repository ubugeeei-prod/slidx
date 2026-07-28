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
import { resolveOptions, slideFileName, type SlidxOptions } from "./options";
import { build as buildDeck } from "./pipeline";
import { blockingSummary, formatReport, groupFindings } from "./report";

export type { SlidxOptions } from "./options";

/** A virtual module so a deck-only project needs no entry of its own. */
const ENTRY_ID = "virtual:slidx-entry";
const RESOLVED_ENTRY_ID = `\0${ENTRY_ID}`;

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

    resolveId(id) {
      return id === ENTRY_ID ? RESOLVED_ENTRY_ID : undefined;
    },

    load(id) {
      return id === RESOLVED_ENTRY_ID ? "export default null;" : undefined;
    },

    configureServer(server) {
      watchSlides(server, root, options.srcDir);
      server.middlewares.use(async (request, response, next) => {
        const index = slideIndexFor(request.url ?? "/", options.base);
        if (index === null) return next();

        try {
          const built = await renderDeck(root, options);
          const slide = built.slides[index];

          if (!slide?.html) {
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
          response.end(await server.transformIndexHtml(request.url ?? "/", slide.html));
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

      const built = await renderDeck(root, options);

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
        if (!slide.html) continue;
        this.emitFile({
          type: "asset",
          fileName: slideFileName(options, index),
          source: slide.html,
        });
      }
    },
  };
}

async function renderDeck(root: string, options: ReturnType<typeof resolveOptions>) {
  const { files, source } = await readDeck(
    root,
    options.srcDir,
    options.extensions,
    options.separator,
  );

  const built = await buildDeck(source, {
    theme: options.theme,
    separator: options.separator,
  });

  return { ...built, fileCount: files.length };
}

/**
 * Which slide a URL asks for, or `null` when the URL is not ours.
 *
 * Returning `null` rather than a 404 lets everything else in the project —
 * assets, other plugins, the dev client — keep working alongside a deck.
 */
export function slideIndexFor(url: string, base: string): number | null {
  const path = url.split("?")[0]!.replace(/\/+$/, "");
  const prefix = base ? `/${base}` : "";

  if (!path.startsWith(prefix)) return null;

  const rest = path.slice(prefix.length).replace(/^\//, "");
  if (rest === "" || rest === "index.html") return 0;

  const match = /^(\d+)(?:\/index\.html)?$/.exec(rest);
  if (!match) return null;

  // Slides are one-based in a URL because that is how a person counts them.
  const number = Number(match[1]);
  return number >= 2 ? number - 1 : null;
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
