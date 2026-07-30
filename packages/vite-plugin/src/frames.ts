/**
 * The per-slide documents and per-stop images an export asked for.
 *
 * `slidx export --target png` has to come from somewhere, and the only place it
 * can honestly come from is here: the browser is already installed for the PDF,
 * the print shell is already emitted with a page per stop, and rendering these
 * anywhere else would be a second renderer producing a deck that could differ
 * from the one on the projector.
 *
 * ## Why an environment variable rather than an option
 *
 * A build writes these only when it is asked, because each is a browser pass
 * over every stop in the deck — seconds, for something almost every build does
 * not want. And the thing that wants them is not the author's config, it is one
 * command line: `slidx export` sets {@link FRAME_VARIABLE} on the build it
 * starts. An option would invite somebody to turn forty screenshots on
 * permanently and pay for them on every save.
 *
 * The Rust side of the same contract is `crates/slidx_cli/src/export/build.rs`,
 * and the directory names are `slidx_export`'s `Frame::directory`.
 *
 * ## Where they land, and why not in the deck
 *
 * Under `export/` at the top of the output, never under the deck's own base.
 * They are staging for one command, not pages of the site, and a static host
 * serving forty screenshots of a talk nobody asked it to serve is a worse
 * default than an extra directory. `slidx export --target browser` leaves them
 * out of the site archive for the same reason.
 *
 * ## One stop, one image
 *
 * The stop is the unit, which is the answer the print shell already gave: a
 * handout that collapses an eight-step build into one slide shows the punchline
 * without the setup. A slide that builds in four steps is four images, and the
 * per-slide documents keep that slide's stops as their pages.
 */

import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import type { Page } from "playwright";

import type { ResolvedOptions } from "./options";
import { printFileName } from "./options";
import { READY_ATTRIBUTE } from "./pdf";
import type { Reporter } from "./artifacts";
import { capture } from "./capture";

/** What `slidx export` sets to say which frames the build has to render. */
export const FRAME_VARIABLE = "SLIDX_EXPORT";

/** The frames an export can ask for. Spelled as `slidx_export`'s `Frame`. */
export type Frame = "pdf" | "pdf-slides" | "png";

const FRAMES: readonly Frame[] = ["pdf", "pdf-slides", "png"];

/** Where frames go, relative to the build's output directory. */
export const FRAME_DIRECTORY = "export";

/**
 * How wide the pages are laid out before an image is taken.
 *
 * The same width the overflow check measures at, so an exported image is the
 * layout that was checked rather than a second one nobody looked at. It is set
 * on the container rather than left to the viewport because a scrollbar reserves
 * layout space on some platforms and not others, and an export whose image size
 * depended on that would not be reproducible.
 */
const LAYOUT_WIDTH = 1600;

/** Which frames this build was asked for, if any. */
export function frameRequested(env: NodeJS.ProcessEnv = process.env): Frame | null {
  const asked = env[FRAME_VARIABLE]?.trim();

  return FRAMES.find((frame) => frame === asked) ?? null;
}

/**
 * One PDF per slide, each holding that slide's stops as its pages.
 *
 * The thing a single document cannot be. Conferences and review forms ask for
 * one file per slide, and splitting the deck's own PDF afterwards would need a
 * PDF parser to do badly what the browser that printed it does exactly.
 */
export async function renderSlideDocuments(
  context: Reporter,
  directory: string,
  options: ResolvedOptions,
): Promise<void> {
  await render(context, directory, options, "pdf-slides", async (page, pages) => {
    const written: Frames = [];
    const total = counts(pages).size;

    for (const [slide, [first, last]] of spans(pages)) {
      written.push({
        name: `slide-${padded(slide + 1, total)}.pdf`,
        bytes: await page.pdf({
          // A range rather than a document per navigation: the shell is already
          // open and staged, and re-opening it per slide would be one browser
          // launch per slide on a deck with forty.
          pageRanges: `${first}-${last}`,
          // The same settings the whole-deck PDF uses, so a slide pulled out on
          // its own is the page it was in the deck.
          preferCSSPageSize: true,
          printBackground: true,
          margin: { top: "0", right: "0", bottom: "0", left: "0" },
        }),
      });
    }

    return written;
  });
}

