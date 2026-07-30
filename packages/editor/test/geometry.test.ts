/**
 * Reading the slide's boxes out of the canvas frame.
 *
 * The rectangles themselves are not what these are about — no test environment
 * lays a page out, and every measurement here comes back zero. What they are
 * about is the part that can silently drift: the attributes. A region's name
 * and a block's index are written onto the page by `slidx_render::region`, and
 * an overlay reading a name this side spells differently would draw handles on
 * nothing at all and report it as an empty slide.
 */

import { afterEach, describe, expect, it } from "vite-plus/test";

import { readGeometry } from "../src/geometry";

/** A frame holding markup shaped the way the renderer emits it. */
function canvas(inner: string, editing = false): HTMLIFrameElement {
  const stage = document.createElement("div");
  if (editing) stage.setAttribute("data-editing", "true");

  const frame = document.createElement("iframe");
  stage.append(frame);
  document.body.append(stage);

  frame.contentDocument!.body.innerHTML = inner;

  return frame;
}

const SPLIT = `
<div class="slidx-slide">
  <div class="slidx-slide-body">
    <div class="slidx-region" data-slidx-region="left">
      <div class="slidx-block" data-slidx-block="0"><h1>One</h1></div>
      <div class="slidx-block" data-slidx-block="1"><p>Second.</p></div>
    </div>
    <div class="slidx-region" data-slidx-region="right">
      <div class="slidx-block" data-slidx-block="2"><pre><code>fn main() {}</code></pre></div>
    </div>
  </div>
</div>`;

afterEach(() => document.body.replaceChildren());

describe("the slide, read out of the canvas", () => {
  it("finds every region the layout declares, in the order it declares them", () => {
    // Including an empty one: the overlay draws the grid an author is dropping
    // into, and a region that only exists once something is in it is a region
    // nobody can aim at.
    const geometry = readGeometry(
      canvas(SPLIT.replace('<div class="slidx-block" data-slidx-block="2">', "<div>")),
    );

    expect(geometry?.regions.map((region) => region.name)).toEqual(["left", "right"]);
    expect(geometry?.regions[1]!.blocks).toEqual([]);
  });

  it("reads each block's index in source order, and which region holds it", () => {
    const geometry = readGeometry(canvas(SPLIT));

    expect(geometry?.blocks.map((block) => block.index)).toEqual([0, 1, 2]);
    expect(geometry?.blocks.map((block) => block.region)).toEqual(["left", "left", "right"]);
    expect(geometry?.regions[0]!.blocks).toEqual([0, 1]);
  });

  it("says there is nothing to measure on a page that is not a slide", () => {
    // Before the frame has loaded, and on a route the deck does not serve.
    expect(readGeometry(canvas("<p>Not a deck.</p>"))).toBeUndefined();
  });

  it("says there is nothing to measure while the Markdown view is up", () => {
    // The canvas hides the frame rather than unloading it, so the content is
    // still there to measure and none of it is on screen. Grips drawn from it
    // would float over the text somebody is typing.
    expect(readGeometry(canvas(SPLIT, true))).toBeUndefined();
  });
});
