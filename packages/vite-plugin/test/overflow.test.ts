/**
 * Content that does not fit the slide.
 *
 * This is the one check in the linted set that cannot be reasoned to. Whether
 * a slide overflows depends on where its lines break, and where its lines break
 * depends on font metrics; a build-time model that counted characters would
 * report slides that fit as broken, and a linter that cries wolf is a linter
 * an author switches off. So the measurement is real, and these tests are the
 * proof that it is real: a deck that fits and a deck that does not, through the
 * same browser, over the same emitted artefact.
 *
 * They run over the *built* print shell rather than a fixture, because a
 * fixture would only prove that a fixture overflows.
 */

import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { build } from "vite";
import { describe, expect, it } from "vite-plus/test";

import { reportOverflow } from "../src/artifacts";
import { resolveOptions, printFileName } from "../src/options";
import { measureOverflow } from "../src/overflow";
import { slidx } from "../src/index";

/**
 * Whether a browser is actually installed.
 *
 * Playwright being a dependency does not mean Chromium is on the machine. The
 * skip is honest here in the way it is for the PDF tests: there is no way to
 * check this without a browser, and a mock would only prove the mock works.
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
    "\nOverflow tests skipped: no browser. `vp exec playwright install chromium` to run them.\n",
  );
}

/** A slide with more bullets on it than any design box holds. */
const CROWDED = [...Array(40).keys()].map((n) => `- point number ${n + 1}`).join("\n");

async function buildDeck(source: string) {
  const root = await mkdtemp(join(tmpdir(), "slidx-overflow-"));
  await mkdir(join(root, "slides"), { recursive: true });
  await writeFile(join(root, "slides", "0001.md"), source);

  await build({
    root,
    logLevel: "silent",
    // The measurement is what these tests are about, so it is not also run as
    // a side effect of the build under test.
    plugins: [slidx({ og: false, overflow: false })],
    build: { outDir: join(root, "dist") },
  });

  return join(root, "dist");
}

describe.skipIf(!hasBrowser)("measuring", () => {
  it("reports nothing for a slide that fits its box", async () => {
    const directory = await buildDeck("# One\n\n- a point\n- another\n");
    const measured = await measureOverflow(join(directory, printFileName(resolveOptions())));

    expect(measured).not.toBeNull();
    expect(measured).toEqual([{ slideIndex: 0, stop: 0, overHeight: 0, overWidth: 0 }]);
  }, 120_000);

  it("finds the content a slide with too much on it loses", async () => {
    // The failure the whole feature exists for: `overflow: hidden` means the
    // author sees the same clipped slide the room will, so nothing about
    // looking at it says anything is missing.
    const directory = await buildDeck(`# Everything\n\n${CROWDED}\n`);
    const measured = await measureOverflow(join(directory, printFileName(resolveOptions())));

    expect(measured?.[0]?.overHeight).toBeGreaterThan(0.05);
  }, 120_000);

  it("measures every stop, not only the resting frame", async () => {
    // A slide that fits until its last reveal is a slide that fits in every
    // rehearsal and fails once, on stage.
    const directory = await buildDeck("- a <!-- step -->\n- b <!-- step -->\n");
    const measured = await measureOverflow(join(directory, printFileName(resolveOptions())));

    expect(measured).toHaveLength(3);
    expect(measured?.map((found) => found.stop)).toEqual([0, 1, 2]);
  }, 120_000);
});

describe.skipIf(!hasBrowser)("reporting", () => {
  function reporter() {
    const said: string[] = [];
    return {
      said,
      context: { info: (m: string) => said.push(m), warn: (m: string) => said.push(m) },
    };
  }

  it("says which slide is clipped, in the linter's words", async () => {
    const source = `# Everything\n\n${CROWDED}\n`;
    const directory = await buildDeck(source);
    const { said, context } = reporter();

    await reportOverflow(context, directory, resolveOptions(), source, ["Everything"]);

    expect(said.join("\n")).toContain("overflow/clipped");
    expect(said.join("\n")).toContain("Everything");
  }, 120_000);

  it("says nothing about a deck whose slides all fit", async () => {
    const source = "# One\n\n- a point\n";
    const directory = await buildDeck(source);
    const { said, context } = reporter();

    await reportOverflow(context, directory, resolveOptions(), source, ["One"]);

    expect(said).toEqual([]);
  }, 120_000);

  it("does nothing at all when the check is turned off", async () => {
    const source = `# Everything\n\n${CROWDED}\n`;
    const directory = await buildDeck(source);
    const { said, context } = reporter();

    await reportOverflow(context, directory, resolveOptions({ overflow: false }), source, [
      "Everything",
    ]);

    expect(said).toEqual([]);
  }, 120_000);
});
