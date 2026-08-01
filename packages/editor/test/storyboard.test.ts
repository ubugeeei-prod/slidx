/**
 * The storyboard, judged by the sentences it says and the operations it makes.
 *
 * Two things are being held here. The arithmetic — which number a slide's width
 * came from, and whether the deck fits — is asserted on the plan, because that
 * is where a wrong answer would come from. Everything else asserts an `EditOp`,
 * for the reason every surface test in this package does: an operation is the
 * only thing the editor is allowed to make.
 */

import { describe, expect, it } from "vite-plus/test";

import { ARRANGE_STYLESHEET } from "../src/arrange-styles";
import type { SlideSummary } from "../src/client";
import type { EditOp } from "../src/operations";
import type { EditorState } from "../src/session";
import { createStoryboard } from "../src/storyboard";
import { formatSeconds, planTime } from "../src/storyboard/plan";
import { STORYBOARD_STYLESHEET } from "../src/storyboard/styles";

function slideOf(over: Partial<SlideSummary> = {}): SlideSummary {
  return {
    id: "one",
    index: 0,
    title: "One",
    notes: [],
    stopCount: 1,
    estimatedSeconds: 0,
    optional: false,
    style: {},
    frontmatter: {},
    ...over,
  };
}

/**
 * Three slides: one budgeted, one carried by its notes, one marked optional.
 *
 * The mix is the point. A storyboard whose slides all declared a budget would
 * never exercise the fallback, which is the state a talk is in while it is
 * being written.
 */
function slides(): SlideSummary[] {
  return [
    slideOf({
      id: "opening",
      index: 0,
      title: "Opening",
      notes: ["Open with the outcome."],
      estimatedSeconds: 40,
      budgetSeconds: 90,
      frontmatter: { budget: "90s" },
    }),
    slideOf({
      id: "middle",
      index: 1,
      title: "The middle",
      notes: ["What the parser does.", "Why it matters."],
      estimatedSeconds: 120,
    }),
    slideOf({
      id: "aside",
      index: 2,
      title: "An aside",
      notes: ["Only if there is time."],
      estimatedSeconds: 25,
      budgetSeconds: 30,
      optional: true,
      frontmatter: { budget: "30s", optional: true },
    }),
  ];
}

function stateOf(over: Partial<EditorState> = {}): EditorState {
  return {
    source: "",
    spans: [],
    slides: slides(),
    layouts: [],
    activeTheme: "",
    themeLocked: false,
    themes: [],
    transitions: [],
    diagnostics: [],
    selection: { slide: 1 },
    viewers: [],
    canUndo: false,
    canRedo: false,
    writing: false,
    durationSeconds: 300,
    ...over,
  };
}

function recorder() {
  const ops: EditOp[] = [];
  const selected: number[] = [];

  return {
    ops,
    selected,
    run: (op: EditOp) => {
      ops.push(op);
    },
    select: (at: number) => {
      selected.push(at);
    },
  };
}

/** The storyboard as an author reaches it: closed, then asked for. */
function open(state = stateOf()) {
  const log = recorder();
  const storyboard = createStoryboard(log);
  storyboard.render(state);
  storyboard.root.querySelector<HTMLElement>(".slidx-sb-launch")!.click();

  return { log, storyboard, root: storyboard.root };
}

const rowsIn = (root: HTMLElement) => [...root.querySelectorAll<HTMLElement>(".slidx-sb-slide")];

