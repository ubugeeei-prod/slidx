/**
 * What a drop means: where the block lands, and what it costs when it gets
 * there.
 *
 * Pure arithmetic over the boxes [`geometry`](./geometry) read, so the whole
 * gesture can be driven in a test without a browser that lays anything out.
 *
 * # Why a landing is a region and a slot
 *
 * The alternative is four floats in the file, and the reason slidx does not
 * take it is written down in `slidx_theme::layout`: nobody can review a
 * rectangle, it means something else at a different aspect ratio, and no rule
 * can reason about it. So a drag resolves to a region and a position in that
 * region's list, and the ghost snaps there while the pointer is still moving —
 * which is not a compromise on direct manipulation but the point of it. What
 * you see under the cursor is exactly what the file will say.
 */

import type { BlockBox, Rect, RegionBox, SlideGeometry } from "./geometry";
import { contains } from "./geometry";

/** Where a dragged block would come to rest. */
export interface Landing {
  region: string;
  /** Position among that region's blocks, this one already lifted out. */
  at: number;
  /** The position an operation names, counted in source order after the lift. */
  to: number;
  /** The box the block would occupy, for the ghost to snap to. */
  rect: Rect;
}

/** A line worth drawing because the block being dragged lines up with it. */
export interface Guide {
  /** What the line belongs to, which is what decides how it is drawn. */
  kind: "region" | "safe" | "block";
  axis: "x" | "y";
  at: number;
}

/**
 * One measurement of a stop, as `slidx_lint::Measurement` expects it.
 *
 * Shares of the box rather than pixels, because the linter is comparing
 * against a box whose size it does not know.
 */
export interface Measurement {
  slideIndex: number;
  stop: number;
  overHeight: number;
  overWidth: number;
  region?: string;
}

/** How close two edges have to be before the alignment is worth saying. */
const SNAP = 6;

/** Which way a keyboard nudge sends a block. */
export type Nudge = "up" | "down" | "before" | "after";

/**
 * Where the block would land if the pointer were let go here.
 *
 * `undefined` outside every region, so a drag that wanders off the slide drops
 * nothing. A nearest-region fallback would put a block somewhere the author
 * never pointed at, on the gesture they used to change their mind.
 */
export function landing(
  geometry: SlideGeometry,
  index: number,
  x: number,
  y: number,
): Landing | undefined {
  const region = geometry.regions.find((candidate) => contains(candidate.rect, x, y));
  if (region === undefined) return undefined;

  const others = region.blocks.filter((block) => block !== index);
  const boxes = others.map((block) => geometry.blocks.find((box) => box.index === block));

  // The midpoint rather than the top edge: dropping onto the lower half of a
  // block means after it, which is what every list with a drag has taught
  // everybody to expect.
  const at = boxes.filter((box) => box !== undefined && middle(box.rect) < y).length;

  return { region: region.name, at, to: sourcePosition(geometry, region, index, at), rect: slot(geometry, region, others, at) };
}

/**
 * Where the same block goes when it is moved by the keyboard.
 *
 * Every gesture on the canvas has one of these, because a deck that can only
 * be arranged with a pointer is a deck half the people who write one cannot
 * arrange at all.
 */
export function nudge(geometry: SlideGeometry, index: number, direction: Nudge): Landing | undefined {
  const box = geometry.blocks.find((candidate) => candidate.index === index);
  const from = geometry.regions.findIndex((region) => region.name === box?.region);
  if (box === undefined || from < 0) return undefined;

  if (direction === "up" || direction === "down") {
    const region = geometry.regions[from]!;
    const others = region.blocks.filter((block) => block !== index);
    const was = region.blocks.indexOf(index);
    const at = direction === "up" ? was - 1 : was + 1;
    if (at < 0 || at > others.length) return undefined;

    return {
      region: region.name,
      at,
      to: sourcePosition(geometry, region, index, at),
      rect: slot(geometry, region, others, at),
    };
  }

  const to = direction === "before" ? from - 1 : from + 1;
  const region = geometry.regions[to];
  if (region === undefined) return undefined;

  // Into the end of the next region, which is where a block dropped on an
  // empty region goes too.
  const others = region.blocks.filter((block) => block !== index);

  return {
    region: region.name,
    at: others.length,
    to: sourcePosition(geometry, region, index, others.length),
    rect: slot(geometry, region, others, others.length),
  };
}

/**
 * The lines the block being dragged currently agrees with.
 *
 * Region boundaries, the safe area, and the edges of the blocks it is landing
 * among — the three things on a slide that an author is ever trying to line
 * something up with. Nothing is drawn until an edge actually meets one, so a
 * drag across a busy slide does not turn the canvas into graph paper.
 */
