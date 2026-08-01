/**
 * Free movement and free resizing, asserted as the one Markdown operation a
 * gesture produces. Geometry is injected because a DOM test runner does not lay
 * out the real deck; browser coverage exercises the same surface over the
 * rendered iframe.
 */

import { afterEach, describe, expect, it } from "vite-plus/test";

import { FREEFORM_STYLESHEET } from "../src/freeform-styles";
import { createFreeform } from "../src/freeform";
import type { BlockBox, Rect, SlideGeometry } from "../src/geometry";
import type { EditOp } from "../src/operations";
import type { EditorState } from "../src/session";

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

function state(selected: number | null = 0, slide = 2): EditorState {
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
    selection: { slide, block: selected ?? undefined },
    viewers: [],
    canUndo: false,
    canRedo: false,
    writing: false,
  };
}

function open() {
  const ops: EditOp[] = [];
  const selections: Array<number | undefined> = [];
  const known = geometry();
  const surface = createFreeform(
    {
      run: (op) => ops.push(op),
      select: (selected) => selections.push(selected),
    },
    { geometry: () => known },
  );

  document.body.append(surface.root);
  surface.render(state());

  return { ops, selections, root: surface.root };
}

function pointer(kind: string, x: number, y: number): Event {
  const event = new MouseEvent(kind, {
    bubbles: true,
    cancelable: true,
    clientX: x,
    clientY: y,
  });
  Object.defineProperty(event, "pointerId", { value: 1 });
  return event;
}

function drag(control: Element, dx: number, dy: number): void {
  control.dispatchEvent(pointer("pointerdown", 0, 0));
  control.dispatchEvent(pointer("pointermove", dx, dy));
  control.dispatchEvent(pointer("pointerup", dx, dy));
}

afterEach(() => document.body.replaceChildren());

