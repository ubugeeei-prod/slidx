/**
 * What a resize means: a share of the region, never a length.
 *
 * The arithmetic behind the handle, so the whole gesture can be driven in a test
 * without a browser that lays anything out — the same split
 * [`placement`](./placement) makes for a drag.
 *
 * # Why a share and not a width
 *
 * The reasoning is written down where the vocabulary is,
 * `slidx_theme::layout::width`, and it comes to this: four floats in a file are
 * unreviewable, mean something else at another aspect ratio, and are opaque to
 * every rule. A share is none of those. `half` in a diff says what happened to
 * the slide; a half is a half at 4:3; and because a region is already a share of
 * the slide, a share of a region closes the chain arithmetically — which is why
 * [`narrowing`] below can ask the linter about a box the block has not been put
 * in yet.
 *
 * So the handle does not write what the pointer is at. It snaps to the nearest
 * share while the pointer is still moving, and what you see under the cursor is
 * what the file will say.
 */

import type { Measurement } from "./placement";
import type { BlockBox, RegionBox } from "./geometry";

/**
 * The shares the theme names, widest first.
 *
 * Mirrors `BlockWidth::ALL`. Drift is safe in both directions and that is why
 * the list is allowed to be here: a share only Rust knows is a snap target
 * nobody can reach, and one only this side knows is written into the file and
 * reported by `layout/no-such-width` on the next build. Neither corrupts
 * anything, and `the_names_are_the_ones_the_editor_offers` pins the Rust half so
 * a change arrives in review as a diff.
 */
export const WIDTHS = [
  { name: "full", share: 1 },
  { name: "three-quarters", share: 0.75 },
  { name: "two-thirds", share: 2 / 3 },
  { name: "half", share: 0.5 },
  { name: "third", share: 1 / 3 },
  { name: "quarter", share: 0.25 },
] as const;

/** One share of a region, as the file names it. */
export type BlockWidth = (typeof WIDTHS)[number]["name"];

/** The share a block currently takes, read off the page. */
export function widthOf(box: BlockBox): BlockWidth {
  return WIDTHS.find((width) => width.name === box.width)?.name ?? "full";
}

/** How much of the region a share is. */
export function shareOf(name: BlockWidth): number {
  return WIDTHS.find((width) => width.name === name)?.share ?? 1;
}

/**
 * The share a pointer at `x` is asking for.
 *
 * Measured from the region's centre and doubled, because a narrowed block is
 * centred in its region: dragging the trailing edge out by one step moves the
 * leading edge in by the same one, so the handle follows the hand rather than
 * running ahead of it.
 */
export function shareAt(region: RegionBox, x: number): BlockWidth {
  if (region.rect.width <= 0) return "full";

  const wanted = ((x - (region.rect.left + region.rect.width / 2)) * 2) / region.rect.width;
  let nearest: (typeof WIDTHS)[number] = WIDTHS[0]!;

  for (const width of WIDTHS) {
    if (Math.abs(width.share - wanted) < Math.abs(nearest.share - wanted)) nearest = width;
  }

  return nearest.name;
}

/** Which way a keyboard step sends a handle. */
export type Step = "wider" | "narrower";

/**
 * The share one step from here, or nothing at either end.
 *
 * Every gesture on the canvas has a keyboard form, for the reason the grips
 * have one: a deck that can only be laid out with a pointer is a deck half the
 * people who write one cannot lay out at all.
 */
export function stepped(from: BlockWidth, step: Step): BlockWidth | undefined {
  const at = WIDTHS.findIndex((width) => width.name === from);
  const to = step === "wider" ? at - 1 : at + 1;

  return WIDTHS[to]?.name;
}

/** The box a block would occupy at a share, for the ghost to snap into. */
export function boxAt(region: RegionBox, block: BlockBox, name: BlockWidth) {
  const width = region.rect.width * shareOf(name);
  const centre = region.rect.left + region.rect.width / 2;

  return { left: centre - width / 2, top: block.rect.top, width, height: block.rect.height };
}

/**
 * What the linter can be told about a share before the block has it.
 *
 * One finding is reachable and no others, for the reason
 * [`geometry`](./geometry) states: a code block does not reflow, so a box
 * narrower than its content will clip it and that is arithmetic. Whether prose
 * will still fit depends on where its lines break, which is a layout that has
 * not happened — so nothing is reported about it rather than a number that looks
 * measured and is not.
 *
 * This is the part a pixel width would have made impossible. The box is
 * `share × region`, the region is a known share of the slide, and the numbers go
 * to the same Rust rule the build runs — so the sentence an author reads while
 * dragging is the sentence a build would have used.
 */
export function narrowing(
  region: RegionBox,
  block: BlockBox,
  name: BlockWidth,
  slideIndex: number,
): Measurement[] {
  const box = region.rect.width * shareOf(name);
  if (box <= 0 || block.needsWidth <= box) return [];

  return [
    {
      slideIndex,
      stop: 0,
      overHeight: 0,
      overWidth: (block.needsWidth - box) / box,
      region: region.name,
    },
  ];
}