export function guides(geometry: SlideGeometry, index: number, rect: Rect): Guide[] {
  const found: Guide[] = [];
  const add = (kind: Guide["kind"], axis: Guide["axis"], edge: number, against: number[]) => {
    for (const candidate of against) {
      if (Math.abs(edge - candidate) <= SNAP) found.push({ kind, axis, at: candidate });
    }
  };

  const verticals = (box: Rect) => [box.left, box.left + box.width];
  const horizontals = (box: Rect) => [box.top, box.top + box.height];

  for (const edge of verticals(rect)) {
    add("safe", "x", edge, verticals(geometry.safe));
    for (const region of geometry.regions) add("region", "x", edge, verticals(region.rect));
  }
  for (const edge of horizontals(rect)) {
    add("safe", "y", edge, horizontals(geometry.safe));
    for (const region of geometry.regions) add("region", "y", edge, horizontals(region.rect));
  }

  for (const block of geometry.blocks) {
    if (block.index === index) continue;

    for (const edge of verticals(rect)) add("block", "x", edge, verticals(block.rect));
    for (const edge of horizontals(rect)) add("block", "y", edge, horizontals(block.rect));
  }

  return unique(found);
}

/**
 * What the linter can be told about a landing before it happens.
 *
 * Two findings are reachable this way and no others, for the reason
 * [`geometry`](./geometry) states: a code block cannot be squeezed, and a
 * region the same width as the one a block came from will lay it out
 * identically. Everywhere else this returns nothing rather than a number that
 * looks measured and is not.
 *
 * The numbers go to the same Rust rule the build runs, so the sentence an
 * author reads mid-drag is the sentence they would have read from a build.
 */
export function arrival(
  geometry: SlideGeometry,
  index: number,
  target: Landing,
  slideIndex: number,
): Measurement[] {
  const block = geometry.blocks.find((candidate) => candidate.index === index);
  const region = geometry.regions.find((candidate) => candidate.name === target.region);
  const from = geometry.regions.find((candidate) => candidate.name === block?.region);
  if (block === undefined || region === undefined || from === undefined) return [];
  if (region.name === from.name) return [];

  const overWidth =
    block.needsWidth > region.rect.width && region.rect.width > 0
      ? (block.needsWidth - region.rect.width) / region.rect.width
      : 0;

  // Only where the two regions are the same width. Anywhere else the block
  // rewraps on arrival and its height is not a thing this side can know.
  const alike = Math.abs(region.rect.width - from.rect.width) <= 1;
  const height = region.contentHeight + block.rect.height + region.gap;
  const overHeight =
    alike && region.rect.height > 0
      ? Math.max(0, (height - region.rect.height) / region.rect.height)
      : 0;

  if (overWidth === 0 && overHeight === 0) return [];

  return [{ slideIndex, stop: 0, overHeight, overWidth, region: region.name }];
}

/** The block position an operation names, counted after the block is lifted. */
function sourcePosition(
  geometry: SlideGeometry,
  region: RegionBox,
  index: number,
  at: number,
): number {
  const others = geometry.blocks
    .map((block) => block.index)
    .filter((block) => block !== index);
  const inside = region.blocks.filter((block) => block !== index);

  const before = inside[at];
  if (before !== undefined) return others.indexOf(before);

  const last = inside[inside.length - 1];
  if (last !== undefined) return others.indexOf(last) + 1;

  // An empty region has no block to count from, so the block goes where that
  // region's content would start: in front of whatever the next region holds.
  const after = geometry.regions.slice(geometry.regions.indexOf(region) + 1);
  for (const next of after) {
    const first = next.blocks.find((block) => others.includes(block));
    if (first !== undefined) return others.indexOf(first);
  }

  return others.length;
}

/** The box a block would occupy once it lands, for the ghost to snap into. */
function slot(geometry: SlideGeometry, region: RegionBox, others: number[], at: number): Rect {
  const boxes = others
    .map((block) => geometry.blocks.find((candidate) => candidate.index === block))
    .filter((box): box is BlockBox => box !== undefined);

  const before = boxes[at];
  const previous = boxes[at - 1];

  const top =
    before !== undefined
      ? before.rect.top
      : previous !== undefined
        ? previous.rect.top + previous.rect.height + region.gap
        : region.rect.top + region.rect.height / 2;

  return { left: region.rect.left, top, width: region.rect.width, height: 0 };
}

function middle(rect: Rect): number {
  return rect.top + rect.height / 2;
}

function unique(guides: Guide[]): Guide[] {
  const seen = new Set<string>();

  return guides.filter((guide) => {
    const key = `${guide.kind}:${guide.axis}:${Math.round(guide.at)}`;
    if (seen.has(key)) return false;

    seen.add(key);
    return true;
  });
}
