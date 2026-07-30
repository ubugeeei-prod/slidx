/**
 * The format, and the one place it has to work.
 *
 * A recording of the editor exists to be looked at in a README, and a README
 * renders an `<img>`. That is the whole constraint, and it is not one a unit test
 * can answer: whether an animation survives being an image is a question about
 * browsers. So the second half of this file opens the documentation page's own
 * markup in three engines and watches the picture to see whether it moves.
 *
 * The first half is about the size of the thing, which is a cost every clone of
 * this repository pays forever — so the sparse frames and the folding are
 * asserted rather than hoped for.
 */

import { readFileSync } from "node:fs";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { describe, expect, it } from "vite-plus/test";

import { encodeApng } from "../animate.mjs";
import { decodePng } from "../png.mjs";
import { mark } from "../stage.mjs";

/** A frame of flat colour, so a change to it is a change of a known size. */
function flat(width, height, [red, green, blue]) {
  const pixels = new Uint8Array(width * height * 4);

  for (let at = 0; at < width * height; at += 1) {
    pixels[at * 4] = red;
    pixels[at * 4 + 1] = green;
    pixels[at * 4 + 2] = blue;
    pixels[at * 4 + 3] = 0xff;
  }

  return pixels;
}

/** The same frame with one pixel changed. */
function dotted(pixels, width, x, y) {
  const changed = pixels.slice();
  const at = (y * width + x) * 4;
  changed[at] = 0xff;
  changed[at + 1] = 0x00;
  changed[at + 2] = 0x00;

  return changed;
}

/** An animated PNG read back as the chunks it is made of. */
function chunks(file) {
  const found = [];

  for (let at = 8; at + 8 <= file.length; ) {
    const length = file.readUInt32BE(at);
    const type = file.toString("ascii", at + 4, at + 8);
    found.push({ type, data: file.subarray(at + 8, at + 8 + length) });
    at += 12 + length;
  }

  return found;
}

function framesOf(file) {
  return chunks(file)
    .filter(({ type }) => type === "fcTL")
    .map(({ data }) => ({
      width: data.readUInt32BE(4),
      height: data.readUInt32BE(8),
      x: data.readUInt32BE(12),
      y: data.readUInt32BE(16),
      delay: (data.readUInt16BE(20) / data.readUInt16BE(22)) * 1000,
      blend: data[25],
    }));
}

const SIZE = { width: 40, height: 20 };

describe("an animation a README can render", () => {
  it("loops forever, because a reader arrives at a page mid-scroll", () => {
    const white = flat(40, 20, [0xff, 0xff, 0xff]);
    const control = chunks(
      encodeApng(
        [
          { pixels: white, delay: 100 },
          { pixels: dotted(white, 40, 5, 5), delay: 100 },
        ],
        SIZE,
      ),
    ).find(({ type }) => type === "acTL");

    expect(control.data.readUInt32BE(0)).toBe(2);
    // Zero plays is forever, which is the only useful number for a loop nobody
    // presses anything to start.
    expect(control.data.readUInt32BE(4)).toBe(0);
  });

  it("keeps a still as one frame that lasts longer, not as the same frame twice", () => {
    // A driver holds a pause by capturing the same screen again, which is the
    // honest way to author one. Storing it twice is only bytes.
    const white = flat(40, 20, [0xff, 0xff, 0xff]);
    const file = encodeApng(
      [
        { pixels: white, delay: 300 },
        { pixels: white, delay: 400 },
        { pixels: dotted(white, 40, 5, 5), delay: 120 },
      ],
      SIZE,
    );

    expect(framesOf(file).map((frame) => frame.delay)).toEqual([700, 120]);
  });

  it("stores the rectangle that changed rather than the whole picture", () => {
    // The reason a recording of an editor is affordable at all: between two
    // frames of a drag, a few per cent of the pixels move.
    const white = flat(40, 20, [0xff, 0xff, 0xff]);
    const file = encodeApng(
      [
        { pixels: white, delay: 100 },
        { pixels: dotted(white, 40, 7, 9), delay: 100 },
      ],
      SIZE,
    );

    const [first, second] = framesOf(file);

    expect(first).toMatchObject({ width: 40, height: 20, x: 0, y: 0 });
    expect(second).toMatchObject({ width: 1, height: 1, x: 7, y: 9 });
    // Composited over what is already on the canvas, which is what lets a frame
    // leave every pixel it did not touch alone.
    expect(second.blend).toBe(1);
  });

  it("is a plain still of the gesture to anything that cannot animate it", () => {
    // The first frame is also the file's ordinary image, so the failure mode of
    // an old viewer is a screenshot rather than a broken image icon.
    const white = flat(40, 20, [0xff, 0xff, 0xff]);
    const file = encodeApng(
      [
        { pixels: white, delay: 100 },
        { pixels: dotted(white, 40, 7, 9), delay: 100 },
      ],
      SIZE,
    );

    const still = decodePng(file);

    expect({ width: still.width, height: still.height }).toEqual(SIZE);
    expect([...still.pixels.subarray(0, 4)]).toEqual([0xff, 0xff, 0xff, 0xff]);
  });
});

