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
    diagnostics: [],
    selection: { slide, block: selected ?? undefined },
    viewers: [],
    canUndo: false,
    canRedo: false,
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
    {
      geometry: () => known,
      visual: () => ({ color: "rgb(165, 201, 255)", managedColor: true }),
    },
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

  it("sets and resets the selected block color as managed style operations", () => {
    const opened = open();
    const color = opened.root.querySelector<HTMLInputElement>(".slidx-freeform-color-input")!;
    const reset = opened.root.querySelector<HTMLButtonElement>(".slidx-freeform-color-reset")!;

    expect(color.value).toBe("#a5c9ff");
    expect(reset.disabled).toBe(false);

    color.value = "#d946ef";
    color.dispatchEvent(new Event("change"));
    reset.click();

    expect(opened.ops).toEqual([
      {
        op: "setBlockStyle",
        slide: 2,
        block: 0,
        property: "color",
        value: "#d946ef",
      },
      {
        op: "setBlockStyle",
        slide: 2,
        block: 0,
        property: "color",
      },
    ]);
    expect(reset.disabled).toBe(true);
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
    expect(FREEFORM_STYLESHEET).toContain("width: 7px");
    expect(FREEFORM_STYLESHEET).toContain("outline: 1px solid");
    expect(FREEFORM_STYLESHEET).toContain(".slidx-freeform-move:focus-visible");
    expect(FREEFORM_STYLESHEET).toContain("outline: 0");
    expect(FREEFORM_STYLESHEET).not.toMatch(/box-shadow|gradient/);
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
