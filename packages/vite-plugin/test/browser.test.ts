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
import { describe, expect, it, afterAll, beforeAll } from "vite-plus/test";

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
  ".css": "text/css; charset=utf-8",
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

/** A deck whose author placed a speaker camera on the slide. */
let cameraPage: string;

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
  const [plain, staged, camera] = await Promise.all([
    // Published, and at a stated address, so the page under test carries every
    // absolute URL a deck can carry — a canonical, an `og:image`, structured
    // data. None of them may cost the room a request.
    buildDeck("browser", {
      "0001.md":
        "---\ndraft: false\nurl: https://example.com/talk/\n---\n\n# Making Decks Fast\n\nA framework.\n\n{width=full}\nA deliberate full-width block.\n",
    }),
    buildDeck("staged", {
      "0001.md":
        "---\nbudget: 30s\n---\n\n# Latency\n\nDropped to [120ms]{#latency}[38ms]{#latency}.\n\n- Now\n- Later <!-- step -->\n",
      "0002.md": "---\nbudget: 30s\n---\n\n# After\n",
    }),
    buildDeck("camera", {
      "0001.md": "---\nlayout: aside\ncamera: side\n---\n\n# Remote\n\nA talk from a desk.\n",
    }),
  ]);

  page = pathToFileURL(join(plain, "slides", "index.html")).href;
  served = await serve(staged);
  cameraPage = pathToFileURL(join(camera, "slides", "index.html")).href;
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

    // Every request the page makes, so the offline guarantee is measured
    // rather than inferred from the markup. The page under test names a
    // canonical URL, an `og:image` and a JSON-LD `@context`, none of which a
    // browser fetches — and this is the assertion that says so out loud.
    const requested: string[] = [];
    tab.on("request", (request) => requested.push(request.url()));

    await tab.goto(page);

    return {
      errors,
      requested,
      ...(await tab.evaluate(() => {
        const slide = document.querySelector(".slidx-slide");
        const heading = document.querySelector("h1");
        const region = document.querySelector<HTMLElement>(".slidx-region");
        const blocks = document.querySelectorAll<HTMLElement>(".slidx-block");
        const fitted = blocks[1];
        const full = blocks[2];
        if (slide === null || heading === null || region === null || !fitted || !full) {
          throw new Error("the slide did not render");
        }

        const box = slide.getBoundingClientRect();

        // Counted by what the browser would run. A `<script>` holding
        // `application/ld+json` is a container for a block of JSON — an unknown
        // type is never executed, and `document.scripts` counts it anyway.
        const executable = [...document.scripts].filter(
          (script) =>
            script.type === "" || script.type === "module" || /javascript/i.test(script.type),
        );

        return {
          slideWidth: box.width,
          slideHeight: box.height,
          headingPx: Number.parseFloat(getComputedStyle(heading).fontSize),
          regionWidth: region.getBoundingClientRect().width,
          fittedWidth: fitted.getBoundingClientRect().width,
          fullWidth: full.getBoundingClientRect().width,
          fittedToken: fitted.getAttribute("data-slidx-width"),
          fullToken: full.getAttribute("data-slidx-width"),
          scripts: executable.length,
          // The two halves fail differently: an inline script is bytes in the
          // document, a `src` is a request off the venue's wifi.
          fetched: executable.filter((script) => script.getAttribute("src") !== null).length,
          navigators: executable.filter((script) =>
            (script.textContent ?? "").includes("slidx-slide-nav"),
          ).length,
          text: heading.textContent ?? "",
        };
      })),
    };
  } finally {
    await browser.close();
  }
}

/**
 * How much of the Japanese setting each engine actually does.
 *
 * `docs/content/typography.md` publishes this table, and a support table copied
 * off a website is out of date the week it is written. This is where the table
 * comes from.
 *
 * It fails when a browser *gains* one of these, which is the point: the two
 * Chromium-only rows are described in the documentation as a degradation, and
 * that sentence should stop being true the moment it stops being true.
 */
const CJK_SUPPORT: Record<string, Record<Engine, boolean>> = {
  "line-break: strict": { chromium: true, firefox: true, webkit: true },
  'font-feature-settings: "palt" 1': { chromium: true, firefox: true, webkit: true },
  "text-spacing-trim: trim-start": { chromium: true, firefox: false, webkit: false },
  "word-break: auto-phrase": { chromium: true, firefox: false, webkit: false },
};

