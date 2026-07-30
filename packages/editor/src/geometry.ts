/**
 * The slide's boxes, read out of the canvas and given to the editor.
 *
 * The canvas is an iframe on the deck's own route, so the only true answer to
 * "where is that block" is inside it, laid out by the same CSS a projector
 * will use. This reads those rectangles and translates them into the editor's
 * own coordinates. Nothing is written back through the boundary: the overlay is
 * drawn *outside* the frame, because the moment the editor puts an element into
 * the deck page the preview stops being the page the build emits.
 *
 * # What can be known before a block moves, and what cannot
 *
 * A region is a grid track and a block is a box, so where each one is is a
 * measurement. Whether a block will still *fit* after it lands somewhere else
 * depends on where its lines break, and line breaking depends on a layout that
 * has not happened. Two things escape that, and only two:
 *
 * **A code block does not reflow.** `pre` scrolls its own overflow, so its
 * content width is the width it needs whatever box it is put in. A region
 * narrower than that will clip it, and that is arithmetic rather than a guess.
 *
 * **Regions of equal width lay out identically.** Moving a block between the
 * halves of a `split` or the quarters of a `quad` changes nothing about how its
 * lines break, so the height it will add to the region it lands in is the
 * height it already has.
 *
 * Everywhere else this reports nothing, which is the same trade
 * `slidx_lint::rules::overflow` makes: a linter's failure mode is not missing a
 * problem, it is being wrong often enough that somebody switches it off.
 */

/**
 * The renderer's own attributes, defined in `slidx_render::region`.
 *
 * Exported so the pair can be checked against a page the pipeline really
 * emitted rather than trusted. A name spelled differently on this side would
 * report every slide as empty and draw no handles at all — which looks exactly
 * like a canvas that has not finished loading.
 */
export const BLOCK_ATTRIBUTE = "data-slidx-block";
export const REGION_ATTRIBUTE = "data-slidx-region";

/**
 * The share of its region a block takes, from `slidx_theme::layout::width`.
 *
 * Absent on a block that takes the whole region, because the default is written
 * by saying nothing — in the file and therefore on the page. A handle reads it to
 * know where it starts.
 */
export const WIDTH_ATTRIBUTE = "data-slidx-width";

/** A box in the editor's own coordinates. */
export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** One block of the slide, as it is currently drawn. */
export interface BlockBox {
  /** Position in source order, which is what an operation names. */
  index: number;
  region: string;
  rect: Rect;
  /**
   * The narrowest box this block's content can be poured into, or zero.
   *
   * Zero for anything that reflows, which is most blocks and every paragraph.
   * A region narrower than a non-zero figure here will clip the block.
   */
  needsWidth: number;
  /**
   * The share of its region the block says it takes, or `"full"`.
   *
   * The word the file uses, not a number: a handle that read a measured width
   * would round its way to a different share every time the canvas re-laid out.
   */
  width: string;
}

/** One region of the layout, and what is in it. */
export interface RegionBox {
  name: string;
  rect: Rect;
  /** The blocks in it, in source order. */
  blocks: number[];
  /** The height its content occupies, which can exceed the box. */
  contentHeight: number;
  /** The gap the layout puts between two blocks in this region. */
  gap: number;
}

/** Everything the overlay draws and everything a drop is decided from. */
export interface SlideGeometry {
  /** The design box: the whole slide, edges included. */
  slide: Rect;
  /** The box the theme's padding leaves, which is the safe area. */
  safe: Rect;
  regions: RegionBox[];
  blocks: BlockBox[];
}

/**
 * Reads the slide in the canvas frame.
 *
 * `undefined` while there is nothing to measure — before the frame has loaded,
 * on a route that is not a slide, and while the author has the Markdown view up
 * instead. A caller cannot tell an empty slide from an absent one any other
 * way, and an overlay over the second would put handles on text somebody is
 * typing.
 */
export function readGeometry(frame: HTMLIFrameElement): SlideGeometry | undefined {
  // The canvas hides the frame rather than unloading it, so its content is
  // still there to measure and none of it is on screen.
  if (frame.closest("[data-editing]")?.getAttribute("data-editing") === "true") return undefined;

  const page = frame.contentDocument;
  const slide = page?.querySelector(".slidx-slide");
  const body = page?.querySelector(".slidx-slide-body");
  if (!page || !slide || !body) return undefined;

  const frameRect = frame.getBoundingClientRect();
  // The frame's border is outside its content box, and every rectangle inside
  // the page is measured from the content box.
  const offset = { x: frameRect.left + frame.clientLeft, y: frameRect.top + frame.clientTop };
  const at = (element: Element) => shift(element.getBoundingClientRect(), offset);

  const view = page.defaultView;
  const blocks: BlockBox[] = [];
  const regions: RegionBox[] = [];

  for (const element of page.querySelectorAll(`[${REGION_ATTRIBUTE}]`)) {
    const name = element.getAttribute(REGION_ATTRIBUTE);
    if (name === null) continue;

    const inside: number[] = [];
    for (const box of element.querySelectorAll(`[${BLOCK_ATTRIBUTE}]`)) {
      const index = Number(box.getAttribute(BLOCK_ATTRIBUTE));
      if (!Number.isInteger(index)) continue;

      inside.push(index);
      blocks.push({
        index,
        region: name,
        rect: at(box),
        needsWidth: needsWidth(box),
        width: box.getAttribute(WIDTH_ATTRIBUTE) ?? "full",
      });
    }

    regions.push({
      name,
      rect: at(element),
      blocks: inside,
      contentHeight: element.scrollHeight,
      gap: view === null ? 0 : Number.parseFloat(view.getComputedStyle(element).rowGap) || 0,
    });
  }

  blocks.sort((a, b) => a.index - b.index);

  return { slide: at(slide), safe: at(body), regions, blocks };
}

/**
 * The width a block cannot be squeezed below.
 *
 * Only `pre` is counted, and that is the whole rule rather than a first pass.
 * The shell gives a code block `overflow: auto`, so its content width is the
 * width it needs; everything else the shell draws is told to shrink —
 * `img { max-width: 100% }`, `table { width: 100% }`, and prose that rewraps —
 * so their current width says nothing about the narrowest box they would fit.
 * Reporting one of those would mean warning that a paragraph does not fit a
 * column it fits perfectly well, which is how a rule stops being believed.
 */
function needsWidth(block: Element): number {
  let widest = 0;

  for (const code of block.querySelectorAll("pre")) {
    widest = Math.max(widest, code.scrollWidth);
  }

  return widest;
}

function shift(rect: DOMRect, offset: { x: number; y: number }): Rect {
  return {
    left: rect.left + offset.x,
    top: rect.top + offset.y,
    width: rect.width,
    height: rect.height,
  };
}

/** Whether a point is inside a box. */
export function contains(rect: Rect, x: number, y: number): boolean {
  return (
    x >= rect.left && x <= rect.left + rect.width && y >= rect.top && y <= rect.top + rect.height
  );
}
