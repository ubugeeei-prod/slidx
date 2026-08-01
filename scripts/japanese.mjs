/**
 * The README's Japanese typesetting picture, from the real pipeline.
 *
 * Same arrangement as `scripts/screenshot.mjs` and for the same reason: an
 * image that was made by hand stops being true the moment the renderer changes
 * and nobody notices. This one fails to regenerate instead.
 *
 * ```sh
 * vp run media:japanese
 * ```
 *
 * # Why a second example deck
 *
 * The thing being shown is decided per *document*. `lang:` lands on `<html>`,
 * and `slidx_theme`'s CJK setting is scoped to it — so a mostly-English deck
 * with one Japanese slide is, correctly, an English deck, and adding a slide to
 * `examples/deck` would have produced a picture of the default behaviour.
 *
 * # Why the preview example rather than the plugin
 *
 * `examples/deck` is built through Vite because it exercises the plugin: assets,
 * steps, a runtime, a PDF. This deck is two files with none of that, and
 * `slidx_render`'s own preview example renders exactly the same shell from
 * exactly the same crate. Nothing a bundler does would appear in the image.
 */

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SOURCE = "examples/japanese/slides";
const OUT = "docs/media";

/**
 * Which slide the README shows.
 *
 * The second one, because it is the one with something to prove: a heading that
 * breaks at a phrase boundary, trimmed 約物, mixed script, and a measure. The
 * title slide is a title slide in any language.
 */
const SLIDE = 1;

const work = mkdtempSync(join(tmpdir(), "slidx-japanese-"));

try {
  const pages = join(work, "pages");
  const images = join(work, "images");

  run("cargo", ["run", "-p", "slidx_render", "--example", "preview", "--", SOURCE, pages]);
  run("node", ["scripts/screenshot.mjs", pages, images]);

  // Collated the way `screenshot.mjs` collates its pages, so the slide picked
  // here is the slide numbered there. Reading the directory rather than
  // spelling the name keeps a retitled slide from silently picking the wrong
  // image — a file name derived from a Japanese title is not one to hard-code.
  const shot = (scheme) =>
    readdirSync(images)
      .filter((name) => name.endsWith(`-${scheme}.png`))
      .sort((a, b) => a.localeCompare(b, "en"))[SLIDE];

  for (const scheme of ["light", "dark"]) {
    const name = shot(scheme);
    if (name === undefined) throw new Error(`no ${scheme} image for slide ${SLIDE + 1}`);

    const out = join(OUT, `japanese-${scheme}.png`);
    copyFileSync(join(images, name), out);
    process.stdout.write(`  ${out}\n`);
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}

function run(command, args) {
  execFileSync(command, args, { stdio: ["ignore", "ignore", "inherit"] });
}