describe("the storyboard's plan", () => {
  it("takes the budget the author declared and says that is where the width came from", () => {
    const plan = planTime(slides(), 300);

    expect(plan.slides[0]!.seconds).toBe(90);
    expect(plan.slides[0]!.source).toBe("budget");
  });

  it("falls back to the spoken length of the notes, and says the width is an estimate", () => {
    // The number a talk has before anyone has rehearsed it, which is the state
    // a deck spends most of its life in.
    const plan = planTime(slides(), 300);

    expect(plan.slides[1]!.seconds).toBe(120);
    expect(plan.slides[1]!.source).toBe("estimate");
  });

  it("lays the deck against the slot rather than against itself", () => {
    // 90 + 120 + 30 against 300: the bar is the slot, and the talk fills four
    // fifths of it. A bar scaled to the deck would make every deck look full.
    const plan = planTime(slides(), 300);

    expect(plan.plannedSeconds).toBe(240);
    expect(plan.spareSeconds).toBe(60);
    expect(plan.overSeconds).toBe(0);
    expect(plan.slides[0]!.share).toBeCloseTo(0.3);
    expect(plan.slotShare).toBe(1);
  });

  it("draws a deck that does not fit past the end of the slot", () => {
    // The slot stops at five sixths of the bar and the slides carry on past
    // it. That overhang is the feature: a speaker sees the overrun.
    const plan = planTime(slides(), 200);

    expect(plan.overSeconds).toBe(40);
    expect(plan.spareSeconds).toBe(0);
    expect(plan.slotShare).toBeCloseTo(200 / 240);
  });

  it("counts the slides marked optional as the slack they are", () => {
    const plan = planTime(slides(), 200);

    expect(plan.slackSeconds).toBe(30);
    expect(plan.slides.filter((slide) => slide.optional)).toHaveLength(1);
  });

  it("keeps the budget as the author typed it rather than as a number", () => {
    // `90s`, `1m30s` and `1:30` are the same budget, and the one in the file
    // is the author's. Rewriting it would make a field they never touched diff.
    expect(planTime(slides(), 300).slides[0]!.written).toBe("90s");
  });

  it("counts the slides that no number accounts for yet", () => {
    const plan = planTime([slideOf(), ...slides()], 300);

    expect(plan.untimed).toBe(1);
    expect(plan.plannedSeconds).toBe(240);
  });

  it("has no bar to draw for a deck with no budgets, no notes and no slot", () => {
    const plan = planTime([slideOf(), slideOf({ index: 1 })], undefined);

    expect(plan.empty).toBe(true);
    expect(plan.slotShare).toBeUndefined();
    expect(plan.slides[0]!.share).toBe(0);
  });

  it("writes a duration the way an author would type one", () => {
    expect(formatSeconds(45)).toBe("45s");
    expect(formatSeconds(90)).toBe("1m30s");
    expect(formatSeconds(1200)).toBe("20m");
    expect(formatSeconds(0)).toBe("0s");
  });
});

