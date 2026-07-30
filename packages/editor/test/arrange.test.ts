/**
 * The gesture, judged by the operation it produces.
 *
 * Every assertion here is about an `EditOp` or about what a keyboard can reach,
 * because those are the two things this surface exists for: one drag is one
 * operation, and everything a pointer can do a key can do too.
 *
 * The geometry is injected. A browser that lays a slide out is not what these
 * are about, and one that does not — which is every test environment — would
 * report every rectangle as zero.
 */

import { describe, expect, it } from "vite-plus/test";

import { createArrange } from "../src/arrange";
import type { Finding } from "../src/client";
import type { BlockBox, SlideGeometry } from "../src/geometry";
import type { EditOp } from "../src/operations";
import type { EditorState } from "../src/session";
import type { Measurement } from "../src/placement";

function block(index: number, region: string, top: number, over: Partial<BlockBox> = {}): BlockBox {
  return {
    index,
    region,
    rect: { left: region === "left" ? 100 : 500, top, width: 400, height: 80 },
    needsWidth: 0,
    ...over,
  };
}

/** A `split` slide: `left` holds blocks 0 and 1, `right` holds block 2. */
function split(over: Partial<SlideGeometry> = {}): SlideGeometry {
  return {
    slide: { left: 80, top: 80, width: 840, height: 640 },
    safe: { left: 100, top: 100, width: 800, height: 600 },
    regions: [
      {
        name: "left",
        rect: { left: 100, top: 100, width: 400, height: 600 },
        blocks: [0, 1],
        contentHeight: 200,
        gap: 20,
      },
      {
        name: "right",
        rect: { left: 500, top: 100, width: 400, height: 600 },
        blocks: [2],
        contentHeight: 100,
        gap: 20,
      },
    ],
    blocks: [block(0, "left", 100), block(1, "left", 200), block(2, "right", 100)],
    ...over,
  };
}

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

interface Opened {
  ops: EditOp[];
  foreseen: Finding[][];
  asked: Measurement[][];
  root: HTMLElement;
  grip(index: number): HTMLButtonElement;
}

function open(
  geometry: SlideGeometry = split(),
  findings: Finding[] = [],
  slide = 0,
): Opened {
  const ops: EditOp[] = [];
  const foreseen: Finding[][] = [];
  const asked: Measurement[][] = [];

  const surface = createArrange(
    { run: (op) => ops.push(op), foresee: (found) => foreseen.push(found) },
    {
      geometry: () => geometry,
      measure: async (measured) => {
        asked.push(measured);
        return findings;
      },
    },
  );

  document.body.append(surface.root);
  surface.render(stateOf(slide));

  return {
    ops,
    foreseen,
    asked,
    root: surface.root,
    grip: (index) =>
      surface.root.querySelector<HTMLButtonElement>(`.slidx-arrange-grip[data-block="${index}"]`)!,
  };
}

/** A pointer event the way a browser sends one, at a point on the slide. */
function pointer(kind: string, x = 0, y = 0): Event {
  const event = new MouseEvent(kind, { clientX: x, clientY: y, bubbles: true });
  Object.defineProperty(event, "pointerId", { value: 1 });

  return event;
}

function drag(opened: Opened, index: number, x: number, y: number): void {
  const handle = opened.grip(index);

  handle.dispatchEvent(pointer("pointerdown", 0, 0));
  handle.dispatchEvent(pointer("pointermove", x, y));
  handle.dispatchEvent(pointer("pointerup", x, y));
}

