/**
 * Screenshots the rendered example deck.
 *
 * The images in the README are produced by the real pipeline — parse, lint,
 * theme, render — rather than drawn. A screenshot that was made by hand stops
 * being true the moment the renderer changes, and nobody notices; this one
 * fails to regenerate instead.
 *
 * ```sh
 * cargo run -p slidx_render --example preview -- examples/deck/slides dist/preview
 * node scripts/screenshot.mjs
 * ```
 */

import { readdirSync, mkdirSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { join, resolve } from "node:path";

import { chromium } from "playwright";

const SOURCE = process.argv[2] ?? "examples/deck/dist/slides";
const OUT = process.argv[3] ?? "docs/images";

/**
 * Rendered at twice the display size.
 *
 * The README is read on high-density screens, and a slide screenshot that is
 * soft undermines the thing it is arguing for.
 */
const VIEWPORT = { width: 1280, height: 720 };
const SCALE = 2;

/**
 * Every page the build emitted, audience and presenter alike.
 *
 * Read from the plugin's own output rather than from a bespoke renderer, so
 * the images are of the thing people install.
 */
function pagesIn(directory, prefix = "") {
  const found = [];

  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) found.push(...pagesIn(join(directory, entry.name), path));
    else if (entry.name.endsWith(".html")) found.push(path);
  }

  return found.sort();
}

const pages = pagesIn(SOURCE);

if (pages.length === 0) {
  process.stderr.write(`no .html files in ${SOURCE} — build the example deck first\n`);
  process.exit(1);
}

mkdirSync(OUT, { recursive: true });

const browser = await chromium.launch();

for (const scheme of ["light", "dark"]) {
  const context = await browser.newContext({
    viewport: VIEWPORT,
    deviceScaleFactor: SCALE,
    colorScheme: scheme,
  });
  const page = await context.newPage();

  for (const name of pages) {
    await page.goto(pathToFileURL(resolve(SOURCE, name)).href);

    // The slide itself rather than the viewport — the letterboxing around it
    // is the browser, not the deck. The presenter view *is* the whole window.
    const presenter = name.includes("presenter");
    const target = presenter ? page.locator(".slidx-presenter") : page.locator(".slidx-slide");
    await target.waitFor();

    const label = name.replace(/\/?index\.html$/, "").replaceAll("/", "-") || "1";
    const out = join(OUT, `${label}-${scheme}.png`);
    await target.screenshot({ path: out });
    process.stdout.write(`  ${out}\n`);
  }

  await context.close();
}

await browser.close();
process.stdout.write(`\n${pages.length * 2} image(s) in ${OUT}\n`);
