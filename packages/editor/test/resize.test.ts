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
import { RESIZE_STYLESHEET } from "../src/resize-styles";
import { createResize } from "../src/resize";
import type { EditorState } from "../src/session";
import { boxAt, narrowing, shareAt, stepped, widthOf, WIDTHS } from "../src/widths";

function stateOf(slide = 0): EditorState {
  return {
    source: "",
    spans: [],
    slides: [],
    layouts: [],
    activeTheme: "",
    themeLocked: false,
    themes: [],
    transitions: [],
    diagnostics: [],
    selection: { slide },
    viewers: [],
    canUndo: false,
    canRedo: false,
    writing: false,
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
    width: "fit",
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
    // `full` is deliberate rather than the default, so the editor must send the
    // word and let `slidx_edit` preserve it in the source.
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

  it("steps outwards and inwards from fit using the measured box", () => {
    const fitted = block({ rect: { left: 240, top: 0, width: 320, height: 100 } });
    const { ops, root } = mounted(geometryOf([fitted], [region()]));
    const handle = root.querySelector(".slidx-resize-grip")!;

    handle.dispatchEvent(new window.KeyboardEvent("keydown", { key: "ArrowRight" }));
    handle.dispatchEvent(new window.KeyboardEvent("keydown", { key: "ArrowLeft" }));

    expect(ops.map((op) => "width" in op && op.width)).toEqual(["half", "third"]);
  });

  it("can make a region-capped fit block explicitly full from the keyboard", () => {
    const { ops, root } = mounted(geometryOf([block()], [region()]));

    root
      .querySelector(".slidx-resize-grip")!
      .dispatchEvent(new window.KeyboardEvent("keydown", { key: "ArrowRight" }));

    expect(ops).toEqual([{ op: "setBlockWidth", slide: 0, block: 0, width: "full" }]);
  });

  it("keeps the intrinsic width as a snap target while a fit block is dragged", () => {
    const fitted = block({ rect: { left: 200, top: 0, width: 400, height: 100 } });
    const { ops, root } = mounted(geometryOf([fitted], [region()]));

    drag(root, 0, 600);

    expect(ops).toEqual([{ op: "setBlockWidth", slide: 0, block: 0, width: "fit" }]);
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

describe("the resize handle as a piece of interface", () => {
  const declarations = () => RESIZE_STYLESHEET.replaceAll(/\/\*[\s\S]*?\*\//g, "");
  const rule = (selector: string) => {
    const source = declarations();
    const start = source.indexOf(`${selector} {`);
    if (start === -1) return "";
    const body = source.indexOf("{", start) + 1;
    return source.slice(body, source.indexOf("}", body));
  };

  it("keeps a 28px keyboard-accessible target around quiet ink", () => {
    const { root } = mounted(geometryOf([block()], [region()]));
    const handle = root.querySelector<HTMLButtonElement>(".slidx-resize-grip")!;

    expect(handle.style.width).toBe("28px");
    expect(handle.style.height).toBe("28px");
    expect(handle.getAttribute("tabindex")).toBe("0");
    expect(handle.getAttribute("aria-label")).toBe("Resize block 1");
    expect(rule(".slidx-resize-grip")).toContain("pointer-events: auto");
    expect(rule(".slidx-resize-grip::before")).toContain("width: var(--slidx-e-hairline)");
    expect(rule(".slidx-resize-grip::before")).toContain("opacity: 0.22");
  });

  it("promotes hover, keyboard focus, and active resize to the accent", () => {
    const active = rule(
      '.slidx-resize-grip:hover::before,\n.slidx-resize-grip:focus-visible::before,\n.slidx-resize-grip[data-moving="true"]::before',
    );

    expect(active).toContain("background: var(--slidx-e-accent)");
    expect(active).toContain("opacity: 1");
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

  it("treats fit as measured geometry rather than inventing a fixed share", () => {
    const fitted = block({ rect: { left: 240, top: 10, width: 320, height: 80 } });

    expect(widthOf(fitted)).toBe("fit");
    expect(shareAt(region(), 560, 0.4)).toBe("fit");
    expect(shareAt(region(), 801, 1)).toBe("full");
    expect(boxAt(region(), fitted, "fit")).toEqual(fitted.rect);
    expect(stepped("fit", "wider", 0.4)).toBe("half");
    expect(stepped("fit", "narrower", 0.4)).toBe("third");
    expect(stepped("fit", "wider", 1)).toBe("full");
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
