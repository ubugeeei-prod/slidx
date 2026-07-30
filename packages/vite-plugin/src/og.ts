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
  let chromium;
  try {
    ({ chromium } = await import("playwright"));
  } catch {
    return null;
  }

  const browser = await chromium.launch();
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

/** The tags a scraper reads, for one page. */
export function metaTags(url: string | undefined, title: string, cardPath: string): string {
  const absolute = url ? new URL(cardPath, url).href : cardPath;

  return [
    `<meta property="og:title" content="${escapeAttribute(title)}">`,
    `<meta property="og:image" content="${escapeAttribute(absolute)}">`,
    `<meta property="og:image:width" content="${OG_WIDTH}">`,
    `<meta property="og:image:height" content="${OG_HEIGHT}">`,
    // Without this, X shows a small square crop and the title is unreadable.
    `<meta name="twitter:card" content="summary_large_image">`,
  ].join("\n");
}

function escapeAttribute(value: string): string {
  return value.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("<", "&lt;");
}

// Re-exported so the plugin has one import for browser-backed work.
export { renderPdf };
