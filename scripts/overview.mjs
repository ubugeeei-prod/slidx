/**
 * The README's overview picture, from the real pipeline.
 *
 * Same arrangement as `scripts/screenshot.mjs` and `scripts/japanese.mjs`: an
 * image made by hand stops being true the moment the renderer changes and
 * nobody notices. This one fails to regenerate instead.
 *
 * ```sh
 * vp run media:overview
 * ```
 *
 * It shoots the whole page rather than one element, because the page *is* the
 * feature — a grid of every slide, which is the one thing a single slide
 * cannot show.
 */

import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import { chromium } from "playwright";

const OUT = "docs/media";
const VIEWPORT = { width: 1280, height: 400 };
const SCALE = 2;

/**
 * A deck long enough that the grid is the point.
 *
 * Twelve slides, because the feature answers "where is the one about X" and
 * four slides is a question nobody has.
 */
const TITLES = [
  ["Making Decks Fast", "A framework for the whole life of a talk — not just the slides."],
  [
    "What actually goes wrong",
    "- The venue Wi-Fi is down and the fonts were on a CDN\n" +
      "- The body text was 18px and unreadable from row 12\n" +
      "- A colour pair that looked fine on a laptop washed out\n" +
      "- The live demo died and there was no fallback",
  ],
  [
    "Steps are snapshots",
    "Each stop is a **complete** state, compiled ahead of time.\n\n" +
      "```rust\nlet frame = timeline.frame(step)?;\nrender(frame);\n```",
  ],
  [
    "The linter checks the room",
    "- Projector washout, not just a WCAG ratio\n" +
      "- Angular size from the back row\n" +
      "- Overflow, measured in a real browser",
  ],
  ["One document per slide", "No router. No framework. One URL each, and one parser behind them."],
  [
    "The visual editor",
    "The canvas is the deck's own page rather than a preview of it, so one drag\n" +
      "is one operation, one undo, and one line in the diff.",
  ],
  ["Two people, one file", "The dev server holds the one document, so a drag and a save merge."],
  ["Japanese, typeset", "禁則処理、約物のアキ、文節での折り返し。行送りは欧文と別に決まります。"],
  [
    "The presenter view",
    "- The next slide, and your notes\n- A clock against the slot you declared\n" +
      "- Which optional slides to drop when you are behind",
  ],
  ["Exporting", "```bash\nslidx export --target pdf\n```\n\nPDF, PNG, PPTX, and the static site."],
  [
    "Publishing",
    "Everything that needs no account, in one command — and the payload for the rest.",
  ],
  ["Thank you", "Questions?"],
];

const root = await mkdtemp(join(tmpdir(), "slidx-overview-"));

try {
  await mkdir(join(root, "slides"), { recursive: true });
  await writeFile(
    join(root, "slides", "0001.md"),
    TITLES.map(
      ([title, body], index) =>
        `${index === 0 ? "---\ntitle: Making Decks Fast\nhashtag: slidx\n---\n\n# " : "## "}${title}\n\n${body}\n`,
    ).join("\n---\n\n"),
  );

  const pages = join(root, "pages");
  execFileSync(
    "cargo",
    [
      "run",
      "--release",
      "-q",
      "-p",
      "slidx_render",
      "--example",
      "preview",
      "--",
      join(root, "slides"),
      pages,
    ],
    { stdio: ["ignore", "ignore", "inherit"] },
  );

  // The preview example writes one file per slide; the overview is its own
  // page, so it is rendered by the same crate through a one-line example.
  const overview = join(root, "overview.html");
  execFileSync(
    "cargo",
    [
      "run",
      "--release",
      "-q",
      "-p",
      "slidx_render",
      "--example",
      "overview",
      "--",
      join(root, "slides"),
      overview,
    ],
    { stdio: ["ignore", "ignore", "inherit"] },
  );

  const browser = await chromium.launch();
  for (const scheme of ["light", "dark"]) {
    const context = await browser.newContext({
      viewport: VIEWPORT,
      deviceScaleFactor: SCALE,
      colorScheme: scheme,
    });
    const page = await context.newPage();
    await page.goto(pathToFileURL(overview).href);
    await page.locator(".slidx-overview").waitFor();

    const out = join(OUT, `overview-${scheme}.png`);
    await page.screenshot({ path: out, fullPage: true });
    process.stdout.write(`  ${out}\n`);
    await context.close();
  }
  await browser.close();
} finally {
  await rm(root, { recursive: true, force: true });
}