describe("the line a gesture wrote", () => {
  it("marks what was not in the file before, and nothing else", () => {
    const before = "## Heading\n\nA paragraph.\n";
    const after = "## Heading\n\n{.right}\nA paragraph.\n";

    expect(mark(before, after).filter((line) => line.added)).toEqual([
      { text: "{.right}", added: true },
    ]);
  });

  it("marks nothing in a file nobody has been shown yet", () => {
    // The first frame is the deck as the author saved it, which is not a change.
    expect(mark("", "## Heading\n").some((line) => line.added)).toBe(false);
  });
});

/** The engines behind every browser a README is read in. */
const ENGINES = ["chromium", "firefox", "webkit"];

async function launchable(engine) {
  try {
    const playwright = await import("playwright");
    const browser = await playwright[engine].launch();
    await browser.close();
    return true;
  } catch {
    return false;
  }
}

const available = Object.fromEntries(
  await Promise.all(ENGINES.map(async (engine) => [engine, await launchable(engine)])),
);

const missing = ENGINES.filter((engine) => !available[engine]);

if (missing.length > 0) {
  process.stdout.write(
    `\nAnimated images: ${missing.join(", ")} not installed. ` +
      `\`vp exec playwright install ${missing.join(" ")}\` to run them.\n`,
  );
}

/**
 * The documentation page's own markup, pointed at the files in this checkout.
 *
 * Read out of the page rather than written here, so what is under test is the
 * embed a reader actually gets — a `<picture>` with one `<source>` per colour
 * scheme, which is the shape the README uses too.
 */
function embed() {
  const page = readFileSync(resolve("docs/content/layout.md"), "utf8");
  const found = /<picture>[\s\S]*?editor-arrange[\s\S]*?<\/picture>/.exec(page);

  if (found === null) throw new Error("layout.md no longer embeds the arrange recording");

  return found[0].replaceAll(
    /"\.\.\/media\/([\w.-]+)"/g,
    (_, file) => `"${pathToFileURL(resolve("docs/media", file)).href}"`,
  );
}

describe.each(ENGINES)("%s, on the page's own markup", (engine) => {
  const runs = it.skipIf(!available[engine]);

  /** Loads the embed, then watches the picture until it moves. */
  async function watch(scheme) {
    const playwright = await import("playwright");
    const browser = await playwright[engine].launch();

    try {
      const root = await mkdtemp(join(tmpdir(), "slidx-animate-"));
      const file = join(root, "embed.html");
      await writeFile(file, `<!doctype html><meta charset="utf-8">\n${embed()}\n`);

      const context = await browser.newContext({
        viewport: { width: 1200, height: 700 },
        colorScheme: scheme,
      });
      const page = await context.newPage();
      await page.goto(pathToFileURL(file).href);

      const picture = page.locator("img");
      await picture.waitFor();
      await page.waitForFunction(() => document.querySelector("img")?.complete === true);

      const chosen = await picture.evaluate((image) => image.currentSrc);
      const first = await picture.screenshot();
      let moved = false;

      // Sampled rather than timed: which frame is on screen when a screenshot
      // lands is the browser's business, and all this has to establish is that
      // the picture is not a still.
      for (let sample = 0; sample < 24 && !moved; sample += 1) {
        await page.waitForTimeout(120);
        moved = !(await picture.screenshot()).equals(first);
      }

      return { chosen, moved, shot: first };
    } finally {
      await browser.close();
    }
  }

  runs(
    "animates the recording inside an img, which is what a README renders",
    async () => {
      // The constraint the whole format was chosen for. A `<video>` in a README
      // is either stripped or a control nobody presses, so this is the only
      // place the recording can live — and whether it moves there is a question
      // about browsers rather than about the encoder.
      const { moved } = await watch("light");

      expect(moved).toBe(true);
    },
    120_000,
  );

  runs(
    "gives a reader in the dark the recording made in the dark",
    async () => {
      // The deck themes carry both schemes and so does the editor's chrome, so a
      // recording of one of them is half an answer.
      const light = await watch("light");
      const dark = await watch("dark");

      expect(light.chosen).toContain("editor-arrange-light.png");
      expect(dark.chosen).toContain("editor-arrange-dark.png");
      expect(dark.shot.equals(light.shot)).toBe(false);
    },
    180_000,
  );

  runs(
    "writes screenshots this repository's own decoder can read",
    async () => {
      // The other half of the loop: the frames a recording is made of are
      // screenshots, and they are decoded by `animate.mjs` rather than by a
      // library. A browser that started writing a different kind of PNG would
      // break the recordings, and this is where it would say so.
      const { shot } = await watch("light");
      const decoded = decodePng(shot);

      expect(decoded.width).toBeGreaterThan(0);
      expect(decoded.pixels.length).toBe(decoded.width * decoded.height * 4);
    },
    120_000,
  );
});
