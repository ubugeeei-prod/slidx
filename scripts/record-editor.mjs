/**
 * The visual editor, recorded by being used.
 *
 * ```sh
 * vp run record:editor
 * ```
 *
 * `scripts/screenshot.mjs` set the standard the rest of this repository's
 * documentation is held to: a picture is output of the real pipeline, so one that
 * stopped being true fails to reproduce rather than quietly misleading.
 * `scripts/record.mjs` extended it to the terminal and to a built deck. The
 * editor is the third surface and the one a still cannot argue for at all —
 * dragging, scrubbing and reordering are gestures, and a photograph of a gesture
 * is a photograph of a slide.
 *
 * So nothing here is a file somebody recorded by hand. A real Vite dev server
 * serves the real editor over a real deck, Playwright performs the gesture with a
 * pointer, and the frames are what the browser drew.
 *
 * # Why frames rather than a video
 *
 * A video of a page is timed by the machine that recorded it, so the same gesture
 * on a slower laptop is a different file: every regeneration churns and the diff
 * says nothing. Here a frame is captured *after* a step of the gesture and
 * carries a delay the scene wrote down, so the recording is a function of the
 * steps rather than of how fast anything ran. It is also the only kind of
 * animation a README can render — `scripts/animate.mjs` has the format, and what
 * it costs.
 *
 * # What a scene is
 *
 * A few lines: the panels it wants in frame, the slide it opens, and the
 * gesture. The server, the deck, the crop, both colour schemes and the encoding
 * are this file's job, and how a gesture is performed and photographed is
 * [`gesture`](./gesture.mjs)'s — so adding one is adding a scene rather than
 * building a second pipeline.
 *
 * # What is kept deterministic, and how
 *
 * - **Nothing in a frame reads a clock.** The caret is hidden and every
 *   screenshot is taken with animations fast-forwarded, so nothing is caught
 *   mid-transition and no cursor blinks.
 * - **The pointer path is arithmetic** between two boxes the editor laid out,
 *   rather than a recording of a hand.
 * - **Every wait is on a condition** — the file having been written, the canvas
 *   having come back, the browser having painted — never on a duration.
 *
 * Two runs of this task write byte-identical files. What is *not* fixed is the
 * type: the editor's chrome and the deck's themes both ask for a system font
 * stack, so regenerating on another operating system re-renders every glyph.
 * That is already true of `screenshot.mjs` and is a property of the product
 * rather than of this script.
 */

import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { slidx } from "@slidx/vite-plugin";
import { chromium } from "playwright";
import { createServer } from "vite";

import { encodeApng } from "./animate.mjs";
import { play } from "./gesture.mjs";
import { decodePng } from "./png.mjs";
import { FILE_WIDTH, stagePage } from "./stage.mjs";

const OUT = process.argv[2] ?? "docs/media";

/** The deck every scene opens, which is the one the README's images come from. */
const DECK = "examples/deck/slides";

/**
 * The editor's own size, which is not the size of the recording.
 *
 * Both numbers are about the canvas. The chrome is a three-column grid of 232px,
 * the canvas, and 296px, so the width leaves the canvas panel a little over 800 —
 * wide enough that the slide in it is a slide rather than a thumbnail, since the
 * theme sizes every glyph as a share of the slide's own height.
 *
 * The height is chosen so the panel is about as tall as a 16:9 slide of that
 * width needs. A taller one letterboxes: the slide keeps its aspect ratio and
 * the frame around it goes empty, which in a recording is a hundred rows of
 * nothing that every clone of this repository pays for.
 */
const EDITOR = { width: 1400, height: 645 };

/**
 * A slide with two regions, and lines short enough to read beside the canvas.
 *
 * Written here rather than added to `examples/deck`, for the reason `record.mjs`
 * gives about the deck it lints: the point of the picture is the gesture, so the
 * slide that provokes it belongs next to the code that photographs it. The
 * example deck is the one a reader copies, and it has no reason to carry a slide
 * whose paragraphs are wrapped at 37 columns.
 */
const TWO_COLUMNS = `---
layout: split
budget: 60s
---

## Steps are snapshots

Each stop is a complete state.

Compiled ahead of time, so going back
costs what going forward costs.
`;

/**
 * Three slides with budgets, so the storyboard has a talk to lay against a slot.
 *
 * The example deck is four slides and declares twenty minutes, which is honest
 * and draws a bar that is mostly empty — a picture of this feature has to be of
 * a deck somebody has actually planned. Written here for the same reason the
 * two-column slide is: it exists for the recording, and the deck a reader copies
 * has no reason to carry it.
 */
