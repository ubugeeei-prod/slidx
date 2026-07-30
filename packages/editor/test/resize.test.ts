/**
 * Making a block take less of its region.
 *
 * The gesture is asserted as the operation it produces and the *word* in it,
 * because the whole design of this feature is that a resize writes a share the
 * theme names rather than a length. A test that accepted a number would pass on
 * the change that ruins the file.
 *
 * The geometry is injected, so these run without a browser laying anything out —
 * the same way the arrange tests do.
 */

import { afterEach, describe, expect, it } from "vite-plus/test";

import type { BlockBox, RegionBox, SlideGeometry } from "../src/geometry";
import type { EditOp } from "../src/operations";
import { createResize } from "../src/resize";
import type { EditorState } from "../src/session";
import { narrowing, shareAt, stepped, WIDTHS } from "../src/widths";

function stateOf(slide = 0): EditorState {
  return {
    source: "",
    spans: [],
    slides: [],
    diagnostics: [],
    selection: { slide },
    canUndo: false,
    canRedo: false,
  };
}

function region(over: Partial<RegionBox> = {}): RegionBox {
  return {
    name: "body",
    rect: { left: 0, top: 0, width: 800, height: 400 },
    blocks: [0],
    contentHeight: 100,
    gap: 10,
    ...over,
  };
}

function block(over: Partial<BlockBox> = {}): BlockBox {
  return {
    index: 0,
    region: "body",
    rect: { left: 0, top: 0, width: 800, height: 100 },
    needsWidth: 0,
    width: "full",
    ...over,
  };
}

function geometryOf(blocks: BlockBox[], regions: RegionBox[]): SlideGeometry {
  return {
    slide: { left: 0, top: 0, width: 800, height: 450 },
    safe: { left: 0, top: 0, width: 800, height: 450 },
    regions,
    blocks,
  };
}

/** The overlay, mounted with a slide whose boxes are known. */
function mounted(geometry: SlideGeometry) {
  const ops: EditOp[] = [];
  const surface = createResize(
    { run: (op) => ops.push(op), foresee: () => {} },
    {
      geometry: () => geometry,
    },
  );

  document.body.append(surface.root);
  surface.render(stateOf());

  return { ops, root: surface.root };
}

function drag(root: Element, index: number, x: number): void {
  const handle = root.querySelector(`.slidx-resize-grip[data-block="${index}"]`)!;

  handle.dispatchEvent(new window.PointerEvent("pointerdown", { clientX: x, cancelable: true }));
  handle.dispatchEvent(new window.PointerEvent("pointermove", { clientX: x }));
  handle.dispatchEvent(new window.PointerEvent("pointerup"));
}

afterEach(() => document.body.replaceChildren());

describe("resizing a block", () => {
  it("writes the share the theme names rather than the width the pointer was at", () => {
    // 200 either side of the region's centre is 400 of 800, and what the file
    // gets is the word for it rather than the four hundred.
    const { ops, root } = mounted(geometryOf([block()], [region()]));

    drag(root, 0, 600);

    expect(ops).toEqual([{ op: "setBlockWidth", slide: 0, block: 0, width: "half" }]);
  });

  it("writes `full` when the handle goes back to the region's edge", () => {
    // Which `slidx_edit` writes by removing the property, so a resize out and
    // back is byte-identical. That rule is in Rust; what matters here is that
    // the gesture can still ask for it.
    const { ops, root } = mounted(geometryOf([block({ width: "half" })], [region()]));

    drag(root, 0, 800);

    expect(ops).toEqual([{ op: "setBlockWidth", slide: 0, block: 0, width: "full" }]);
  });

  it("steps one share at a time from the keyboard", () => {
    // A deck that can only be laid out with a pointer is a deck half the people
    // who write one cannot lay out at all.
    const { ops, root } = mounted(geometryOf([block({ width: "half" })], [region()]));
    const handle = root.querySelector(".slidx-resize-grip")!;

    handle.dispatchEvent(new window.KeyboardEvent("keydown", { key: "ArrowLeft" }));
    handle.dispatchEvent(new window.KeyboardEvent("keydown", { key: "ArrowRight" }));

    expect(ops.map((op) => "width" in op && op.width)).toEqual(["third", "two-thirds"]);
  });

  it("has nothing past either end of the scale", () => {
    // No handle writes a share the theme does not name, so the ends are silent
    // rather than clamped to a repeat of the same operation.
    const { ops, root } = mounted(geometryOf([block({ width: "quarter" })], [region()]));

    root
      .querySelector(".slidx-resize-grip")!
      .dispatchEvent(new window.KeyboardEvent("keydown", { key: "ArrowLeft" }));

    expect(ops).toEqual([]);
  });

  it("says in words which share a block now takes", () => {
    // The live region, because the handle's own position is not something a
    // screen reader can read as a width.
    const { root } = mounted(geometryOf([block()], [region()]));

    drag(root, 0, 400);

    expect(root.querySelector(".slidx-resize-status")!.textContent).toContain("quarter");
  });

  it("draws no handle on a region that has not been laid out", () => {
    // Before the frame has finished. A handle there would resize against a
    // region of zero width, which snaps to every share at once.
    const { root } = mounted(
      geometryOf([block()], [region({ rect: { left: 0, top: 0, width: 0, height: 0 } })]),
    );

    expect(root.querySelector(".slidx-resize-grip")).toBeNull();
  });
});

describe("what a share means", () => {
  it("is the nearest one the theme names, measured from the region's centre", () => {
    // Doubled, because a narrowed block is centred: the trailing edge moving out
    // by one step moves the leading edge in by the same one.
    const box = region();

    expect(shareAt(box, 800)).toBe("full");
    expect(shareAt(box, 600)).toBe("half");
    expect(shareAt(box, 400)).toBe("quarter");
  });

  it("runs from the whole region down, and no further", () => {
    expect(WIDTHS[0]!.name).toBe("full");
    expect(WIDTHS.map((width) => width.share)).toEqual([1, 0.75, 2 / 3, 0.5, 1 / 3, 0.25]);
    expect(stepped("full", "wider")).toBeUndefined();
    expect(stepped("quarter", "narrower")).toBeUndefined();
  });

  it("tells the linter about a code block that will not fit the box it is heading for", () => {
    // The measurement a pixel width could not have produced: the box is
    // `share × region`, and the region is a known share of the slide, so the
    // number goes to the same Rust rule the build runs.
    const measured = narrowing(region(), block({ needsWidth: 600 }), "half", 3);

    expect(measured).toEqual([
      { slideIndex: 3, stop: 0, overHeight: 0, overWidth: 0.5, region: "body" },
    ]);
  });

  it("says nothing about content that reflows", () => {
    // Whether prose still fits depends on where its lines break, which is a
    // layout that has not happened. A number here would look measured and not be.
    expect(narrowing(region(), block(), "quarter", 0)).toEqual([]);
    expect(narrowing(region(), block({ needsWidth: 100 }), "half", 0)).toEqual([]);
  });
});
