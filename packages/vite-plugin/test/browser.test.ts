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
 * Most of these run over `file://` deliberately: a slide with no steps has no
 * script, no import, and no fetch, so there is nothing a server would add
 * except a way for the test to pass on a deck that would fail off a USB stick.
 *
 * The staged-slide block at the end is the exception, and the exception is
 * informative. A slide *with* steps imports the runtime as a module, and a
 * module import from a `file://` page is refused as cross-origin from a null
 * origin — so that block gets a server, and the contrast between the two is
 * the honest shape of the format.
 */

import { createServer, type Server } from "node:http";
import { readFile, mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { extname, join, normalize } from "node:path";
import { pathToFileURL } from "node:url";

import { build } from "vite";
import { describe, expect, it, afterAll, beforeAll } from "vitest";

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

const CONTENT_TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".svg": "image/svg+xml",
};

let server: Server | undefined;

/**
 * A static server for the one thing `file://` cannot do.
 *
 * A staged slide imports the runtime as a module, and a module import from a
 * `file://` page is a cross-origin request from a null origin whatever the
 * path says — the browser refuses it. That is a real property of the format,
 * not of this test, and it is why the print shell inlines its runtime instead.
 *
 * Everything else in this file stays on `file://` on purpose. Serving them
 * would let a deck that fails off a USB stick pass.
 */
async function serve(root: string): Promise<string> {
  server = createServer((request, response) => {
    // Normalised and re-rooted: a test server is still a server, and `..` in a
    // request path should not reach the machine's filesystem.
    const path = decodeURIComponent(new URL(request.url ?? "/", "http://localhost").pathname);
    // `index.html` is appended before normalising, not after: on Windows
    // `normalize` rewrites the separators, so a directory request arrives as
    // `\slides\` and there is no trailing `/` left to notice.
    const requested = path.endsWith("/") ? `${path}index.html` : path;
    const relative = normalize(requested).replace(/^(\.\.[/\\])+/, "");
    const file = join(root, relative);

    readFile(file).then(
      (body) => {
        response.writeHead(200, {
          "content-type": CONTENT_TYPES[extname(file)] ?? "application/octet-stream",
        });
        response.end(body);
      },
      () => {
        response.writeHead(404);
        response.end();
      },
    );
  });

  await new Promise<void>((resolve) => server?.listen(0, "127.0.0.1", resolve));

  const address = server.address();
  const port = typeof address === "object" && address !== null ? address.port : 0;

  return `http://127.0.0.1:${port}`;
}

async function stopServing(): Promise<void> {
  await new Promise<void>((resolve) => (server ? server.close(() => resolve()) : resolve()));
}

/** A deck with a heading, built once and read by every engine. */
let page: string;

/** Where the staged deck is served from, because a module needs an origin. */
let served: string;

async function buildDeck(name: string, slides: Record<string, string>): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), `slidx-${name}-`));
  await mkdir(join(root, "slides"), { recursive: true });

  for (const [file, source] of Object.entries(slides)) {
    await writeFile(join(root, "slides", file), source);
  }

  await build({
    root,
    logLevel: "silent",
    plugins: [slidx()],
    build: { outDir: join(root, "dist") },
  });

  return join(root, "dist");
}

beforeAll(async () => {
  const [plain, staged] = await Promise.all([
    buildDeck("browser", { "0001.md": "# Making Decks Fast\n\nA framework.\n" }),
    buildDeck("staged", {
      "0001.md": "# Latency\n\nDropped to [120ms]{#latency}[38ms]{#latency}.\n",
      "0002.md": "# After\n",
    }),
  ]);

  page = pathToFileURL(join(plain, "slides", "index.html")).href;
  served = await serve(staged);
}, 180_000);

afterAll(async () => {
  await stopServing();
});

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

/**
 * The compiled step pipeline, on the screen the audience actually watches.
 *
 * This is the deck's central feature and the easiest one to ship broken,
 * because every *other* way of reading a stop keeps working when the projector
 * does not: the PDF has a page per stop, the print shell walks them, and the
 * presenter view steps happily on the speaker's own laptop. A deck can pass
 * all of that and still be frozen on stop one in the room.
 *
 * The mark under test is a *take* — two adjacent marks sharing one key, which
 * compile to one element whose text changes. It is the case that cannot be
 * faked with CSS, so if this advances, the pipeline is genuinely running.
 */
describe.each(ENGINES)("%s, on a slide with steps", (engine) => {
  const runs = it.skipIf(!available[engine]);

  async function open(path: string) {
    const playwright = await import("playwright");
    const browser = await playwright[engine].launch();
    const tab = await (
      await browser.newContext({ viewport: { width: 1280, height: 800 } })
    ).newPage();

    const errors: string[] = [];
    tab.on("pageerror", (error) => errors.push(error.message));

    await tab.goto(`${served}${path}`);
    await tab.waitForFunction(() => document.querySelector("[data-slidx-staged]") !== null);

    const take = () => tab.textContent("[data-slidx-mark='latency']");

    return { browser, tab, errors, take };
  }

  runs(
    "changes an element that was already on screen",
    async () => {
      const { browser, tab, errors, take } = await open("/slides/");

      try {
        expect(await take()).toBe("120ms");

        await tab.keyboard.press("ArrowRight");
        await tab.waitForFunction(
          () => document.querySelector("[data-slidx-mark='latency']")?.textContent === "38ms",
        );

        // The URL carries the stop, so what is on screen can be linked to.
        expect(new URL(tab.url()).search).toBe("?step=1");

        await tab.keyboard.press("ArrowLeft");
        await tab.waitForFunction(
          () => document.querySelector("[data-slidx-mark='latency']")?.textContent === "120ms",
        );

        // `?step=0` is noise in a URL somebody is about to share.
        expect(new URL(tab.url()).search).toBe("");
        expect(errors).toEqual([]);
      } finally {
        await browser.close();
      }
    },
    120_000,
  );

  runs(
    "opens at the stop a link names",
    async () => {
      // A link to a build is a link to what was on screen when it was shared.
      const { browser, take } = await open("/slides/?step=1");

      try {
        expect(await take()).toBe("38ms");
      } finally {
        await browser.close();
      }
    },
    120_000,
  );

  runs(
    "leaves a slide with no steps alone",
    async () => {
      // Served from the same origin as the staged slide, so this is not
      // passing because a module happened to be unreachable. The second slide
      // has one stop, and a finished slide ships nothing.
      const playwright = await import("playwright");
      const browser = await playwright[engine].launch();

      try {
        const tab = await browser.newPage();
        await tab.goto(`${served}/slides/2/`);

        expect(await tab.evaluate(() => document.scripts.length)).toBe(0);
      } finally {
        await browser.close();
      }
    },
    120_000,
  );
});
