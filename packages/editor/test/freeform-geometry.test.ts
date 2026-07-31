/** The deterministic coordinate rules behind freeform canvas gestures. */

import { describe, expect, it } from "vite-plus/test";

import { insetOf, manipulateFrame } from "../src/freeform-geometry";
import type { BlockBox, Rect, SlideGeometry } from "../src/geometry";

const SAFE: Rect = { left: 100, top: 100, width: 800, height: 400 };
const SELECTED: Rect = { left: 200, top: 150, width: 300, height: 100 };

function block(index: number, rect: Rect): BlockBox {
  return { index, region: "body", rect, needsWidth: 0, width: "full" };
}

function geometry(): SlideGeometry {
  return {
    slide: { left: 80, top: 80, width: 840, height: 460 },
    safe: SAFE,
    regions: [
      {
        name: "body",
        rect: SAFE,
        blocks: [0, 1],
        contentHeight: 200,
        gap: 20,
      },
    ],
    blocks: [block(0, SELECTED), block(1, { left: 650, top: 350, width: 200, height: 100 })],
  };
}

describe("freeform geometry", () => {
  it("stores a stable four-side value in safe-area percentages", () => {
    expect(insetOf(SELECTED, SAFE)).toBe("12.5% 50% 62.5% 12.5%");
  });

  it("clamps a move to the safe area", () => {
    const moved = manipulateFrame(geometry(), 0, SELECTED, "move", -500, -500);

    expect(moved.rect).toEqual({ left: 100, top: 100, width: 300, height: 100 });
  });

  it("keeps corner resizing inside the safe area and above the minimum frame", () => {
    const smallest = manipulateFrame(geometry(), 0, SELECTED, "nw", 999, 999);
    const largest = manipulateFrame(geometry(), 0, SELECTED, "se", 999, 999);

    expect(smallest.rect).toEqual({ left: 472, top: 222, width: 28, height: 28 });
    expect(largest.rect).toEqual({ left: 200, top: 150, width: 700, height: 350 });
  });

  it("snaps edges and centres to another block and names the guide", () => {
    const known = geometry();
    known.blocks[1]!.rect = { left: 700, top: 350, width: 150, height: 100 };
    const moved = manipulateFrame(known, 0, SELECTED, "move", 348, 199);

    expect(moved.rect).toEqual({ left: 550, top: 350, width: 300, height: 100 });
    expect(moved.guides).toEqual([
      { kind: "block", axis: "x", at: 700 },
      { kind: "block", axis: "y", at: 350 },
    ]);
  });
});
