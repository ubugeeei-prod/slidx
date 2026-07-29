/**
 * What happens after the files are written.
 *
 * The PDF and the PNG social cards are both produced by driving a real
 * browser over the *emitted* output, which is what makes them agree with what
 * a person sees. Both are also optional in the same way: a missing browser
 * costs a nicer artifact and nothing else, so neither may fail a build that
 * otherwise succeeded.
 *
 * Split from the plugin because the plugin's job is deciding *what* a deck
 * contains, and this is the separate job of turning that into files a
 * platform will accept.
 */

import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { rasterise } from "./og";
import type { ResolvedOptions } from "./options";
import { measureOverflow } from "./overflow";
import { renderPdf } from "./pdf";
import { lintMeasured } from "./pipeline";
import { formatReport } from "./report";
import { printFileName } from "./options";

/** Just enough of the Rollup context to report, so tests need no plugin. */
export interface Reporter {
  info: (message: string) => void;
  warn: (message: string) => void;
}

/**
 * Converts every emitted card to PNG.
 *
 * Almost no social platform renders SVG, so the PNG is the one that matters —
 * but a missing browser costs a nicer link preview and nothing else, so this
 * reports and moves on rather than failing a build over it.
 */
export async function rasteriseCards(
  context: Reporter,
  directory: string,
  options: ResolvedOptions,
): Promise<void> {
  const cardDirectory = options.base ? join(directory, options.base) : directory;

  let names: string[];
  try {
    names = (await readdir(cardDirectory)).filter(
      (name) => name.startsWith("og") && name.endsWith(".svg"),
    );
  } catch {
    return;
  }

  let written = 0;
  for (const name of names) {
    const svg = await readFile(join(cardDirectory, name), "utf8");
    const png = await rasterise(svg);

    if (!png) {
      context.warn(
        "Social cards stayed as SVG: no browser to convert them.\n" +
          "  vp add -D playwright && vp exec playwright install chromium",
      );
      return;
    }

    await writeFile(join(cardDirectory, name.replace(/\.svg$/, ".png")), png);
    written += 1;
  }

  if (written > 0) context.info(`rendered ${written} social card(s)`);
}

/**
 * Exports the deck to PDF.
 *
 * A speaker whose PDF did not render still has a deck to present; losing both
 * to one problem is the failure worth designing against, so this reports and
 * returns rather than throwing.
 */
export async function exportPdf(
  context: Reporter,
  directory: string,
  options: ResolvedOptions,
): Promise<void> {
  if (!options.pdf || !options.print) return;

  const shell = join(directory, printFileName(options));
  const target = join(directory, options.pdf.fileName);

  try {
    await writeFile(target, await renderPdf(shell));
    context.info(`exported ${options.pdf.fileName}`);
  } catch (error) {
    context.warn(`PDF export failed — the deck built anyway.\n${(error as Error).message}`);
  }
}

/**
 * Measures the built pages and reports anything the design box cut off.
 *
 * This runs against written files rather than the model, so it necessarily
 * happens after the bundle exists — which is also why it warns rather than
 * failing the build. Stopping here would leave a written `dist` beside a red
 * build, and a speaker who then has neither a passing build nor a reason to
 * trust the deck sitting in the directory.
 *
 * A missing browser is reported as *unchecked*, not as clean. Those are
 * opposite answers, and only one of them means the deck is fine.
 */
export async function reportOverflow(
  context: Reporter,
  directory: string,
  options: ResolvedOptions,
  source: string,
  titles: readonly (string | null)[],
): Promise<void> {
  if (!options.overflow || !options.print) return;

  let findings;
  try {
    const measured = await measureOverflow(join(directory, printFileName(options)));

    if (measured === null) {
      context.info(
        "Content overflow unchecked: no browser to measure the built pages with.\n" +
          "  vp add -D playwright && vp exec playwright install chromium",
      );
      return;
    }

    findings = await lintMeasured(source, measured, {
      separator: options.separator,
      theme: options.theme,
    });
  } catch (error) {
    context.warn(
      `Content overflow unchecked — the deck built anyway.\n${(error as Error).message}`,
    );
    return;
  }

  if (findings.length > 0) context.warn(`\n${formatReport(findings, titles)}`);
}
