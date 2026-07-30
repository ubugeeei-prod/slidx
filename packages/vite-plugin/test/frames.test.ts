/**
 * The frames an export asks a build for.
 *
 * The claim worth checking is the unit. A build that wrote one image per
 * *slide* would look right in a file listing and be wrong in the only way that
 * matters — the four-step build reduced to its punchline — so these count files
 * against stops rather than asserting that something appeared.
 *
 * It runs a real browser against the real emitted shell. A mocked renderer
 * would only prove the mock works, and the failure this guards against lives
 * entirely in what the browser does with the shell.
 */

import { mkdir, mkdtemp, readFile, readdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build } from "vite";
import { describe, expect, it } from "vite-plus/test";

import { FRAME_VARIABLE, frameRequested } from "../src/frames";
import { countPdfPages } from "../src/pdf";
import { slidx } from "../src/index";

/**
 * Whether a browser is actually installed.
 *
 * Playwright being a dependency does not mean Chromium is on the machine, so
 * these skip rather than failing every job that has not run `playwright
 * install`. Honest here in a way it usually is not: there is no way to check
 * rendered output without a renderer.
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
    "\nFrame tests skipped: no browser. `vp exec playwright install chromium` to run them.\n",
  );
}

/** Two slides, four stops: one resting, then a slide that builds twice. */
const DECK = {
  "0001.md": "---\ntitle: Making Decks Fast\n---\n\n# One\n",
  "0002.md": "## Two\n\n- a <!-- step -->\n- b <!-- step -->\n",
};

async function buildDeck(frame: string | undefined): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "slidx-frames-"));
  await mkdir(join(root, "slides"), { recursive: true });

  for (const [name, source] of Object.entries(DECK)) {
    await writeFile(join(root, "slides", name), source);
  }

  const before = process.env[FRAME_VARIABLE];
  if (frame === undefined) delete process.env[FRAME_VARIABLE];
  else process.env[FRAME_VARIABLE] = frame;

  try {
    await build({
      root,
      logLevel: "silent",
      plugins: [slidx()],
      build: { outDir: join(root, "dist") },
    });
  } finally {
    if (before === undefined) delete process.env[FRAME_VARIABLE];
    else process.env[FRAME_VARIABLE] = before;
  }

  return root;
}

async function listing(directory: string): Promise<string[]> {
  try {
    return (await readdir(directory)).sort();
  } catch {
    return [];
  }
}

describe("asking a build for frames", () => {
  it("renders nothing extra when nothing asked", () => {
    // The reason this is an environment variable and not an option: every
    // ordinary build has to stay free of a browser pass over every stop.
    expect(frameRequested({})).toBe(null);
    expect(frameRequested({ [FRAME_VARIABLE]: "keynote" })).toBe(null);
    expect(frameRequested({ [FRAME_VARIABLE]: " png " })).toBe("png");
  });
});

describe.skipIf(!hasBrowser)("rendering frames", () => {
  it("writes one image per stop, not one per slide", async () => {
    // Four stops across two slides. Three of them belong to the second slide,
    // and a build that collapsed them would show its last bullet and nothing
    // of how the slide got there.
    const root = await buildDeck("png");

    expect(await listing(join(root, "dist/export/png"))).toEqual([
      "slide-01-stop-01.png",
      "slide-02-stop-01.png",
      "slide-02-stop-02.png",
      "slide-02-stop-03.png",
    ]);
  }, 120_000);

  it("writes images that are actually PNG", async () => {
    // A file with the right name and the wrong bytes is the failure mode an
    // export cannot afford: it is opened somewhere else, by someone else.
    const root = await buildDeck("png");
    const image = await readFile(join(root, "dist/export/png/slide-01-stop-01.png"));

    expect([...image.subarray(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
  }, 120_000);

  it("writes one document per slide, holding that slide's stops as its pages", async () => {
    // The thing a single PDF cannot be, and the reason the split happens in the
    // browser that printed it: nothing is dropped, the file boundary is just
    // the slide.
    const root = await buildDeck("pdf-slides");

    expect(await listing(join(root, "dist/export/pdf"))).toEqual(["slide-01.pdf", "slide-02.pdf"]);
    expect(await countPdfPages(join(root, "dist/export/pdf/slide-01.pdf"))).toBe(1);
    expect(await countPdfPages(join(root, "dist/export/pdf/slide-02.pdf"))).toBe(3);
  }, 120_000);

  it("renders the deck's document even when the project leaves the PDF off", async () => {
    // `pdf` is off by default to keep a browser download out of every install.
    // A person asking for the document on a command line has answered that.
    const root = await buildDeck("pdf");
    const document = await readFile(join(root, "dist/deck.pdf"));

    expect(document.subarray(0, 5).toString()).toBe("%PDF-");
  }, 120_000);

  it("keeps the frames out of the deck's own pages", async () => {
    // They are staging for one command, not part of the site. A static host
    // serving forty screenshots of a talk was nobody's intention.
    const root = await buildDeck("png");

    expect(await listing(join(root, "dist/slides/export"))).toEqual([]);
    expect(await listing(join(root, "dist/export"))).toEqual(["png"]);
  }, 120_000);

  it("leaves an ordinary build with no frames in it at all", async () => {
    const root = await buildDeck(undefined);

    expect(await listing(join(root, "dist/export"))).toEqual([]);
  }, 120_000);
});
