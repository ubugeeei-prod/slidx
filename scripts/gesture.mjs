/**
 * What a scene is handed: gestures, and the frames they leave behind.
 *
 * A scene in [`record-editor`](./record-editor.mjs) says what to do — drag this
 * block into that region, hold for a beat, show the file — and never says when a
 * screenshot is taken or how many there are. That division is the point of this
 * module: adding a gesture to the recordings should be a few lines of a scene,
 * and everything that makes a frame reproducible should be here, where it is
 * written down once.
 *
 * # The two waits, and why neither of them is a duration
 *
 * **The editor is waited on by condition**: the file having been written, the
 * canvas having laid the slide out again. A `waitForTimeout` before a screenshot
 * is a race that resolves differently on a busy machine, and the whole reason
 * these recordings are frames rather than a video is that they must not depend
 * on how fast anything ran.
 *
 * **The picture is waited on until it stops changing.** Dispatching an input
 * event only means it was delivered, and a screenshot returns whatever was last
 * composited — of the outermost of three nested documents, which can be a frame
 * behind one of the inner two. So a frame is taken when two screenshots agree,
 * which is the only definition of "nothing left to arrive" that does not name a
 * number of milliseconds.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";

import { mark } from "./stage.mjs";

/** How a screenshot is taken, everywhere. */
const SHOT = {
  // The chrome transitions a grip's opacity under a pointer, so a frame caught
  // halfway through one lands differently on every machine.
  animations: "disabled",
  caret: "hide",
};

/**
 * Runs a scene against a mounted editor and returns the frames it captured.
 *
 * @returns {Promise<{shot: Buffer, delay: number}[]>}
 */
export async function play(page, editor, scene, root) {
  const frames = [];
  const canvas = page.frameLocator(".editor");
  let shown = "";

  const capture = async (delay) => {
    frames.push({ shot: await still(page), delay });
  };

  /** The deck file as it is on disk, with whatever the last gesture added marked. */
  const file = async () => {
    if (scene.file === undefined) return;

    // A deck file ends with a newline, and drawing the empty line after it would
    // put a row in the picture that nothing in the file corresponds to.
    const text = readFileSync(join(root, "slides", scene.file), "utf8").replace(/\n$/, "");
    const rows = mark(shown, text);
    shown = text;

    await page.evaluate(
      ([path, lines]) => window.slidxStage.file(path, lines),
      [`slides/${scene.file}`, rows],
    );
  };

  /**
   * Moves the mouse, and tells the stage where it now is.
   *
   * Both, always, from one place: a screenshot has no cursor in it, so a frame
   * whose pointer was drawn somewhere other than where the mouse actually went
   * would be a picture of a gesture nobody performed.
   */
  const point = async ({ x, y }) => {
    await page.mouse.move(x, y);
    await page.evaluate(([at, over]) => window.slidxStage.pointer(at, over), [x, y]);
  };

  /**
   * One drag, from a block's grip to the middle of a region.
   *
   * The path is interpolated between two boxes the editor laid out, so the ghost
   * and the guides in every frame are the editor's answer to a position rather
   * than to a moment.
   */
  const drag = async ({ block, into, steps, each }) => {
    const from = middle(await canvas.locator(gripFor(block)).boundingBox());
    const to = middle(await canvas.locator(regionFor(into)).boundingBox());

    await point(from);
    await page.mouse.down();
    await capture(each);

    for (let step = 1; step <= steps; step += 1) {
      const along = step / steps;
      await point({
        x: from.x + (to.x - from.x) * along,
        y: from.y + (to.y - from.y) * along,
      });
      await capture(each);
    }

    await page.mouse.up();

    // The drop is one round trip to the dev server, which splices the file and
    // answers with the deck it read back. Waiting for a grip to say where the
    // block now is waits for all of it: the write, the parse, and the canvas
    // laying the slide out again.
    await editor.waitForFunction(
      (label) =>
        [...document.querySelectorAll(".slidx-arrange-grip")].some((handle) =>
          handle.getAttribute("aria-label")?.endsWith(label),
        ),
      `in ${into}`,
    );
  };

  await file();
  await scene.play({
    drag,
    file,
    hold: capture,
    click: async (selector) => {
      await canvas.locator(selector).click();
    },
    press: (key) => page.keyboard.press(key),
    shown: (selector) => editor.waitForSelector(selector),
  });

  return frames;
}

