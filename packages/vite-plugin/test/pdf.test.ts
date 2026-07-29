/**
 * Exporting a deck to PDF.
 *
 * The claim worth checking is not "a PDF appears" — it is that the PDF has a
 * page for every *stop*. A build that silently produced one page per slide
 * would look right in a file listing and be wrong in the only way that
 * matters, so these count pages rather than bytes.
 *
 * The export runs a real browser against the real emitted shell. There is no
 * way to check this without one, and a mocked exporter would only prove the
 * mock works.
 */

import { mkdtemp, mkdir, readdir, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build } from "vite";
import { describe, expect, it } from "vitest";

import { countPdfPages } from "../src/pdf";
import { slidx } from "../src/index";

/**
 * Whether a browser is actually installed.
 *
 * Playwright being a dependency does not mean Chromium is on the machine —
 * the binaries are a separate download. Rather than fail on every machine and
 * every CI job that has not run `playwright install`, these skip and CI runs
 * them in one job that has. Skipping is honest here in a way it usually is
 * not: there is no way to check PDF output without a browser, and a mocked
 * exporter would only prove the mock works.
 */
async function browserAvailable(): Promise<boolean> {
  try {
    const { chromium } = await import("playwright");
    const browser = await chromium.launch();
    await browser.close();
    return true;
  } catch {
    return false;
  }
}

const hasBrowser = await browserAvailable();

if (!hasBrowser) {
  process.stdout.write(
    "\nPDF tests skipped: no browser. `vp exec playwright install chromium` to run them.\n",
  );
}

async function buildDeck(slides: Record<string, string>, options: Parameters<typeof slidx>[0]) {
  const root = await mkdtemp(join(tmpdir(), "slidx-pdf-"));
  await mkdir(join(root, "slides"), { recursive: true });

  for (const [name, source] of Object.entries(slides)) {
    await writeFile(join(root, "slides", name), source);
  }

  await build({
    root,
    logLevel: "silent",
    plugins: [slidx(options)],
    build: { outDir: join(root, "dist") },
  });

  return root;
}

describe.skipIf(!hasBrowser)("exporting", () => {
  it("writes one page per stop, not per slide", async () => {
    // Two slides, five stops: a resting frame plus two reveals on the second.
    const root = await buildDeck(
      { "0001.md": "# One\n", "0002.md": "- a <!-- step -->\n- b <!-- step -->\n" },
      { pdf: true },
    );

    expect(await countPdfPages(join(root, "dist/deck.pdf"))).toBe(4);
  }, 120_000);

  it("names the file what it was asked to", async () => {
    const root = await buildDeck({ "0001.md": "# One\n" }, { pdf: { fileName: "talk.pdf" } });

    expect(await readdir(join(root, "dist"))).toContain("talk.pdf");
  }, 120_000);

  it("writes nothing when the export is off", async () => {
    const root = await buildDeck({ "0001.md": "# One\n" }, {});
    const files = await readdir(join(root, "dist"));

    expect(files.filter((file) => file.endsWith(".pdf"))).toEqual([]);
  }, 120_000);

  it("produces a file with real content", async () => {
    // A zero-byte PDF is what a browser writes when the page never loaded.
    const root = await buildDeck({ "0001.md": "# One\n" }, { pdf: true });
    const { size } = await stat(join(root, "dist/deck.pdf"));

    expect(size).toBeGreaterThan(1_000);
  }, 120_000);

  it("leaves the deck built when the export fails", async () => {
    // A speaker whose PDF failed still has a deck to present. Losing both to
    // one problem is the failure worth designing against.
    const root = await buildDeck({ "0001.md": "# One\n" }, { pdf: true, print: false });
    const files = await readdir(join(root, "dist", "slides"));

    expect(files).toContain("index.html");
  }, 120_000);
});