describe.each(ENGINES)("%s", (engine) => {
  const runs = it.skipIf(!available[engine]);

  runs(
    "does as much of the Japanese setting as the documentation says it does",
    async () => {
      const playwright = await import("playwright");
      const browser = await playwright[engine].launch();

      try {
        const tab = await browser.newPage();
        const measured = await tab.evaluate(
          (declarations) =>
            Object.fromEntries(
              declarations.map((declaration) => {
                const [property, value] = declaration.split(/:\s*/);
                return [declaration, CSS.supports(property!, value!)];
              }),
            ),
          Object.keys(CJK_SUPPORT),
        );

        for (const [declaration, engines] of Object.entries(CJK_SUPPORT)) {
          expect(
            measured[declaration],
            `${engine} ${measured[declaration] ? "now supports" : "no longer supports"} ` +
              `${declaration} — update the table in docs/content/typography.md`,
          ).toBe(engines[engine]);
        }
      } finally {
        await browser.close();
      }
    },
    120_000,
  );

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

  runs("fits ordinary blocks to content and keeps explicit full width", async () => {
    const measured = await measure(engine, 1280, 800);

    expect(measured.fittedToken).toBeNull();
    expect(measured.fittedWidth).toBeLessThan(measured.regionWidth);
    expect(measured.fullToken).toBe("full");
    expect(measured.fullWidth).toBeCloseTo(measured.regionWidth, 1);
  });

  runs(
    "fetches no script at all",
    async () => {
      // The claim on the front page, checked in the browser rather than in the
      // emitted string: nothing to block rendering, nothing to go wrong in the
      // room, nothing to fail with the network cable pulled.
      //
      // The one thing that runs is the navigator, inline, which is what gives a
      // slide with no steps a key and a clicker at all — see
      // `slidx_render::navigation`. It is a few hundred bytes already in the
      // document, so it is not a request and cannot fail on its own.
      const { scripts, fetched, navigators, errors, text } = await measure(engine, 1280, 800);

      expect(fetched).toBe(0);
      expect(scripts).toBe(navigators);
      expect(navigators).toBe(1);
      expect(errors).toEqual([]);
      expect(text).toBe("Making Decks Fast");
    },
    120_000,
  );

  runs(
    "makes no request to anywhere",
    async () => {
      // The offline guarantee, measured. This deck's pages name an origin —
      // there is a canonical link, an `og:image` and a JSON-LD `@context` in
      // the head of the page being opened — and the number of requests a
      // browser makes because of them has to be zero. The only thing fetched
      // is the document itself, off `file://`.
      const { requested } = await measure(engine, 1280, 800);

      expect(requested.filter((url) => !url.startsWith("file://"))).toEqual([]);
    },
    120_000,
  );
});

/**
 * A complete rehearsal across real presenter documents.
 *
 * This is deliberately a cross-page browser test rather than a renderer
 * assertion. The failure #127 described was exactly that the recorder existed
 * as code while no speaker could reach it; this starts it through the visible
 * control, navigates the deck, and reads the report the speaker receives.
 */