/** One image per stop. */
export async function renderStopImages(
  context: Reporter,
  directory: string,
  options: ResolvedOptions,
): Promise<void> {
  await render(context, directory, options, "png", async (page, pages) => {
    const written: Frames = [];
    const stops = new Map<number, number>();
    const total = counts(pages);
    const widest = Math.max(...total.values());

    // Laid out at a fixed width rather than the viewport's, so two machines
    // produce the same image whether or not their scrollbars take space.
    await page.addStyleTag({ content: `.slidx-print { width: ${LAYOUT_WIDTH}px; }` });

    const elements = await page.$$(".slidx-page");

    for (const [index, element] of elements.entries()) {
      const slide = pages[index] ?? 0;
      const stop = (stops.get(slide) ?? 0) + 1;
      stops.set(slide, stop);

      written.push({
        name: `slide-${padded(slide + 1, total.size)}-stop-${padded(stop, widest)}.png`,
        // The element rather than the viewport: the page box *is* the slide, so
        // there is nothing to crop and no margin to guess at.
        bytes: await capture(() => element.screenshot({ type: "png" })),
      });
    }

    return written;
  });
}

/** One rendered frame, and the name it is written under. */
type Frames = { name: string; bytes: Buffer }[];

/**
 * Opens the emitted print shell and writes whatever the caller renders from it.
 *
 * A missing browser is reported and nothing is written. It is not a build
 * failure: the deck built, and the export is what has nothing to package —
 * which `slidx export` says, naming the same install line. Failing the build
 * here would take the deck away too, over an artefact for somewhere else.
 */
async function render(
  context: Reporter,
  directory: string,
  options: ResolvedOptions,
  frame: Frame,
  produce: (page: Page, pages: number[]) => Promise<Frames>,
): Promise<void> {
  if (!options.print) {
    context.warn(
      `No ${frame} frames: the print shell is what they are rendered from, and ` +
        "`print: false` turns it off.",
    );
    return;
  }

  let chromium;
  try {
    ({ chromium } = await import("playwright"));
  } catch {
    context.warn(
      `No ${frame} frames: rendering them needs a browser.\n` +
        "  vp add -D playwright && vp exec playwright install chromium",
    );
    return;
  }

  const browser = await chromium.launch();

  try {
    const page = await browser.newPage({
      viewport: { width: LAYOUT_WIDTH, height: 1200 },
      // One image pixel per CSS pixel. Two would quadruple a forty-stop deck's
      // archive for detail nothing downstream renders at.
      deviceScaleFactor: 1,
    });

    // `file://`, for the reason the PDF exporter uses it: the shell inlines
    // everything, so what is rendered is the artefact a person opens.
    await page.goto(pathToFileURL(join(directory, printFileName(options))).href, {
      waitUntil: "load",
    });

    // The pages do not exist until the shell has cloned them. Rendering early
    // gives one frame per slide and no build at all — the exact failure the
    // per-stop unit exists to prevent.
    await page.waitForSelector(`html[${READY_ATTRIBUTE}]`, { timeout: 60_000 });

    const pages = await page.$$eval(".slidx-page", (found) =>
      found.map((element) => Number((element as HTMLElement).dataset["slidxSlide"] ?? 0)),
    );

    const frames = await produce(page, pages);
    const out = join(directory, FRAME_DIRECTORY, frame === "png" ? "png" : "pdf");
    await mkdir(out, { recursive: true });

    for (const { name, bytes } of frames) {
      await writeFile(join(out, name), bytes);
    }

    context.info(`rendered ${frames.length} ${frame} frame(s) for export`);
  } catch (error) {
    context.warn(`No ${frame} frames — the deck built anyway.\n${(error as Error).message}`);
  } finally {
    await browser.close();
  }
}

/**
 * Where each slide's stops start and end, as 1-based printed page numbers.
 *
 * A slide's stops are contiguous in the shell — it clones each page in place —
 * so first and last is the whole range and no list of pages is needed.
 */
function spans(pages: number[]): Map<number, [number, number]> {
  const found = new Map<number, [number, number]>();

  pages.forEach((slide, index) => {
    const at = index + 1;
    const seen = found.get(slide);
    found.set(slide, seen ? [seen[0], at] : [at, at]);
  });

  return found;
}

/** How many stops each slide expanded into. */
function counts(pages: number[]): Map<number, number> {
  const found = new Map<number, number>();

  for (const slide of pages) found.set(slide, (found.get(slide) ?? 0) + 1);

  return found;
}

/**
 * A number padded so the files sort the way the deck reads.
 *
 * Two digits at least, and as many as the largest needs: `slide-100` sorts
 * before `slide-99` in every file browser and every archive listing, and a
 * handout in that order is one somebody has to reorder by hand.
 */
function padded(value: number, of: number): string {
  return String(value).padStart(Math.max(2, String(of).length), "0");
}