const gripFor = (block) => `.slidx-arrange-grip[data-block="${block}"]`;
const regionFor = (name) => `.slidx-arrange-region[data-region="${name}"]`;

/**
 * Waits until the browser has drawn what the last step asked for.
 *
 * Twice in every document in the picture, because the first callback runs
 * *before* the frame it belongs to is presented, and because there are three of
 * them: the stage, the editor inside it, and the deck's own page inside that.
 * Each paints on its own, and the deck's is the one behind the canvas frame's
 * rounded corner — where three pixels came out a shade different depending on
 * which had drawn first.
 *
 * Only the documents on screen. A browser never runs an animation callback for
 * one it is not drawing, so waiting on a hidden frame — the history panel's
 * preview, before anybody has opened it — waits forever.
 */
async function painted(page) {
  const twice = () =>
    new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));

  for (const document of page.frames()) {
    if (document !== page.mainFrame() && !(await onScreen(document))) continue;

    // A frame that navigated out from under the wait has nothing to say about
    // what was drawn. The canvas reloads after every edit, so that happens.
    await document.evaluate(twice).catch(() => undefined);
  }
}

/**
 * The picture, once it has stopped changing: two screenshots that agree.
 *
 * Waiting for a paint is not enough on its own, and the reason is where the
 * editor lives. It is a document inside the stage and the deck's page is a
 * document inside that, each drawn on its own; a screenshot is taken of the
 * outermost surface, which can be one frame behind a change that has already
 * been drawn further in. What that produced was a recording where the region
 * boxes of a finished drag were still on screen in some runs and gone in
 * others, and three pixels of an anti-aliased corner that settled a beat after
 * the stage was sized. Either rewrites the whole file for nothing.
 *
 * Nothing about a scene is timed by this: the picture is only ever waited on
 * *after* the editor has answered the condition the step was waiting for, and
 * two frames that agree is the definition of nothing left to arrive.
 */
async function still(page) {
  let last = await page.screenshot(SHOT);

  for (let attempt = 0; attempt < 20; attempt += 1) {
    await painted(page);
    const now = await page.screenshot(SHOT);
    if (now.equals(last)) return now;

    last = now;
  }

  throw new Error("the editor never stopped moving, so no frame of it is reproducible");
}

/** Whether a frame is one the browser is actually drawing. */
async function onScreen(frame) {
  // Lazy outline previews exist as an empty document before the browser elects
  // to load them. An empty document contributes no pixels, and a backgrounded
  // one receives no animation frame to wait for.
  if (frame.url() === "about:blank") return false;

  const element = await frame.frameElement().catch(() => null);

  if (element === null || !(await element.isVisible())) return false;

  // Playwright considers an iframe below a scrollport "visible" because it has
  // layout, while the browser correctly withholds animation frames from a
  // document nobody can see. Waiting on that document would therefore wait
  // forever.
  return element.evaluate((node) => {
    // A lazy preview is passive output. Chromium may keep its document
    // backgrounded even after laying the iframe out, so it contributes pixels
    // to screenshot stability but has no animation frame of its own to await.
    if (node instanceof HTMLIFrameElement && node.loading === "lazy") return false;

    // Active frames can still sit beyond their own document's viewport. Nested
    // frames are checked in the coordinate space of each parent document.
    const view = node.ownerDocument.defaultView;
    if (view === null) return false;

    const box = node.getBoundingClientRect();
    return (
      box.width > 0 &&
      box.height > 0 &&
      box.right > 0 &&
      box.bottom > 0 &&
      box.left < view.innerWidth &&
      box.top < view.innerHeight
    );
  });
}

/** The middle of a box, in the page's coordinates. */
function middle(found) {
  if (found === null) throw new Error("the editor drew nothing to point at");

  return { x: found.x + found.width / 2, y: found.y + found.height / 2 };
}