describe("the storyboard", () => {
  it("stays out of the way until it is asked for", () => {
    const log = recorder();
    const storyboard = createStoryboard(log);
    storyboard.render(stateOf());

    const launch = storyboard.root.querySelector<HTMLElement>(".slidx-sb-launch")!;
    expect(launch.getAttribute("aria-expanded")).toBe("false");
    expect(rowsIn(storyboard.root)).toHaveLength(0);

    launch.click();
    expect(launch.getAttribute("aria-expanded")).toBe("true");
    expect(rowsIn(storyboard.root)).toHaveLength(3);
  });

  it("is one message per slide, and says so plainly when there is no message", () => {
    // A slide with no notes is a slide whose message has not been written
    // down. Leaving the row blank would read as a slide with nothing to say.
    const { root } = open(stateOf({ slides: [...slides(), slideOf({ id: "end", index: 3 })] }));
    const rows = rowsIn(root);

    expect(rows[1]!.querySelector<HTMLTextAreaElement>(".slidx-sb-message")!.value).toBe(
      "What the parser does.\n\nWhy it matters.",
    );
    expect(rows[1]!.querySelector(".slidx-sb-unwritten")).toBeNull();
    expect(rows[3]!.querySelector(".slidx-sb-unwritten")!.textContent).toContain(
      "No message written down",
    );
  });

  it("draws each slide's width in proportion to the time it is budgeted", () => {
    const { root } = open();

    const segments = [...root.querySelectorAll<HTMLElement>(".slidx-sb-segment")];
    expect(segments.map((segment) => segment.style.width)).toEqual(["30%", "40%", "10%"]);
    expect(segments[1]!.getAttribute("data-source")).toBe("estimate");
  });

  it("marks off the part of the bar that is past the end of the slot", () => {
    // A segment is opaque, so the band ending underneath one is not something a
    // reader can see. Without this the overrun is a 1px line and a sentence.
    const over = open(stateOf({ durationSeconds: 220 }));
    expect(over.root.querySelector<HTMLElement>(".slidx-sb-overrun")!.style.left).toBe("91.6667%");

    const fits = open();
    expect(fits.root.querySelector(".slidx-sb-overrun")).toBeNull();
  });

  it("shows the deck against the slot in words as well as in width", () => {
    const { root } = open();

    expect(root.querySelector(".slidx-sb-summary")!.textContent).toBe(
      "4m planned against a 5m slot, 1m spare.",
    );
  });

  it("names what is prepared to be cut when the deck runs long", () => {
    // "What do I cut" answered before the talk rather than during it.
    const { root } = open(stateOf({ durationSeconds: 220 }));

    expect(root.querySelector(".slidx-sb-summary")!.textContent).toBe(
      "4m planned against a 3m40s slot, 20s over.",
    );
    expect(root.querySelector(".slidx-sb-slack")!.textContent).toBe(
      "Dropping the slide marked optional brings it to 3m30s, which fits.",
    );
  });

  it("does not claim the slack is enough when it is not", () => {
    const { root } = open(stateOf({ durationSeconds: 120 }));

    expect(root.querySelector(".slidx-sb-slack")!.textContent).toBe(
      "Dropping the slide marked optional brings it to 3m30s, still 1m30s over.",
    );
  });

  it("says there is nothing prepared to cut when nothing is marked", () => {
    const bare = slides().map((slide) => ({ ...slide, optional: false }));
    const { root } = open(stateOf({ slides: bare, durationSeconds: 200 }));

    expect(root.querySelector(".slidx-sb-slack")!.textContent).toContain("Nothing is marked");
  });

  it("says how much of the deck no number accounts for yet", () => {
    const { root } = open(stateOf({ slides: [...slides(), slideOf({ id: "end", index: 3 })] }));

    expect(root.querySelector(".slidx-sb-untimed")!.textContent).toContain("1 slide has no");
    expect([...root.querySelectorAll(".slidx-sb-segment")]).toHaveLength(3);
  });

  it("says there is no slot to lay the deck against rather than inventing one", () => {
    const { root } = open(stateOf({ durationSeconds: undefined }));

    expect(root.querySelector(".slidx-sb-summary")!.textContent).toContain("no `duration:`");
    expect(root.querySelector(".slidx-sb-slot")).toBeNull();
  });

  it("moves a slide when it is dropped between two others", () => {
    // `moveSlide` counts the destination after the slide is lifted out, so a
    // gap index is not the operation's index and the difference is a bug.
    const { log, root } = open();

    rowsIn(root)[2]!.dispatchEvent(new Event("dragstart", { bubbles: true }));
    root
      .querySelectorAll<HTMLElement>(".slidx-sb-drop")[1]!
      .dispatchEvent(new Event("drop", { bubbles: true, cancelable: true }));

    expect(log.ops).toEqual([{ op: "moveSlide", slide: 2, to: 1 }]);
  });

  it("spends no operation on a drop back where it came from", () => {
    const { log, root } = open();
    const gaps = [...root.querySelectorAll<HTMLElement>(".slidx-sb-drop")];

    rowsIn(root)[1]!.dispatchEvent(new Event("dragstart", { bubbles: true }));
    gaps[1]!.dispatchEvent(new Event("drop", { bubbles: true, cancelable: true }));
    rowsIn(root)[1]!.dispatchEvent(new Event("dragstart", { bubbles: true }));
    gaps[2]!.dispatchEvent(new Event("drop", { bubbles: true, cancelable: true }));

    expect(log.ops).toEqual([]);
  });

  it("reorders from the keyboard, as one operation", () => {
    const { log, root } = open();

    sheet(root).dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", altKey: true, bubbles: true }),
    );

    expect(log.ops).toEqual([{ op: "moveSlide", slide: 1, to: 2 }]);
  });

  it("does not reorder a slide off either end of the deck", () => {
    const { log, root } = open(stateOf({ selection: { slide: 0 } }));

    sheet(root).dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowUp", altKey: true, bubbles: true }),
    );

    expect(log.ops).toEqual([]);
  });

  it("jumps between slides from the keyboard", () => {
    const { log, root } = open();

    sheet(root).dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    sheet(root).dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }));

    expect(log.selected).toEqual([2, 0]);
  });

  it("jumps to the slide whose row was clicked", () => {
    const { log, root } = open();

    rowsIn(root)[2]!.querySelector<HTMLElement>(".slidx-sb-jump")!.click();

    expect(log.selected).toEqual([2]);
  });

  it("marks a slide safe to drop as one operation, from the row or from the keyboard", () => {
    const { log, root } = open();

    rowsIn(root)[1]!.querySelector<HTMLElement>(".slidx-sb-optional")!.click();
    sheet(root).dispatchEvent(new KeyboardEvent("keydown", { key: "o", bubbles: true }));

    expect(log.ops).toEqual([
      { op: "setField", slide: 1, key: "optional", value: true },
      { op: "setField", slide: 1, key: "optional", value: true },
    ]);
  });

  it("takes a slide back out of the slack it was put in", () => {
    const { log, root } = open();

    rowsIn(root)[2]!.querySelector<HTMLElement>(".slidx-sb-optional")!.click();

    expect(log.ops).toEqual([{ op: "setField", slide: 2, key: "optional", value: false }]);
  });

  it("sends the message as one block when the author leaves the field", () => {
    // On blur rather than on every keystroke: an operation per character would
    // write the file per character, and undo is a list of operations.
    const { log, root } = open();

    const message = rowsIn(root)[1]!.querySelector<HTMLTextAreaElement>(".slidx-sb-message")!;
    message.value = "Say the number first.";
    message.dispatchEvent(new Event("blur"));

    expect(log.ops).toEqual([{ op: "setNotes", slide: 1, notes: "Say the number first." }]);
  });

  it("writes the budget as the author typed it rather than as a number it parsed", () => {
    // `budget:` accepts four notations and Rust reads all of them. A browser
    // that converted the text would be a second duration parser.
    const { log, root } = open();

    const budget = rowsIn(root)[1]!.querySelector<HTMLInputElement>(".slidx-sb-budget")!;
    budget.value = "2m30s";
    budget.dispatchEvent(new Event("blur"));

    expect(log.ops).toEqual([{ op: "setField", slide: 1, key: "budget", value: "2m30s" }]);
  });

  it("writes nothing for a budget the author looked at and left alone", () => {
    const { log, root } = open();

    rowsIn(root)[0]!
      .querySelector<HTMLInputElement>(".slidx-sb-budget")!
      .dispatchEvent(new Event("blur"));

    expect(log.ops).toEqual([]);
  });

  it("removes a budget when the author clears it", () => {
    const { log, root } = open();
    const budget = rowsIn(root)[0]!.querySelector<HTMLInputElement>(".slidx-sb-budget")!;

    budget.value = "";
    budget.dispatchEvent(new Event("blur"));

    expect(log.ops).toEqual([{ op: "removeField", slide: 0, key: "budget" }]);
  });

  it("keeps its keys to itself while a message is being typed", () => {
    // `o` is a letter before it is a shortcut. A tool that steals keys from the
    // field an author is typing in is a tool they fight.
    const { log, root } = open();
    const message = rowsIn(root)[1]!.querySelector<HTMLTextAreaElement>(".slidx-sb-message")!;

    message.dispatchEvent(new KeyboardEvent("keydown", { key: "o", bubbles: true }));
    message.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", altKey: true, bubbles: true }),
    );

    expect(log.ops).toEqual([]);
    expect(log.selected).toEqual([]);
  });

  it("closes on the key every overlay closes on", () => {
    const { root } = open();

    sheet(root).dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));

    expect(root.querySelector(".slidx-sb-launch")!.getAttribute("aria-expanded")).toBe("false");
  });

  it("marks the slide the rest of the editor is on", () => {
    const { root } = open();

    expect(rowsIn(root)[1]!.getAttribute("aria-current")).toBe("true");
    expect(rowsIn(root)[0]!.getAttribute("aria-current")).toBeNull();
  });

  it("draws no thumbnail of any slide", () => {
    // The canvas is the one answer about what a slide looks like. A second
    // preview drawn another way would be a second source of truth about layout.
    const { root } = open();

    expect(root.querySelector("iframe")).toBeNull();
    expect(root.querySelector("img")).toBeNull();
  });

  it("covers what is drawn over the canvas, because it is a mode and not a panel", () => {
    // Both layers are fixed to the whole window, so the order between them is a
    // number rather than a place in the document — and the storyboard was below.
    // A talk's running order with three block grips floating on top of it is
    // what that looks like, which is how `scripts/record-editor.mjs` found it.
    const layer = (stylesheet: string) => Number(/z-index:\s*(\d+)/.exec(stylesheet)![1]);

    expect(layer(STORYBOARD_STYLESHEET)).toBeGreaterThan(layer(ARRANGE_STYLESHEET));
  });
});

function sheet(root: HTMLElement): HTMLElement {
  return root.querySelector<HTMLElement>(".slidx-sb-sheet")!;
}
