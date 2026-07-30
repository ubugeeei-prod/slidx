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
 * A screenshot returns whatever the browser last composited. Dispatching an
 * input event only means the event was delivered — the handler that moves a
 * ghost runs after it, and the frame carrying the result is drawn after that. So
 * every capture waits for a *paint*, in each document that is on screen.
 *
 * And once, before a scene's first frame, it waits for the picture to stop
 * changing at all. Sizing the stage to the panels a scene wants resizes a window
 * holding two nested documents that composite in processes of their own, and the
 * frame straight after that resize is not always the last word.
 *
 * Everything else waits on a condition the editor answers: the file having been
 * written, the canvas having laid the slide out again. A `waitForTimeout` before
 * a screenshot is a race that resolves differently on a busy machine, and the
 * whole reason these recordings are frames rather than a video is that they must
 * not depend on how fast anything ran.
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
    await painted(page);
    frames.push({ shot: await page.screenshot(SHOT), delay });
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
  await stillness(page);
  await scene.play({
    drag,
    file,
    hold: capture,
    click: (selector) => canvas.locator(selector).click(),
    press: (key) => page.keyboard.press(key),
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
 * Waits until the picture has stopped changing at all.
 *
 * Once, before a scene's first frame, because the settling after the stage is
 * sized is the one thing no condition in the editor answers: three pixels of an
 * anti-aliased corner arrived a beat late, which is a hundred kilobytes of diff
 * on a regeneration that changed nothing.
 */
async function stillness(page) {
  let last = await page.screenshot(SHOT);

  for (let attempt = 0; attempt < 20; attempt += 1) {
    await painted(page);
    const now = await page.screenshot(SHOT);
    if (now.equals(last)) return;

    last = now;
  }

  throw new Error("the editor never stopped moving, so no frame of it is reproducible");
}

/** Whether a frame is one the browser is actually drawing. */
async function onScreen(frame) {
  const element = await frame.frameElement().catch(() => null);

  return element !== null && (await element.isVisible());
}

/** The middle of a box, in the page's coordinates. */
function middle(found) {
  if (found === null) throw new Error("the editor drew nothing to point at");

  return { x: found.x + found.width / 2, y: found.y + found.height / 2 };
}