const settled = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("arranging a slide", () => {
  it("gives every block a grip, and every grip is something a key can reach", () => {
    // A deck that can only be arranged with a pointer is a deck half the people
    // who write one cannot arrange at all.
    const opened = open();
    const grips = [...opened.root.querySelectorAll(".slidx-arrange-grip")];

    expect(grips).toHaveLength(3);
    expect(grips.every((grip) => grip.tagName === "BUTTON")).toBe(true);
    expect(grips[2]!.getAttribute("aria-label")).toBe("Move block 3, in right");
  });

  it("draws a box for every region the block could be dropped into", () => {
    const named = [...open().root.querySelectorAll(".slidx-arrange-region")].map((area) =>
      area.getAttribute("data-region"),
    );

    expect(named).toEqual(["left", "right"]);
  });

  it("ends a drag in one operation naming a region and a position", () => {
    // One gesture, one operation, one press of undo. Two operations would make
    // taking a drag back cost two.
    const opened = open();
    drag(opened, 0, 600, 120);

    expect(opened.ops).toEqual([
      { op: "moveBlock", slide: 0, block: 0, to: 1, region: "right" },
    ]);
  });

  it("sends the place a block is already in when a drag ends where it started", () => {
    // The operation still goes, because the pipeline is the only thing allowed
    // to decide that a change is not one: it plans an empty edit, and the undo
    // stack ignores an empty inverse. Deciding it here would be the editor
    // reasoning about Markdown.
    const opened = open();
    drag(opened, 0, 200, 120);

    expect(opened.ops).toEqual([{ op: "moveBlock", slide: 0, block: 0, to: 0, region: "left" }]);
  });

  it("drops nothing when the drag ends off the slide", () => {
    const opened = open();
    drag(opened, 0, 2000, 2000);

    expect(opened.ops).toEqual([]);
  });

  it("moves a block with the arrow keys and never with a pointer", () => {
    const opened = open();
    opened.grip(0).dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }));
    opened.grip(2).dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft" }));

    expect(opened.ops).toEqual([
      { op: "moveBlock", slide: 0, block: 0, to: 1, region: "left" },
      { op: "moveBlock", slide: 0, block: 2, to: 2, region: "left" },
    ]);
  });

  it("leaves a key that would move a block off the end alone", () => {
    const opened = open();
    opened.grip(0).dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp" }));

    expect(opened.ops).toEqual([]);
  });

  it("says where the block went, for a reader who cannot see the ghost", () => {
    const opened = open();
    opened.grip(0).dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight" }));

    const status = opened.root.querySelector(".slidx-arrange-status")!;
    expect(status.getAttribute("aria-live")).toBe("polite");
    expect(status.textContent).toBe("Block 1 moved to right, position 2.");
  });

  it("names the slide being edited rather than the first one", () => {
    const opened = open(split(), [], 4);
    drag(opened, 0, 600, 120);

    expect(opened.ops[0]).toMatchObject({ slide: 4 });
  });
});

describe("warning before the block lands", () => {
  const narrow = () => {
    const geometry = split();
    geometry.regions[1]!.rect.width = 200;
    geometry.blocks[0]!.needsWidth = 380;

    return geometry;
  };

  const clipped: Finding[] = [
    {
      severity: "error",
      code: "overflow/clipped",
      message: "the right region loses content off its right-hand edge",
      slideIndex: 0,
    },
  ];

  it("asks the linter what a landing would cost, and shows what it says", async () => {
    // The whole reason a region beats four floats in the file: a region is its
    // own box, so the rule the build runs can be asked about it mid-drag.
    const opened = open(narrow(), clipped);
    opened.grip(0).dispatchEvent(pointer("pointerdown", 0, 0));
    opened.grip(0).dispatchEvent(pointer("pointermove", 600, 400));
    await settled();

    expect(opened.asked).toEqual([
      [{ slideIndex: 0, stop: 0, overHeight: 0, overWidth: 0.9, region: "right" }],
    ]);
    expect(opened.foreseen.at(-1)).toEqual(clipped);
  });

  it("asks once per region rather than once per pointer event", async () => {
    const opened = open(narrow(), clipped);
    opened.grip(0).dispatchEvent(pointer("pointerdown", 0, 0));
    for (const y of [200, 300, 400, 500]) {
      opened.grip(0).dispatchEvent(pointer("pointermove", 600, y));
    }
    await settled();

    expect(opened.asked).toHaveLength(1);
  });

  it("takes the warning away the moment the gesture ends", async () => {
    // A warning about a placement nobody made is a warning about nothing.
    const opened = open(narrow(), clipped);
    opened.grip(0).dispatchEvent(pointer("pointerdown", 0, 0));
    opened.grip(0).dispatchEvent(pointer("pointermove", 600, 400));
    await settled();
    opened.grip(0).dispatchEvent(pointer("pointerup", 600, 400));

    expect(opened.foreseen.at(-1)).toEqual([]);
  });

  it("asks nothing at all when there is nothing exact to say", async () => {
    // Whether a paragraph still fits depends on where its lines break, and this
    // side of the boundary cannot know. Silence rather than a guess.
    const opened = open(split(), clipped);
    opened.grip(0).dispatchEvent(pointer("pointerdown", 0, 0));
    opened.grip(0).dispatchEvent(pointer("pointermove", 600, 400));
    await settled();

    expect(opened.asked).toEqual([]);
    expect(opened.foreseen.at(-1)).toEqual([]);
  });
});
