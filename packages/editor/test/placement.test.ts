/**
 * Where a drop lands, and what the editor can say about it beforehand.
 *
 * All of it is arithmetic over rectangles, so none of it needs a browser that
 * lays anything out — which is the reason the geometry is a value rather than
 * something the placement code reads for itself.
 *
 * The slide these are written against is a `split`: two equal columns, `left`
 * holding two blocks and `right` holding one. Equal columns matter, because
 * whether the two regions are the same width is what decides which of these
 * questions has an exact answer.
 */

import { describe, expect, it } from "vite-plus/test";

import type { BlockBox, RegionBox, SlideGeometry } from "../src/geometry";
import { arrival, guides, insertion, landing, nudge } from "../src/placement";

function block(index: number, region: string, top: number, over: Partial<BlockBox> = {}): BlockBox {
  const left = region === "left" ? 100 : 500;

  return {
    index,
    region,
    rect: { left, top, width: 400, height: 80 },
    needsWidth: 0,
    width: "full",
    ...over,
  };
}

function region(name: string, left: number, blocks: number[], over: Partial<RegionBox> = {}) {
  return {
    name,
    rect: { left, top: 100, width: 400, height: 600 },
    blocks,
    contentHeight: 200,
    gap: 20,
    ...over,
  };
}

/** A `split` slide: `left` holds blocks 0 and 1, `right` holds block 2. */
function split(over: Partial<SlideGeometry> = {}): SlideGeometry {
  return {
    slide: { left: 80, top: 80, width: 840, height: 640 },
    safe: { left: 100, top: 100, width: 800, height: 600 },
    regions: [region("left", 100, [0, 1]), region("right", 500, [2])],
    blocks: [block(0, "left", 100), block(1, "left", 200), block(2, "right", 100)],
    ...over,
  };
}

describe("where a drop lands", () => {
  it("puts a block in the region the pointer is over", () => {
    const found = landing(split(), 0, 600, 400);

    expect(found?.region).toBe("right");
  });

  it("lands nothing when the pointer has wandered off the slide", () => {
    // A nearest-region fallback would place a block somewhere the author never
    // pointed at, on the gesture they were using to change their mind.
    expect(landing(split(), 0, 2000, 400)).toBeUndefined();
  });

  it("drops a block after the one whose lower half the pointer is in", () => {
    // Block 1 spans 200 to 280, so its midpoint is 240.
    expect(landing(split(), 0, 200, 230)?.at).toBe(0);
    expect(landing(split(), 0, 200, 260)?.at).toBe(1);
  });

  it("counts the position an operation names in source order, the block lifted out", () => {
    // Dropping block 0 above block 2 in the right column: with block 0 lifted,
    // the remaining source order is [1, 2], and block 2 sits at position 1.
    const found = landing(split(), 0, 600, 120);

    expect(found).toMatchObject({ region: "right", at: 0, to: 1 });
  });

  it("sends a block dropped on an empty region to where that region's content would start", () => {
    // There is no block in it to count from, so the position is the one the
    // region's own place in the layout implies.
    const empty = split({
      regions: [region("left", 100, [0, 1]), region("right", 500, [])],
      blocks: [block(0, "left", 100), block(1, "left", 200)],
    });
    const found = landing(empty, 0, 600, 400);

    expect(found).toMatchObject({ region: "right", at: 0, to: 1 });
  });

  it("snaps the ghost to the slot rather than leaving it under the cursor", () => {
    // The file will say a region and a position, so that is what the gesture
    // shows while it is still a gesture.
    const found = landing(split(), 2, 150, 120);

    expect(found?.rect.left).toBe(100);
    expect(found?.rect.width).toBe(400);
  });
});

describe("where new media is inserted", () => {
  it("uses the region and source position under the pointer", () => {
    expect(insertion(split(), 600, 120)).toMatchObject({ region: "right", at: 0, to: 2 });
    expect(insertion(split(), 200, 260)).toMatchObject({ region: "left", at: 2, to: 2 });
  });

  it("puts media in an empty region before the next region's first block", () => {
    const geometry = split({
      regions: [region("left", 100, []), region("right", 500, [0, 1, 2])],
      blocks: [block(0, "right", 100), block(1, "right", 200), block(2, "right", 300)],
    });

    expect(insertion(geometry, 200, 300)).toMatchObject({ region: "left", at: 0, to: 0 });
  });

  it("appends in an empty final region and refuses outside the slide", () => {
    const geometry = split({
      regions: [region("left", 100, [0, 1, 2]), region("right", 500, [])],
      blocks: [block(0, "left", 100), block(1, "left", 200), block(2, "left", 300)],
    });

    expect(insertion(geometry, 600, 300)).toMatchObject({ region: "right", at: 0, to: 3 });
    expect(insertion(geometry, 2_000, 300)).toBeUndefined();
  });
});