const PLANNED = {
  "0005.md": `---
budget: 4m
---

## What we measured

<!-- notes: The numbers, before anybody argues about the method. -->
`,
  "0006.md": `---
budget: 5m
---

## How the pipeline fits together

<!-- notes: The four stages, and where the time went. -->
`,
  "0007.md": `---
budget: 3m30s
optional: false
---

## The parts we cut

<!-- notes: The three ideas that did not survive contact with a room. -->
`,
};

const SCENES = [
  {
    name: "arrange",
    // The claim the editor exists to keep, and the only way to show it: a block
    // crossing a region boundary while the file gains the one line that says so.
    // The canvas alone would be a recording of a slide.
    add: { "0005.md": TWO_COLUMNS },
    open: "0005.md",
    frame: [".slidx-canvas"],
    file: "0005.md",

    async play({ drag, hold, file }) {
      await hold(700);
      await drag({ block: 2, into: "right", steps: 9, each: 110 });
      await file();
      await hold(1600);
    },
  },

  {
    name: "storyboard",
    // The deck as a talk rather than as slides: every slide as wide as the time
    // it was given, laid against the slot the frontmatter declares. The gesture
    // is the one that changes the plan — `o` marks a slide optional, which is
    // one line in the file and a sentence about what dropping it would buy.
    add: PLANNED,
    open: "0003.md",
    editor: { width: 1040, height: 620 },
    frame: [".slidx-sb-sheet"],

    async play({ press, hold, shown }) {
      await hold(1000);
      await press("o");
      await shown(".slidx-sb-slack");
      await hold(1900);
    },

    async prepare({ click, shown }) {
      await click(".slidx-sb-launch");
      await shown(".slidx-sb-rows .slidx-sb-slide");
    },
  },
];

const tokens = execFileSync("cargo", ["run", "-q", "-p", "slidx_docs", "--example", "tokens"], {
  encoding: "utf8",
});

const scratch = mkdtempSync(join(tmpdir(), "slidx-record-editor-"));
mkdirSync(OUT, { recursive: true });

const browser = await chromium.launch();
let written = 0;

for (const scene of SCENES) {
  for (const scheme of ["light", "dark"]) {
    const out = join(OUT, `editor-${scene.name}-${scheme}.png`);
    const image = await record(scene, scheme);
    writeFileSync(out, image);
    written += 1;

    process.stdout.write(`  ${out} (${(image.length / 1024).toFixed(0)} kB)\n`);
  }
}

await browser.close();
rmSync(scratch, { recursive: true, force: true });

process.stdout.write(`\n${written} recording(s) in ${OUT}\n`);

/**
 * One scene, in one colour scheme, as one animated image.
 *
 * A server and a copy of the deck per recording rather than one shared between
 * them: the editor writes to the files it is given, so a scene that ran second
 * would open a deck the scene before it had already rearranged.
 */
async function record(scene, scheme) {
  const root = join(scratch, `${scene.name}-${scheme}`);
  const editorSize = scene.editor ?? EDITOR;
  cpSync(resolve(DECK), join(root, "slides"), { recursive: true });

  for (const [name, body] of Object.entries(scene.add ?? {})) {
    writeFileSync(join(root, "slides", name), body);
  }

  const server = await createServer({
    root,
    logLevel: "silent",
    plugins: [slidx()],
    // No watcher and no HMR socket, for the reason the session test gives: this
    // drives the editor's own routes and needs neither, and both hold handles
    // that outlive the server on Windows.
    server: { port: 0, watch: null, hmr: false },
  });

  await server.listen();
  const context = await browser.newContext({
    viewport: { width: editorSize.width + FILE_WIDTH, height: editorSize.height },
    colorScheme: scheme,
  });

  try {
    const page = await context.newPage();
    const broke = [];
    page.on("pageerror", (error) => broke.push(error.message));

    const stage = join(scratch, `${scene.name}-${scheme}.html`);
    writeFileSync(
      stage,
      stagePage(`${server.resolvedUrls.local[0]}__slidx/`, {
        tokens,
        editorWidth: editorSize.width,
        editorHeight: editorSize.height,
        withFile: scene.file !== undefined,
      }),
    );
    await page.goto(pathToFileURL(stage).href);

    const editor = await mounted(page);
    await select(page, editor, scene, root);

    // Before the frame is measured, because a scene about a panel that opens
    // has to open it first: a sheet nobody pressed the button for has no box.
    await scene.prepare?.(controls(page, editor));

    const size = await frameOn(page, editor, scene);

    const frames = await play(page, editor, scene, root);

    // A page error is the editor having failed quietly, and this is the shape
    // that failure takes: the module is handed to the browser with nothing left
    // to resolve, so an import that survived packing mounts nothing at all and
    // photographs as a blank frame nobody would question.
    if (broke.length > 0) throw new Error(`the editor reported: ${broke.join("; ")}`);

    return encodeApng(
      frames.map(({ shot, delay }) => ({ pixels: decodePng(shot).pixels, delay })),
      size,
    );
  } finally {
    await context.close();
    await server.close();
  }
}

