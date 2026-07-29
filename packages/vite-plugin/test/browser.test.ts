/**
 * The deck, in every browser a talk will actually be given in.
 *
 * The roadmap word for this section is *verified, not assumed*, and it is
 * there because of how this project renders. A slide does not scale with a
 * transform and a resize listener — it is a size container, and every length
 * inside it is a share of the slide. That is one CSS mechanism carrying the
 * entire layout, with no script to fall back on, so "container queries are
 * widely supported now" is a claim to check rather than one to repeat.
 *
 * The failure it guards against is the one that already happened once here: a
 * layout that looks right on the machine it was written on and silently
 * computes to nothing somewhere else. `transform: scale()` was the first
 * attempt and it was inert, because `calc()` cannot divide a length by a
 * length — inert in a way that renders a *plausible* slide rather than an
 * obviously broken one, which is what made it survive review.
 *
 * So the test that matters here is not "the page loaded". It is that the
 * heading is a different number of pixels in a small window than in a large
 * one, in the right proportion. A browser without working container queries
 * passes every other check and fails that one.
 *
 * These run over `file://` deliberately: an audience slide has no script, no
 * import, and no fetch, so there is nothing a server would add except a way
 * for the test to pass on a deck that would fail off a USB stick.
 */

import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import { build } from "vite";
import { describe, expect, it, beforeAll } from "vitest";

import { slidx } from "../src/index";

/** The engines behind every browser a conference room has. */
const ENGINES = ["chromium", "firefox", "webkit"] as const;

type Engine = (typeof ENGINES)[number];

/**
 * Which engines are actually on this machine.
 *
 * Playwright being a dependency does not mean its browsers are downloaded —
 * they are a separate fetch of a few hundred megabytes. Rather than fail on
 * every developer machine, each engine skips independently and CI installs
 * all three. The skip is loud, because a matrix that quietly ran one engine
 * would be worse than no matrix at all.
 */
async function launchable(engine: Engine): Promise<boolean> {
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
  await Promise.all(ENGINES.map(async (engine) => [engine, await launchable(engine)] as const)),
) as Record<Engine, boolean>;

const missing = ENGINES.filter((engine) => !available[engine]);

if (missing.length > 0) {
  process.stdout.write(
    `\nBrowser matrix: ${missing.join(", ")} not installed. ` +
      `\`vp exec playwright install ${missing.join(" ")}\` to run them.\n`,
  );
}

/** A deck with a heading, built once and read by every engine. */
let page: string;

beforeAll(async () => {
  const root = await mkdtemp(join(tmpdir(), "slidx-browser-"));
  await mkdir(join(root, "slides"), { recursive: true });
  await writeFile(join(root, "slides", "0001.md"), "# Making Decks Fast\n\nA framework.\n");

  await build({
    root,
    logLevel: "silent",
    plugins: [slidx()],
    build: { outDir: join(root, "dist") },
  });

  page = pathToFileURL(join(root, "dist", "slides", "index.html")).href;
}, 120_000);

/** One engine, one page, measured at a given viewport. */
async function measure(engine: Engine, width: number, height: number) {
  const playwright = await import("playwright");
  const browser = await playwright[engine].launch();

  try {
    const context = await browser.newContext({ viewport: { width, height } });
    const tab = await context.newPage();

    const errors: string[] = [];
    tab.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    tab.on("pageerror", (error) => errors.push(error.message));

    await tab.goto(page);

    return {
      errors,
      ...(await tab.evaluate(() => {
        const slide = document.querySelector(".slidx-slide");
        const heading = document.querySelector("h1");
        if (slide === null || heading === null) throw new Error("the slide did not render");

        const box = slide.getBoundingClientRect();

        return {
          slideWidth: box.width,
          slideHeight: box.height,
          headingPx: Number.parseFloat(getComputedStyle(heading).fontSize),
          scripts: document.scripts.length,
          text: heading.textContent ?? "",
        };
      })),
    };
  } finally {
    await browser.close();
  }
}

describe.each(ENGINES)("%s", (engine) => {
  const runs = it.skipIf(!available[engine]);

  runs(
    "sizes the heading against the slide, not the window",
    async () => {
      // The mechanism the whole layout rests on. A browser that ignored
      // `container-type: size` resolves every `cqh` length to the same number
      // at both viewports, and this is the only check that notices.
      const large = await measure(engine, 1280, 800);
      const small = await measure(engine, 640, 400);

      expect(large.headingPx).toBeGreaterThan(0);
      expect(small.headingPx).toBeGreaterThan(0);

      const typeRatio = large.headingPx / small.headingPx;
      const slideRatio = large.slideHeight / small.slideHeight;

      expect(typeRatio).toBeCloseTo(slideRatio, 1);
      expect(typeRatio).toBeGreaterThan(1.5);
    },
    120_000,
  );

  runs(
    "keeps the deck's aspect ratio whatever shape the window is",
    async () => {
      // A projector is 16:9 and a laptop lid is not. The slide is the design
      // box, so it keeps its ratio and letterboxes rather than stretching.
      for (const [width, height] of [
        [1280, 800],
        [800, 1200],
      ] as const) {
        const { slideWidth, slideHeight } = await measure(engine, width, height);

        expect(slideWidth / slideHeight, `${width}x${height}`).toBeCloseTo(16 / 9, 1);
        expect(slideWidth, `${width}x${height}`).toBeLessThanOrEqual(width);
      }
    },
    120_000,
  );

  runs(
    "runs no script at all",
    async () => {
      // The claim on the front page, checked in the browser rather than in the
      // emitted string: nothing to block rendering, nothing to go wrong in the
      // room, nothing to fail with the network cable pulled.
      const { scripts, errors, text } = await measure(engine, 1280, 800);

      expect(scripts).toBe(0);
      expect(errors).toEqual([]);
      expect(text).toBe("Making Decks Fast");
    },
    120_000,
  );
});