describe("the keyboard equivalent", () => {
  it("moves a block down through its own region", () => {
    expect(nudge(split(), 0, "down")).toMatchObject({ region: "left", at: 1, to: 1 });
  });

  it("moves a block into the region beside it, after what is already there", () => {
    expect(nudge(split(), 0, "after")).toMatchObject({ region: "right", at: 1, to: 2 });
  });

  it("does nothing past the ends, rather than wrapping round", () => {
    expect(nudge(split(), 0, "up")).toBeUndefined();
    expect(nudge(split(), 0, "before")).toBeUndefined();
    expect(nudge(split(), 2, "after")).toBeUndefined();
  });

  it("says nothing about a block the slide does not have", () => {
    // The overlay is drawn from a page rendered a keystroke ago.
    expect(nudge(split(), 9, "down")).toBeUndefined();
  });
});

describe("the guides", () => {
  it("draws nothing until an edge meets something", () => {
    expect(guides(split(), 0, { left: 231, top: 337, width: 55, height: 40 })).toEqual([]);
  });

  it("names the region boundary a block has lined up with", () => {
    const drawn = guides(split(), 0, { left: 500, top: 400, width: 400, height: 40 });

    expect(drawn.some((guide) => guide.kind === "region" && guide.at === 500)).toBe(true);
  });

  it("names the safe area, which is the edge the room takes", () => {
    const drawn = guides(split(), 0, { left: 100, top: 400, width: 400, height: 40 });

    expect(drawn.some((guide) => guide.kind === "safe" && guide.at === 100)).toBe(true);
  });

  it("names another block's edge, and never the dragged block's own", () => {
    const drawn = guides(split(), 0, { left: 500, top: 200, width: 400, height: 80 });

    expect(drawn.some((guide) => guide.kind === "block")).toBe(true);
    // Block 0 sits at 100 and is the one being dragged, so its own top is not
    // something to line up with.
    expect(drawn.some((guide) => guide.kind === "block" && guide.at === 100)).toBe(false);
  });
});

describe("what can be known before the block lands", () => {
  it("measures a code block heading for a column too narrow to hold it", () => {
    // A `pre` scrolls its own overflow, so the width it needs is the width it
    // needs whatever box it is put in. This is arithmetic, not a guess.
    const narrow = split({
      regions: [
        region("left", 100, [0, 1]),
        region("right", 500, [2], { rect: { left: 500, top: 100, width: 200, height: 600 } }),
      ],
      blocks: [
        block(0, "left", 100, { needsWidth: 380 }),
        block(1, "left", 200),
        block(2, "right", 100),
      ],
    });
    const target = landing(narrow, 0, 600, 400)!;

    expect(arrival(narrow, 0, target, 3)).toEqual([
      { slideIndex: 3, stop: 0, overHeight: 0, overWidth: 0.9, region: "right" },
    ]);
  });

  it("says nothing about a paragraph, because nothing here knows where its lines will break", () => {
    // The trade the linter already makes everywhere else: silence beats a
    // number that looks measured and is not.
    const geometry = split({
      regions: [
        region("left", 100, [0, 1]),
        region("right", 500, [2], { rect: { left: 500, top: 100, width: 200, height: 600 } }),
      ],
    });

    expect(arrival(geometry, 0, landing(geometry, 0, 600, 400)!, 0)).toEqual([]);
  });

  it("knows the height a block will add to a region exactly as wide as the one it left", () => {
    // Equal columns lay the same block out the same way, so nothing rewraps and
    // the height it arrives with is the height it already has.
    const full = split({
      regions: [
        region("left", 100, [0, 1]),
        region("right", 500, [2], {
          contentHeight: 560,
          rect: { left: 500, top: 100, width: 400, height: 600 },
        }),
      ],
    });
    const target = landing(full, 0, 600, 400)!;

    const [measured] = arrival(full, 0, target, 0);
    expect(measured?.region).toBe("right");
    expect(measured?.overHeight).toBeCloseTo((560 + 80 + 20 - 600) / 600, 6);
  });

  it("says nothing about a block that is not going anywhere", () => {
    const geometry = split();

    expect(arrival(geometry, 0, landing(geometry, 0, 200, 260)!, 0)).toEqual([]);
  });
});
