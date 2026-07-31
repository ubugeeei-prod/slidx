/**
 * The timeline, judged by the operations it produces and the stop it shows.
 *
 * Two halves, and they are tested apart because they fail for different
 * reasons. What a cell means is arithmetic over a grid — which position an
 * inserted action needs to land on the column that was clicked — and it is
 * wrong silently. What the playhead does is a message on a channel, and it is
 * wrong loudly. Neither needs a running dev server to say so.
 */

import { describe, expect, it } from "vite-plus/test";

import type { StepGrid } from "../src/client";
import type { EditOp } from "../src/operations";
import { createPlayhead, withStop } from "../src/playhead";
import { createTimeline } from "../src/timeline";
import { NO_STEPS, actionAt, isGenerated, positionFor, toggleCell } from "../src/timeline-cells";
import type { EditorState } from "../src/session";

/** A slide with three reveals, which is the shape an author builds first. */
function build(): StepGrid {
  return {
    declared: true,
    stops: 4,
    rows: [
      { target: "#a", label: "first", key: "a", visible: [false, true, true, true] },
      { target: "#b", label: "second", key: "b", visible: [false, false, true, true] },
      { target: "#c", label: "third", key: "c", visible: [false, false, false, true] },
    ],
    actions: [
      { index: 0, kind: "reveal", stop: 1, targets: ["#a"], timed: false, source: 'reveal: "#a"' },
      { index: 1, kind: "reveal", stop: 2, targets: ["#b"], timed: false, source: 'reveal: "#b"' },
      { index: 2, kind: "reveal", stop: 3, targets: ["#c"], timed: false, source: 'reveal: "#c"' },
    ],
  };
}

/** The same slide, staged by `autoSteps:` and therefore without lines to edit. */
function generated(): StepGrid {
  return { ...build(), declared: false, auto: "list" };
}

function stateOf(grid: StepGrid, slide = 0): EditorState {
  const slides = [0, 1].map((index) => ({
    id: `slide-${index}`,
    index,
    title: `Slide ${index}`,
    notes: [],
    stopCount: index === slide ? grid.stops : 1,
    steps: index === slide ? grid : NO_STEPS,
    // Required since the storyboard began drawing a slide's width from its
    // time; the timeline reads neither, so the values only have to be there.
    estimatedSeconds: 0,
    optional: false,
    style: {},
  }));

  return {
    source: "",
    spans: [],
    slides,
    layouts: [],
    diagnostics: [],
    selection: { slide },
    viewers: [],
    canUndo: false,
    canRedo: false,
  };
}

describe("a cell of the grid", () => {
  it("holds the action that lands on its stop and names its row", () => {
    const grid = build();

    expect(actionAt(grid, "#b", 2)?.index).toBe(1);
    expect(actionAt(grid, "#b", 1)).toBeUndefined();
  });

  it("puts a new action where the column it was clicked is, not at the end", () => {
    // The whole reason `addStep` takes a position. Clicking the second column
    // has to insert before the action that currently makes it, so the stop the
    // author clicked is the stop the action lands on.
    expect(positionFor(build(), 2)).toBe(1);
    expect(positionFor(build(), 1)).toBe(0);
  });

  it("appends when the column asked for is past the last stop", () => {
    expect(positionFor(build(), 4)).toBe(3);
  });

  it("adds a stop for its row when it is empty", () => {
    // The third row is not on screen until the last stop, so a click on the
    // first column means "bring it on here" and lands before every action.
    const op = toggleCell(build(), 0, build().rows[2]!, 1);

    expect(op).toEqual({
      op: "addStep",
      slide: 0,
      at: 0,
      action: { reveal: { target: "#c", options: {} } },
    } satisfies EditOp);
  });

  it("takes the row away rather than revealing it again when it is already on screen", () => {
    // A reveal of something the audience is looking at is not a step anybody
    // meant to author. The other intent that reads as "something happens here"
    // is taking it away.
    const op = toggleCell(build(), 0, build().rows[0]!, 2);

    expect(op).toEqual({
      op: "addStep",
      slide: 0,
      at: 1,
      action: { hide: { target: "#a", options: {} } },
    } satisfies EditOp);
  });

  it("removes the stop it holds when it holds one", () => {
    const op = toggleCell(build(), 0, build().rows[2]!, 3);

    expect(op).toEqual({ op: "removeStep", slide: 0, index: 2 } satisfies EditOp);
  });

  it("is not a step at the resting frame, because nothing has happened yet", () => {
    expect(toggleCell(build(), 0, build().rows[0]!, 0)).toBeUndefined();
  });

  it("cannot be changed in place when the stops were generated", () => {
    // There is no line in the file to splice. Writing one is a separate
    // operation an author has to ask for, which is the point of the door.
    expect(toggleCell(generated(), 0, generated().rows[0]!, 1)).toBeUndefined();
  });

  it("is editable on a slide that has no steps yet, because adding one writes the key", () => {
    const empty: StepGrid = { ...NO_STEPS, rows: build().rows, stops: 1 };

    expect(toggleCell(empty, 0, empty.rows[0]!, 1)).toEqual({
      op: "addStep",
      slide: 0,
      at: 0,
      action: { reveal: { target: "#a", options: {} } },
    } satisfies EditOp);
  });

  it("knows generated stops from a list an author has since written out", () => {
    expect(isGenerated(generated())).toBe(true);
    expect(isGenerated(build())).toBe(false);
    expect(isGenerated({ ...generated(), declared: true })).toBe(false);
    expect(isGenerated(NO_STEPS)).toBe(false);
  });
});