describe.each(ENGINES)("%s, rehearsing from the presenter view", (engine) => {
  const runs = it.skipIf(!available[engine]);

  runs(
    "keeps time across slides and finishes with a budget report",
    async () => {
      const playwright = await import("playwright");
      const browser = await playwright[engine].launch();
      const tab = await browser.newPage();
      const errors: string[] = [];
      tab.on("pageerror", (error) => errors.push(error.message));

      try {
        await tab.goto(`${served}/slides/presenter/`);
        await tab.click('[data-slidx-action="rehearse"]');
        await tab.waitForTimeout(1_100);

        // Two stops within the first slide, then the next presenter document.
        await tab.keyboard.press("ArrowRight");
        await tab.keyboard.press("ArrowRight");
        await Promise.all([
          tab.waitForURL(/\/slides\/2\/presenter\/$/),
          tab.keyboard.press("ArrowRight"),
        ]);

        await tab.waitForFunction(() =>
          document
            .querySelector("[data-slidx-rehearsal-status]")
            ?.textContent?.includes("recording"),
        );
        await tab.waitForTimeout(1_100);
        await tab.click('[data-slidx-action="finish-rehearsal"]');
        await tab.waitForFunction(
          () => !(document.querySelector("[data-slidx-rehearsal-report]") as HTMLElement).hidden,
        );
        await tab.setViewportSize({ width: 390, height: 844 });

        const result = await tab.evaluate(() => {
          // Two keys share the prefix now: the run in progress, and the runs
          // that have ended for the trend to compare. `find` used to be exact
          // because there was only ever one.
          const entries = Object.keys(localStorage).filter((entry) =>
            entry.startsWith("slidx:rehearsal:"),
          );
          const key = entries.find((entry) => !entry.endsWith(":history"));
          const past = entries.find((entry) => entry.endsWith(":history"));
          if (!key) throw new Error("the rehearsal was not stored");

          return {
            clock: document.querySelector("[data-slidx-elapsed]")?.textContent,
            report: JSON.parse(localStorage.getItem(key) ?? "null"),
            history: JSON.parse(localStorage.getItem(past ?? "") ?? "null"),
            trend: document.querySelector("[data-slidx-rehearsal-trend]")?.textContent,
            rows: [...document.querySelectorAll("[data-slidx-rehearsal-slides] li")].map(
              (row) => row.textContent,
            ),
            viewport: {
              inner: window.innerWidth,
              content: document.documentElement.scrollWidth,
            },
          };
        });

        expect(result.clock).not.toBe("0:00");
        expect(result.report).toMatchObject({
          status: "finished",
          slides: [
            { budgetMs: 30_000, visits: 1 },
            { budgetMs: 30_000, visits: 1 },
          ],
        });
        expect(result.report.slides[0].actualMs).toBeGreaterThan(0);
        expect(result.report.slides[1].actualMs).toBeGreaterThan(0);
        // A finished run is filed for the next one to be compared against, and
        // the note says so rather than inventing a direction from one run.
        expect(result.history).toHaveLength(1);
        expect(result.trend).toContain("First rehearsal recorded");
        expect(result.rows).toHaveLength(2);
        expect(result.rows.every((row) => row?.includes("/ 30s"))).toBe(true);
        expect(result.viewport.content).toBe(result.viewport.inner);
        expect(errors).toEqual([]);
      } finally {
        await browser.close();
      }
    },
    120_000,
  );
});

/** The window's counter, set by the init script below. */
interface Counted extends Window {
  __cameraRequests: number;
}

/**
 * Opens a deck that declares a speaker camera, watching for any request for one.
 *
 * The counter is installed before the page exists, so it sees a request made
 * from anywhere — an inline script, a module, a stylesheet's side effect. A
 * page that prompts once is a page an audience closes, and "did it prompt" is
 * only answerable in a browser.
 */
async function openDeclaringCamera(engine: Engine) {
  const playwright = await import("playwright");
  const browser = await playwright[engine].launch();

  try {
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });

    await context.addInitScript(() => {
      (window as unknown as Counted).__cameraRequests = 0;

      const devices: MediaDevices | undefined = navigator.mediaDevices;
      if (devices === undefined) return;

      const original = devices.getUserMedia.bind(devices);
      devices.getUserMedia = (constraints?: MediaStreamConstraints) => {
        (window as unknown as Counted).__cameraRequests += 1;
        return original(constraints);
      };
    });

    const tab = await context.newPage();
    const errors: string[] = [];
    tab.on("pageerror", (error) => errors.push(error.message));

    await tab.goto(cameraPage);

    return {
      errors,
      ...(await tab.evaluate(() => {
        const tile = document.querySelector("[data-slidx-camera]");
        const heading = document.querySelector("h1");

        // Counted the way `measure` counts it: by what the browser would run.
        // The structured data in the head is a `<script>` holding a block of
        // JSON, of a type nothing executes, and only something that executes
        // could ask for a device.
        const executable = [...document.scripts].filter(
          (script) =>
            script.type === "" || script.type === "module" || /javascript/i.test(script.type),
        );

        return {
          requests: (window as unknown as Counted).__cameraRequests,
          scripts: executable.length,
          navigators: executable.filter((script) =>
            (script.textContent ?? "").includes("slidx-slide-nav"),
          ).length,
          videos: document.querySelectorAll("video").length,
          declared: tile !== null,
          tileWidth: tile === null ? 0 : tile.getBoundingClientRect().width,
          heading: heading?.textContent ?? "",
        };
      })),
    };
  } finally {
    await browser.close();
  }
}