/**
 * The editor, once it has read the deck and measured the slide.
 *
 * Waiting for a grip rather than for the frame to load: a grip is drawn from a
 * rectangle read inside the canvas, so one existing proves the deck parsed, the
 * page rendered, and the overlay found it. Nothing a scene does means anything
 * before that.
 */
async function mounted(page) {
  const editor = page.frames().find((frame) => frame.url().includes("/__slidx/"));
  if (editor === undefined) throw new Error("the stage has no editor frame");

  await editor.waitForSelector(".slidx-arrange-grip");
  return editor;
}

/**
 * Opens the slide the scene is about, named by the file it lives in.
 *
 * Through the outline, which is the control an author uses, rather than by
 * reaching into the session: a scene that selected a slide in a way nobody can
 * would be recording a state the editor cannot reach.
 */
async function select(page, editor, scene, root) {
  if (scene.open === undefined) return;

  const at = slidesIn(root).indexOf(scene.open);
  if (at < 0) throw new Error(`${scene.open} is not a slide of the scene's deck`);
  if (at === 0) return;

  await page
    .frameLocator(".editor")
    .locator(`.slidx-outline-row[data-slide="${at}"] .slidx-outline-open`)
    .click();

  // The canvas is an iframe on the deck's own route, so selecting a slide is a
  // navigation. Every coordinate the gesture uses comes from the grips, and
  // those are redrawn once the new page has laid out.
  await editor.waitForFunction(
    (route) => document.querySelector(".slidx-canvas-frame")?.getAttribute("src")?.includes(route),
    `/${at + 1}/`,
  );
  await editor.waitForSelector(".slidx-arrange-grip");
}

/**
 * What a scene can press, before its frame is measured.
 *
 * The same two verbs a scene is given while it is playing, minus everything that
 * captures: nothing here is in the recording, so a scene cannot accidentally
 * photograph the panel it is still opening.
 */
function controls(page, editor) {
  const canvas = page.frameLocator(".editor");

  return {
    click: (selector) => canvas.locator(selector).click(),
    press: (key) => page.keyboard.press(key),
    shown: (selector) => editor.waitForSelector(selector),
  };
}

/** The deck's slides, in the order the outline lists them. */
function slidesIn(root) {
  return (
    readdirSync(join(root, "slides"))
      .filter((name) => name.endsWith(".md"))
      // Explicit collation, so the slide a scene names is the same one on every
      // machine.
      .sort((one, other) => one.localeCompare(other, "en"))
  );
}

/**
 * Sizes the recording to the panels the scene named.
 *
 * The rectangle comes from the editor's own layout rather than from a constant
 * here, so a change to the chrome's grid moves the crop with it instead of
 * cutting a panel in half. The viewport is then set to exactly that frame, which
 * is what makes each screenshot the picture rather than something to crop later.
 */
async function frameOn(page, editor, scene) {
  const box = await editor.evaluate((wanted) => {
    const edges = wanted.map((selector) => {
      const found = document.querySelector(selector);
      if (found === null) throw new Error(`the editor has no ${selector}`);

      const rect = found.getBoundingClientRect();
      return { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom };
    });

    const left = Math.min(...edges.map((edge) => edge.left));
    const top = Math.min(...edges.map((edge) => edge.top));

    return {
      x: Math.round(left),
      y: Math.round(top),
      width: Math.round(Math.max(...edges.map((edge) => edge.right)) - left),
      height: Math.round(Math.max(...edges.map((edge) => edge.bottom)) - top),
    };
  }, scene.frame);

  await page.evaluate((rect) => window.slidxStage.crop(rect), box);

  const size = {
    width: box.width + (scene.file === undefined ? 0 : FILE_WIDTH),
    height: box.height,
  };
  await page.setViewportSize(size);

  return size;
}