describe("the timeline panel", () => {
  it("draws one row per thing the slide addresses and one column per stop", () => {
    const timeline = createTimeline({ run: () => {} }, { transport: null });
    timeline.render(stateOf(build()));

    const rows = timeline.root.querySelectorAll(".slidx-timeline-row");
    expect([...rows].map((row) => row.querySelector(".slidx-timeline-label")!.textContent)).toEqual(
      ["first", "second", "third"],
    );
    // Four stops and one column past the end, which is where a new stop goes.
    expect(rows[0]!.querySelectorAll(".slidx-timeline-cell")).toHaveLength(5);
  });

  it("shows a row as on screen for every stop it is painted at", () => {
    const timeline = createTimeline({ run: () => {} }, { transport: null });
    timeline.render(stateOf(build()));

    const cells = timeline.root
      .querySelectorAll(".slidx-timeline-row")[1]!
      .querySelectorAll(".slidx-timeline-cell");

    expect([...cells].map((cell) => cell.getAttribute("data-on"))).toEqual([
      "false",
      "false",
      "true",
      "true",
      "true",
    ]);
  });

  it("asks for one operation when a cell is clicked", () => {
    const ops: EditOp[] = [];
    const timeline = createTimeline({ run: (op) => ops.push(op) }, { transport: null });
    timeline.render(stateOf(build()));

    cell(timeline.root, 2, 3).click();

    expect(ops).toEqual([{ op: "removeStep", slide: 0, index: 2 }]);
  });

  it("says a slide's stops are generated and offers to write them out", () => {
    const ops: EditOp[] = [];
    const timeline = createTimeline({ run: (op) => ops.push(op) }, { transport: null });
    timeline.render(stateOf(generated()));

    const notice = timeline.root.querySelector(".slidx-timeline-generated");
    expect(notice?.textContent).toContain("autoSteps: list");

    timeline.root.querySelector<HTMLElement>(".slidx-timeline-adopt")!.click();
    expect(ops).toEqual([{ op: "adoptSteps", slide: 0 }]);
  });

  it("does not offer to write out a list the author already wrote", () => {
    const timeline = createTimeline({ run: () => {} }, { transport: null });
    timeline.render(stateOf(build()));

    expect(timeline.root.querySelector(".slidx-timeline-adopt")).toBeNull();
  });

  it("refuses a click on a generated cell rather than converting the slide", () => {
    // One click must not open a one-way door. The author asks for that with
    // the action the notice offers, and nothing else.
    const ops: EditOp[] = [];
    const timeline = createTimeline({ run: (op) => ops.push(op) }, { transport: null });
    timeline.render(stateOf(generated()));

    cell(timeline.root, 0, 2).click();

    expect(ops).toEqual([]);
  });

  it("moves the playhead when a column is clicked, and says where it is", () => {
    const posted: unknown[] = [];
    const timeline = createTimeline(
      { run: () => {} },
      { transport: { post: (m) => posted.push(m), listen: () => () => {}, close: () => {} } },
    );
    timeline.render(stateOf(build()));

    timeline.root.querySelectorAll<HTMLElement>(".slidx-timeline-stop")[2]!.click();

    expect(posted).toEqual([{ type: "position", position: { slide: 0, step: 2 }, sequence: 1 }]);
    expect(timeline.root.querySelector(".slidx-timeline-where")!.textContent).toContain("2 of 4");
  });

  it("scrubs on the arrow keys and moves the selected row on the vertical ones", () => {
    const posted: unknown[] = [];
    const timeline = createTimeline(
      { run: () => {} },
      { transport: { post: (m) => posted.push(m), listen: () => () => {}, close: () => {} } },
    );
    timeline.render(stateOf(build()));

    press(timeline.root, "ArrowRight");
    press(timeline.root, "ArrowRight");
    press(timeline.root, "ArrowLeft");
    press(timeline.root, "ArrowDown");

    expect(posted).toHaveLength(3);
    expect(timeline.root.querySelector(".slidx-timeline-where")!.textContent).toContain("1 of 4");
    expect(cell(timeline.root, 1, 1).getAttribute("aria-current")).toBe("true");
  });

  it("adds a stop on Enter, as one operation", () => {
    const ops: EditOp[] = [];
    const timeline = createTimeline({ run: (op) => ops.push(op) }, { transport: null });
    timeline.render(stateOf(build()));

    press(timeline.root, "ArrowRight");
    press(timeline.root, "ArrowRight");
    press(timeline.root, "Enter");

    expect(ops).toEqual([
      { op: "addStep", slide: 0, at: 1, action: { hide: { target: "#a", options: {} } } },
    ]);
  });

  it("deletes the stop under the playhead on Delete", () => {
    const ops: EditOp[] = [];
    const timeline = createTimeline({ run: (op) => ops.push(op) }, { transport: null });
    timeline.render(stateOf(build()));

    press(timeline.root, "ArrowRight");
    press(timeline.root, "Delete");

    expect(ops).toEqual([{ op: "removeStep", slide: 0, index: 0 }]);
  });

  it("moves the selected action a stop at a time with a modifier held", () => {
    // The same axis as scrubbing, so the same two keys — with a modifier the
    // action moves rather than the playhead.
    const ops: EditOp[] = [];
    const timeline = createTimeline({ run: (op) => ops.push(op) }, { transport: null });
    timeline.render(stateOf(build()));

    press(timeline.root, "ArrowRight");
    press(timeline.root, "ArrowRight", { altKey: true });

    expect(ops).toEqual([{ op: "moveStep", slide: 0, from: 0, to: 1 }]);
  });

  it("changes what the selected stop does without moving it", () => {
    const ops: EditOp[] = [];
    const timeline = createTimeline({ run: (op) => ops.push(op) }, { transport: null });
    timeline.render(stateOf(build()));

    press(timeline.root, "ArrowRight");
    timeline.root.querySelector<HTMLElement>('[data-kind="emphasize"]')!.click();

    expect(ops).toEqual([
      {
        op: "setStep",
        slide: 0,
        index: 0,
        action: { emphasize: { target: "#a", options: {} } },
      },
    ]);
  });

  it("says the stop again when the canvas reloads under it", () => {
    // Every edit reloads that frame and a reloaded page opens at its first
    // stop. Without this, clicking a cell to see what stop three now does would
    // answer by showing stop zero.
    const posted: unknown[] = [];
    const frame = document.createElement("iframe");
    const timeline = createTimeline(
      { run: () => {} },
      {
        frame: () => frame,
        transport: { post: (m) => posted.push(m), listen: () => () => {}, close: () => {} },
      },
    );
    timeline.render(stateOf(build()));
    timeline.root.querySelectorAll<HTMLElement>(".slidx-timeline-stop")[3]!.click();

    frame.dispatchEvent(new Event("load"));

    expect(posted).toEqual([
      { type: "position", position: { slide: 0, step: 3 }, sequence: 1 },
      { type: "position", position: { slide: 0, step: 3 }, sequence: 2 },
    ]);
  });

  it("says nothing is staged rather than drawing an empty grid", () => {
    const timeline = createTimeline({ run: () => {} }, { transport: null });
    timeline.render(stateOf(NO_STEPS, 1));

    expect(timeline.root.querySelector(".slidx-timeline-grid")).toBeNull();
    expect(timeline.root.querySelector(".slidx-hint")!.textContent).toContain("name");
  });

  it("hides the playhead on a slide that advances in one press", () => {
    // A control that cannot move is a control an author tries to move.
    const timeline = createTimeline({ run: () => {} }, { transport: null });
    const scrub = timeline.root.querySelector(".slidx-timeline-scrub")!;

    timeline.render(stateOf(NO_STEPS, 1));
    expect(scrub.hasAttribute("hidden")).toBe(true);

    timeline.render(stateOf(build()));
    expect(scrub.hasAttribute("hidden")).toBe(false);
  });
});