/**
 * The speaker's camera, on the page a stranger opens.
 *
 * This is the constraint the whole feature is shaped around, and the only place
 * it can honestly be checked. A published deck is a static page reached from a
 * link, a QR code, or an archive years later, and the browser is the only thing
 * that can say whether opening it asks for a webcam.
 *
 * Checked on `file://` for the same reason as everything else here: a deck off
 * a USB stick is the worst case, and it must behave identically.
 */
describe.each(ENGINES)("%s, on a slide that declares a camera", (engine) => {
  const runs = it.skipIf(!available[engine]);

  runs(
    "never asks the reader for a webcam",
    async () => {
      const { requests, scripts, navigators, errors } = await openDeclaringCamera(engine);

      expect(requests).toBe(0);
      // The reason it cannot: the only thing on the page that runs is the
      // navigator, which moves between two addresses the markup already names
      // and touches no device.
      expect(scripts).toBe(navigators);
      expect(navigators).toBe(1);
      expect(errors).toEqual([]);
    },
    120_000,
  );

  runs(
    "carries the camera's place and leaves no hole in the slide",
    async () => {
      // The tile is in the document, so the runtime has somewhere to put a
      // stream when a speaker starts one. It paints nothing until then: an
      // empty rectangle on every audience slide would be worse than the
      // feature not existing.
      const { declared, tileWidth, videos, heading } = await openDeclaringCamera(engine);

      expect(declared).toBe(true);
      expect(tileWidth).toBe(0);
      expect(videos).toBe(0);
      expect(heading).toBe("Remote");
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
        await tab.keyboard.press("ArrowRight");
        await tab.waitForFunction(
          () => document.querySelector("[data-slidx-mark='latency']")?.textContent === "38ms",
        );

        // The URL carries the stop, so what is on screen can be linked to.
        expect(new URL(tab.url()).search).toBe("?step=2");

        await tab.keyboard.press("ArrowLeft");
        await tab.waitForFunction(
          () => document.querySelector("[data-slidx-mark='latency']")?.textContent === "120ms",
        );
        expect(new URL(tab.url()).search).toBe("?step=1");

        // `?step=0` is noise in a URL somebody is about to share.
        await tab.keyboard.press("ArrowLeft");
        await tab.waitForFunction(() => window.location.search === "");
        expect(new URL(tab.url()).search).toBe("");
        expect(errors).toEqual([]);
      } finally {
        await browser.close();
      }
    },
    120_000,
  );

  runs(
    "hides an unrevealed element until its stop",
    async () => {
      // The runtime used to write the attribute while the only stylesheet
      // that understood it stayed in node_modules. The timeline advanced and
      // every item was visible from the first frame.
      const { browser, tab } = await open("/slides/");

      try {
        const visibility = () =>
          tab.$eval("li:last-child", (element) => getComputedStyle(element).visibility);

        expect(await visibility()).toBe("hidden");

        await tab.keyboard.press("ArrowRight");
        await tab.waitForFunction(
          () => getComputedStyle(document.querySelector("li:last-child")!).visibility === "visible",
        );
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
      const { browser, take } = await open("/slides/?step=2");

      try {
        expect(await take()).toBe("38ms");
      } finally {
        await browser.close();
      }
    },
    120_000,
  );

  runs(
    "keeps the stage off a slide with no steps",
    async () => {
      // Served from the same origin as the staged slide, so this is not
      // passing because a module happened to be unreachable. The second slide
      // has one stop, and a finished slide fetches nothing: the only thing on
      // it that runs is the navigator, inline, which is how a clicker leaves a
      // slide that has nothing to reveal.
      const playwright = await import("playwright");
      const browser = await playwright[engine].launch();

      try {
        const tab = await browser.newPage();
        await tab.goto(`${served}/slides/2/`);

        // Executable scripts. The structured data in the head is a `<script>`
        // element holding JSON that nothing runs, and `document.scripts`
        // counts it regardless of type.
        const { running, fetched, navigators } = await tab.evaluate(() => {
          const executable = [...document.scripts].filter(
            (script) => script.type !== "application/ld+json",
          );

          return {
            running: executable.length,
            fetched: executable.filter((script) => script.getAttribute("src") !== null).length,
            navigators: executable.filter((script) =>
              (script.textContent ?? "").includes("slidx-slide-nav"),
            ).length,
          };
        });

        expect(fetched).toBe(0);
        expect(running).toBe(navigators);
        expect(navigators).toBe(1);
      } finally {
        await browser.close();
      }
    },
    120_000,
  );
});
