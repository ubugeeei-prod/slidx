/**
 * Where the other people are, drawn on the block they are actually in.
 *
 * The assertions that matter here are the ones about not drawing. A mark is a
 * claim that a named person is working on a named paragraph, and the position
 * and the deck it is a position in arrive on the same stream but not in the
 * same frame — so the interesting cases are the moments where the two disagree,
 * and every one of them has to end in nothing rather than in a guess.
 *
 * Geometry is injected, because a DOM test runner does not lay out the deck.
 */

import { describe, expect, it } from "vite-plus/test";

import { BEACON_INKS, createBeacons, inkFor, marksFor } from "../src/beacons";
import type { Viewer } from "../src/collab";
import type { BlockBox, Rect, SlideGeometry } from "../src/geometry";
import type { EditorState } from "../src/session";

const FIRST: Rect = { left: 200, top: 150, width: 300, height: 100 };
const SECOND: Rect = { left: 200, top: 300, width: 300, height: 80 };

function block(index: number, rect: Rect): BlockBox {
  return { index, region: "body", rect, needsWidth: 0, width: "full" };
}

function geometry(): SlideGeometry {
  return {
    slide: { left: 80, top: 80, width: 840, height: 460 },
    safe: { left: 100, top: 100, width: 800, height: 400 },
    regions: [
      {
        name: "body",
        rect: { left: 100, top: 100, width: 800, height: 400 },
        blocks: [0, 1],
        contentHeight: 200,
        gap: 20,
      },
    ],
    blocks: [block(0, FIRST), block(1, SECOND)],
  };
}

function state(slide = 0, viewers: Viewer[] = []): EditorState {
  return {
    source: "",
    spans: [],
    slides: [],
    layouts: [],
    diagnostics: [],
    selection: { slide },
    viewers,
    canUndo: false,
    canRedo: false,
  };
}

function guest(id: string, slide: number, block?: number): Viewer {
  return {
    id,
    label: `guest ${id}`,
    local: false,
    canEdit: true,
    slide,
    ...(block === undefined ? {} : { block }),
  };
}

/**
 * A surface reading the one state every other surface reads.
 *
 * `null` rather than `undefined` for a canvas with nothing to measure, because
 * an argument of `undefined` is what a default parameter is for.
 */
function open(slide = 0, known: SlideGeometry | null = geometry()) {
  const surface = createBeacons({ geometry: () => known ?? undefined });

  return {
    surface,
    saw: (viewers: Viewer[], on = slide) => surface.render(state(on, viewers)),
    marks: () => [...surface.root.querySelectorAll(".slidx-beacon")],
    names: () =>
      [...surface.root.querySelectorAll(".slidx-beacon-name")].map((node) => node.textContent),
  };
}

describe("marking where somebody else is working", () => {
  it("draws a mark on the block a guest has selected", () => {
    const { saw, marks } = open(2);
    saw([guest("b", 2, 1)]);

    expect(marks()).toHaveLength(1);
    expect(marks()[0]!.getAttribute("data-viewer")).toBe("b");
  });

  it("puts it on that block's rectangle", () => {
    const { saw, marks } = open(2);
    saw([guest("b", 2, 1)]);

    const style = (marks()[0] as HTMLElement).style;
    expect([style.left, style.top, style.width, style.height]).toEqual([
      `${SECOND.left}px`,
      `${SECOND.top}px`,
      `${SECOND.width}px`,
      `${SECOND.height}px`,
    ]);
  });

  it("names the guest on the mark", () => {
    const { saw, names } = open(2);
    saw([guest("b", 2, 0)]);

    expect(names()).toEqual(["guest b"]);
  });

  it("draws two guests on one slide in different colours", () => {
    const { saw, marks } = open(2);
    saw([guest("b", 2, 0), guest("c", 2, 1)]);

    const inks = marks().map((node) =>
      (node as HTMLElement).style.getPropertyValue("--slidx-beacon-ink"),
    );
    expect(inks[0]).not.toBe(inks[1]);
  });

  it("keeps a guest's colour when another guest closes their tab", () => {
    // The colour comes from the seat rather than from a place in the roster,
    // so the marks do not all change identity when one person leaves.
    const { saw, marks } = open(2);
    saw([guest("b", 2, 0), guest("c", 2, 1)]);
    const before = (marks()[1] as HTMLElement).style.getPropertyValue("--slidx-beacon-ink");

    saw([guest("c", 2, 1)]);

    expect((marks()[0] as HTMLElement).style.getPropertyValue("--slidx-beacon-ink")).toBe(before);
  });
});

describe("what is never drawn", () => {
  it("draws nothing for the author's own seat", () => {
    // The author is in the roster too, and a mark around the block they have
    // selected would sit under the handles they are already holding.
    const { saw, marks } = open(2);
    saw([{ id: "me", label: "you", local: true, canEdit: true, slide: 2, block: 0 }]);

    expect(marks()).toEqual([]);
  });

  it("draws nothing for a guest on another slide", () => {
    const { saw, marks } = open(2);
    saw([guest("b", 3, 0)]);

    expect(marks()).toEqual([]);
  });

  it("draws nothing for a guest who has selected nothing", () => {
    const { saw, marks } = open(2);
    saw([guest("b", 2)]);

    expect(marks()).toEqual([]);
  });

  it("draws nothing for a block this slide does not have", () => {
    // Their position and the deck it is a position in arrive on the same
    // stream and not in the same frame, so this is a real second of every
    // move. A rectangle guessed for them would carry a name.
    const { saw, marks } = open(2);
    saw([guest("b", 2, 9)]);

    expect(marks()).toEqual([]);
  });

  it("draws nothing at all while there is nothing to measure", () => {
    // Before the canvas has loaded, and while the author has the Markdown view
    // up instead of the slide.
    const { saw, marks } = open(2, null);
    saw([guest("b", 2, 0)]);

    expect(marks()).toEqual([]);
  });

  it("clears the marks when everybody leaves", () => {
    const { saw, marks } = open(2);
    saw([guest("b", 2, 0)]);
    saw([]);

    expect(marks()).toEqual([]);
  });

  it("clears the marks when the author moves to another slide", () => {
    const { saw, marks } = open(2);
    saw([guest("b", 2, 0)]);
    saw([guest("b", 2, 0)], 3);

    expect(marks()).toEqual([]);
  });
});

describe("choosing which viewers have somewhere to be drawn", () => {
  it("reports one mark per viewer with a block on this slide", () => {
    const marks = marksFor([guest("b", 1, 0), guest("c", 1, 1), guest("d", 2, 0)], 1, geometry());

    expect(marks.map(({ viewer }) => viewer.id)).toEqual(["b", "c"]);
    expect(marks[0]!.rect).toEqual(FIRST);
  });

  it("gives every seat one of the inks and nothing else", () => {
    const inks = new Set(
      ["a", "b", "c", "d", "e", "f", "seat-1", "01234567-89ab-cdef-0123-456789abcdef"].map(inkFor),
    );

    for (const ink of inks) expect(BEACON_INKS).toContain(ink);
  });

  it("gives one seat the same ink every time it is asked", () => {
    expect(inkFor("seat-1")).toBe(inkFor("seat-1"));
  });
});