describe("the playhead", () => {
  it("keeps the stop inside the slide it is pointed at", () => {
    const playhead = createPlayhead({ transport: null });
    playhead.moveTo(0, 3);

    expect(playhead.show(9)).toBe(2);
    expect(playhead.show(-1)).toBe(0);
    expect(playhead.step(1)).toBe(1);
  });

  it("starts a different slide at its first stop and keeps the same one where it was", () => {
    const playhead = createPlayhead({ transport: null });
    playhead.moveTo(0, 4);
    playhead.show(3);

    expect(playhead.moveTo(0, 4)).toBe(3);
    expect(playhead.moveTo(1, 4)).toBe(0);
  });

  it("clamps a stop the slide lost to an edit rather than showing nothing", () => {
    const playhead = createPlayhead({ transport: null });
    playhead.moveTo(0, 5);
    playhead.show(4);

    expect(playhead.moveTo(0, 2)).toBe(1);
  });

  it("sends the position rather than reloading the frame when a channel exists", () => {
    const posted: unknown[] = [];
    const frame = document.createElement("iframe");
    frame.setAttribute("src", "/slides/2/?at=1");

    const playhead = createPlayhead({
      frame: () => frame,
      transport: { post: (m) => posted.push(m), listen: () => () => {}, close: () => {} },
    });
    playhead.moveTo(1, 4);
    playhead.show(2);

    expect(posted).toHaveLength(1);
    expect(frame.getAttribute("src")).toBe("/slides/2/?at=1");
    expect(playhead.mirrored).toBe(true);
  });

  it("falls back to the deck's own deep link where there is no channel", () => {
    const frame = document.createElement("iframe");
    frame.setAttribute("src", "/slides/2/?at=1");

    const playhead = createPlayhead({ frame: () => frame, transport: null });
    playhead.moveTo(1, 4);
    playhead.show(2);

    expect(frame.getAttribute("src")).toBe("/slides/2/?at=1&step=2");
    expect(playhead.mirrored).toBe(false);
  });

  it("writes the stop into a frame url the way the deck writes its own", () => {
    expect(withStop("/slides/2/?at=1", 3)).toBe("/slides/2/?at=1&step=3");
    expect(withStop("/slides/2/?at=1&step=3", 5)).toBe("/slides/2/?at=1&step=5");
    // `?step=0` is noise in a URL, and the deck leaves it off for that reason.
    expect(withStop("/slides/2/?step=3", 0)).toBe("/slides/2/");
  });
});

/** One cell of the drawn grid, by row and column. */
function cell(root: HTMLElement, row: number, stop: number): HTMLElement {
  const rows = root.querySelectorAll(".slidx-timeline-row");
  return rows[row]!.querySelectorAll<HTMLElement>(".slidx-timeline-cell")[stop]!;
}

function press(root: HTMLElement, key: string, over: KeyboardEventInit = {}): void {
  root.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, ...over }));
}
