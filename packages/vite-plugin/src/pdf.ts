/**
 * Printing the built deck to a PDF.
 *
 * The PDF is produced from the *emitted* print shell, opened over `file://`,
 * rather than from a second render. Everything a slide needs is inlined, so
 * the file opens with no server — and printing the artifact means the PDF
 * cannot differ from what a person gets by pressing Cmd-P on the same page.
 *
 * Playwright is an optional peer dependency. Making it required would put a
 * browser download in front of every `install`, which is a strange price for
 * a deck that may never be exported. When it is missing, the message says
 * exactly what to run.
 */

import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

/** Set by the print shell once every stop has been expanded into a page. */
const READY_ATTRIBUTE = "data-slidx-print-ready";

export interface PdfOptions {
  /** Page width, as CSS. Defaults to what the shell's `@page` declares. */
  width?: string;
  height?: string;
  /** How long to wait for the expansion. A long deck on a cold browser. */
  timeoutMs?: number;
}

/**
 * Renders a print shell to PDF bytes.
 *
 * Throws with an actionable message rather than a stack trace when Playwright
 * is absent, because that is the common case and it has a one-line fix.
 */
export async function renderPdf(printHtmlPath: string, options: PdfOptions = {}): Promise<Buffer> {
  const { chromium } = await loadPlaywright();

  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();

    // `file://` rather than a served URL: the shell inlines everything, so
    // there is nothing to serve, and a build that needed a server to export
    // would need one in CI too.
    await page.goto(pathToFileURL(printHtmlPath).href, { waitUntil: "load" });

    // The pages do not exist until the shell has cloned them. Printing early
    // yields one page per slide and no build at all — the exact failure this
    // whole feature exists to prevent.
    await page.waitForSelector(`html[${READY_ATTRIBUTE}]`, {
      timeout: options.timeoutMs ?? 30_000,
    });

    // Screen styles, not print styles: the shell already *is* the print
    // layout, and `@media print` there only drops the canvas background.
    await page.emulateMedia({ media: "print" });

    return await page.pdf({
      // The shell's `@page` sets the size from the deck's aspect ratio.
      preferCSSPageSize: options.width === undefined,
      width: options.width,
      height: options.height,
      printBackground: true,
      margin: { top: "0", right: "0", bottom: "0", left: "0" },
    });
  } finally {
    await browser.close();
  }
}

/** Number of pages in a PDF, read from its page tree. */
export async function countPdfPages(path: string): Promise<number> {
  const source = await readFile(path, "latin1");

  // Every page object declares its type. Counting them needs no parser and no
  // dependency, which matters for a check that only exists to catch a build
  // that silently produced one page.
  return (source.match(/\/Type\s*\/Page[^s]/g) ?? []).length;
}

async function loadPlaywright(): Promise<typeof import("playwright")> {
  try {
    return await import("playwright");
  } catch {
    throw new Error(
      "PDF export needs Playwright, which is not installed.\n" +
        "  vp add -D playwright && vp exec playwright install chromium\n" +
        "Or turn it off with `pdf: false`.",
    );
  }
}