describe("freeform canvas controls", () => {
  /** The inline box `paint` writes, which is the geometry under test. */
  function boxOf(element: HTMLElement) {
    return {
      top: Number.parseFloat(element.style.top),
      left: Number.parseFloat(element.style.left),
      width: Number.parseFloat(element.style.width),
      height: Number.parseFloat(element.style.height),
    };
  }

  it("keeps the move grip off the block it moves", () => {
    // It used to be the block's own width and straddle its top edge, so a
    // transparent button covered the first fourteen pixels of the content —
    // most of a one-line block. Every click that landed there began a drag
    // instead of reaching the text.
    const grip = boxOf(open().root.querySelector<HTMLElement>(".slidx-freeform-move")!);

    expect(grip.top + grip.height).toBeLessThanOrEqual(SELECTED.top);
  });

  it("keeps the move grip clear of the handles that share the top edge", () => {
    // `n`, `nw` and `ne` are centred on the top edge, so they own the band half
    // a hit target either side of it. A grip reaching into it would take the
    // clicks meant for a resize.
    const root = open().root;
    const grip = boxOf(root.querySelector<HTMLElement>(".slidx-freeform-move")!);
    const north = boxOf(root.querySelector<HTMLElement>('[data-handle="n"]')!);

    expect(grip.top + grip.height).toBeLessThan(north.top);
  });

  it("sizes the move grip against the hand, not against the block", () => {
    // The property the old code got backwards: it was `max(block, hit)` wide,
    // so a wide block got a wide button and a wide button covered wide content.
    const wide = boxOf(open().root.querySelector<HTMLElement>(".slidx-freeform-move")!);

    expect(wide.width).toBeLessThan(SELECTED.width);
    expect(wide.height).toBeGreaterThanOrEqual(28);
  });

  it("marks the move grip, because an invisible button is a trap", () => {
    // The eight resize handles each draw a dot. This one drew nothing at all,
    // so the only way to discover it was to click where it happened to be.
    expect(FREEFORM_STYLESHEET).toContain(".slidx-freeform-move::before");
    expect(FREEFORM_STYLESHEET).toContain(".slidx-freeform-move:hover::before");
  });

  it("moves a selected block in one inset operation", () => {
    const opened = open();

    drag(opened.root.querySelector(".slidx-freeform-move")!, 80, 40);

    expect(opened.ops).toEqual([
      {
        op: "setBlockStyle",
        slide: 2,
        block: 0,
        property: "inset",
        value: "22.5% 40% 52.5% 22.5%",
      },
    ]);
  });

  it("resizes two edges with one corner gesture and one operation", () => {
    const opened = open();

    drag(opened.root.querySelector('[data-handle="se"]')!, 80, 40);

    expect(opened.ops).toEqual([
      {
        op: "setBlockStyle",
        slide: 2,
        block: 0,
        property: "inset",
        value: "12.5% 40% 52.5% 12.5%",
      },
    ]);
  });

  it("does not write when a pointer gesture changed nothing", () => {
    const opened = open();

    drag(opened.root.querySelector('[data-handle="se"]')!, 0, 0);

    expect(opened.ops).toEqual([]);
    expect(opened.root.getAttribute("data-manipulating")).toBe("false");
  });

  it("nudges by one pixel, or ten with Shift", () => {
    const opened = open();
    const move = opened.root.querySelector(".slidx-freeform-move")!;

    move.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight" }));
    move.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", shiftKey: true }));

    expect(opened.ops.map((op) => ("value" in op ? op.value : undefined))).toEqual([
      "12.5% 49.875% 62.5% 12.625%",
      "15% 50% 60% 12.5%",
    ]);
  });

  it("clears the block selection with Escape", () => {
    const opened = open();

    opened.root
      .querySelector(".slidx-freeform-move")!
      .dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));

    expect(opened.selections).toEqual([undefined]);
    expect(opened.ops).toEqual([]);
  });

  it("keeps compact visible handles but generous invisible hit targets", () => {
    const opened = open();
    const frame = opened.root.querySelector<HTMLElement>(".slidx-freeform-frame")!;
    const handle = opened.root.querySelector<HTMLElement>('[data-handle="nw"]')!;

    expect(frame.style.left).toBe("192px");
    expect(frame.style.top).toBe("142px");
    expect(handle.style.width).toBe("28px");
    expect(handle.style.height).toBe("28px");
    expect(opened.root.getAttribute("data-manipulating")).toBe("false");
    expect(FREEFORM_STYLESHEET).toContain("width: 5px");
    expect(FREEFORM_STYLESHEET).toContain("outline: 1px solid");
    expect(FREEFORM_STYLESHEET).toContain(".slidx-freeform-move:focus-visible");
    expect(FREEFORM_STYLESHEET).toContain("outline: 0");
    expect(FREEFORM_STYLESHEET).not.toMatch(/box-shadow|gradient/);
  });

  it("keeps the idle frame and handles quiet until they are used", () => {
    const declarations = FREEFORM_STYLESHEET.replaceAll(/\/\*[\s\S]*?\*\//g, "");

    expect(declarations).toMatch(
      /\.slidx-freeform-frame \{[^}]*color-mix\(in srgb, var\(--slidx-e-accent\) 20%, transparent\)/s,
    );
    expect(declarations).toMatch(
      /\.slidx-freeform-handle::before \{[^}]*width: 5px;[^}]*height: 5px;[^}]*border: 1px solid var\(--slidx-e-line\);[^}]*opacity: 0\.32;/s,
    );
    expect(declarations).toMatch(
      /\.slidx-freeform-handle:hover::before,\s*\.slidx-freeform-handle:focus-visible::before \{[^}]*background: var\(--slidx-e-accent\);[^}]*opacity: 1;/s,
    );
    expect(declarations).toMatch(
      /\.slidx-freeform\[data-manipulating="true"\] \.slidx-freeform-handle::before \{[^}]*border-color: var\(--slidx-e-accent\);[^}]*opacity: 1;/s,
    );
    expect(declarations).toContain(
      ".slidx-freeform:has(.slidx-freeform-move:focus-visible, .slidx-freeform-handle:focus-visible) .slidx-freeform-frame",
    );
  });

  it("hides the controls when no block is selected", () => {
    const opened = open();
    const surface = createFreeform({ run: () => {}, select: () => {} }, { geometry });
    document.body.append(surface.root);

    surface.render(state(null));

    expect(surface.root.getAttribute("data-active")).toBe("false");
    expect(opened.root.getAttribute("data-active")).toBe("true");
  });
});
