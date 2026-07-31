/**
 * Turning a social card into something a scraper will accept.
 *
 * The card is drawn as SVG, which is small, themeable, and produced by the
 * same tokens as the slides. Almost no social platform renders SVG, so it is
 * converted to PNG at build time by the browser that is already installed for
 * the PDF — and where no browser exists the SVG is still emitted, because a
 * card nobody converts is better than no card at all.
 */

import { renderPdf } from "./pdf";
import { capture } from "./capture";

/** What every scraper crops to. */
export const OG_WIDTH = 1200;
export const OG_HEIGHT = 630;

/**
 * Rasterises an SVG card.
 *
 * Returns `null` rather than throwing when no browser is available: a missing
 * PNG costs a nicer link preview, and failing a build over one would be a
 * wildly disproportionate response.
 */
export async function rasterise(svg: string): Promise<Buffer | null> {
  // Both halves of "there is no browser here", because they fail differently
  // and only one of them was caught. A missing *package* throws on the import;
  // a missing *browser binary* throws on the launch, and that is the ordinary
  // state of a machine where somebody ran `pnpm install` and not `playwright
  // install` — every non-Linux CI runner in this repository, and every
  // contributor's first checkout.
  //
  // Answering `null` either way is what makes the caller's warn-and-continue
  // path reachable. It was written and could not be reached.
  let browser;
  try {
    const { chromium } = await import("playwright");
    browser = await chromium.launch();
  } catch {
    return null;
  }
  try {
    const page = await browser.newPage({
      viewport: { width: OG_WIDTH, height: OG_HEIGHT },
      // Cards are shown at 1200 wide and read at 400. Rendering at 1x keeps
      // the file small; the type is large enough that it survives the crop.
      deviceScaleFactor: 1,
    });

    // A data URL rather than a file: the SVG has no external references, so
    // there is nothing for a base directory to resolve.
    await page.setContent(`<!doctype html><style>html,body{margin:0}</style>${svg}`, {
      waitUntil: "load",
    });

    return await capture(() => page.screenshot({ type: "png" }));
  } finally {
    await browser.close();
  }
}

// The tags that point a scraper at these cards are composed in Rust, beside the
// canonical link and the structured data they have to agree with. A second
// composer here was written before any of that existed and never called.

// Re-exported so the plugin has one import for browser-backed work.
export { renderPdf };
