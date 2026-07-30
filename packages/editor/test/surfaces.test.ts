/**
 * The three surfaces, judged by the operations they produce.
 *
 * Every one of these asserts an `EditOp` rather than a rendered string, because
 * an operation is the only thing the editor is allowed to make. A surface that
 * produced Markdown would pass a snapshot test and break the promise the whole
 * project is sold on.
 */

import { describe, expect, it } from "vite-plus/test";

import { attachEditing, createCanvas, routeFor } from "../src/canvas";
import { createDiagnostics } from "../src/diagnostics";
import { createInspector } from "../src/inspector";
import { createOutline } from "../src/outline";
import type { EditOp } from "../src/operations";
import type { EditorState } from "../src/session";

/** What every slide here shares, so a row says only what it is about. */
const timing = { notes: [], stopCount: 1, estimatedSeconds: 0, optional: false, frontmatter: {} };

function stateOf(over: Partial<EditorState> = {}): EditorState {
  return {
    source: "# One\n\n---\n\n# Two\n\n---\n\n# Three",
    spans: [],
    slides: [
      { ...timing, id: "one", index: 0, title: "One", frontmatter: { title: "A" } },
      { ...timing, id: "two", index: 1, title: "Two", notes: ["said"] },
      { ...timing, id: "three", index: 2, title: "Three" },
    ],
    diagnostics: [],
    selection: { slide: 1 },
    canUndo: false,
    canRedo: false,
    ...over,
  };
}

function recorder() {
  const ops: EditOp[] = [];
  const selected: number[] = [];

  return {
    ops,
    selected,
    run: (op: EditOp) => ops.push(op),
    select: (at: number) => selected.push(at),
  };
}

describe("the outline", () => {
  it("lists the deck and marks the slide being edited", () => {
    const log = recorder();
    const outline = createOutline(log);
    outline.render(stateOf());

    const rows = [...outline.root.querySelectorAll(".slidx-outline-row")];
    expect(rows.map((row) => row.querySelector(".slidx-outline-title")!.textContent)).toEqual([
      "One",
      "Two",
      "Three",
    ]);
    expect(rows[1]!.getAttribute("aria-current")).toBe("true");
  });

  it("jumps to a slide when its row is clicked", () => {
    const log = recorder();
    const outline = createOutline(log);
    outline.render(stateOf());

    outline.root.querySelectorAll<HTMLElement>(".slidx-outline-open")[2]!.click();

    expect(log.selected).toEqual([2]);
  });

  it("reorders by moving a slide rather than by rewriting one", () => {
    // The whole reason `moveSlide` exists: the bytes that move are the ones
    // already in the file, so a reordered deck diffs as moved lines.
    const log = recorder();
    const outline = createOutline(log);
    outline.render(stateOf());

    const rows = outline.root.querySelectorAll<HTMLElement>(".slidx-outline-row");
    rows[2]!.dispatchEvent(new Event("dragstart", { bubbles: true }));
    rows[0]!.dispatchEvent(new Event("drop", { bubbles: true, cancelable: true }));

    expect(log.ops).toEqual([{ op: "moveSlide", slide: 2, to: 0 }]);
  });

  it("does not spend an operation on a drag that ended where it started", () => {
    const log = recorder();
    const outline = createOutline(log);
    outline.render(stateOf());

    const rows = outline.root.querySelectorAll<HTMLElement>(".slidx-outline-row");
    rows[1]!.dispatchEvent(new Event("dragstart", { bubbles: true }));
    rows[1]!.dispatchEvent(new Event("drop", { bubbles: true, cancelable: true }));

    expect(log.ops).toEqual([]);
  });

  it("adds a slide at the end and removes the one that was asked for", () => {
    const log = recorder();
    const outline = createOutline(log);
    outline.render(stateOf());

    outline.root.querySelector<HTMLElement>(".slidx-add")!.click();
    outline.root.querySelectorAll<HTMLElement>(".slidx-outline-remove")[0]!.click();

    expect(log.ops).toEqual([
      { op: "insertSlide", at: 3, body: "## New slide" },
      { op: "removeSlide", slide: 0 },
    ]);
  });

  it("shows which slide the linter has something to say about", () => {
    const log = recorder();
    const outline = createOutline(log);
    outline.render(
      stateOf({
        diagnostics: [
          { severity: "error", code: "contrast/ratio", message: "too low", slideIndex: 2 },
        ],
      }),
    );

    const rows = outline.root.querySelectorAll(".slidx-outline-row");
    expect(rows[2]!.getAttribute("data-severity")).toBe("error");
    expect(rows[0]!.querySelector(".slidx-dot")).toBeNull();
  });
});

describe("the canvas", () => {
  it("shows the deck's own page rather than a second renderer's", () => {
    // The preview is the route the build emits, produced by the same module.
    // Anything else would be a second answer about what a slide looks like.
    expect(routeFor("slides", 0)).toBe("/slides/");
    expect(routeFor("slides", 3)).toBe("/slides/4/");
    expect(routeFor("", 1)).toBe("/2/");
  });

  it("puts the slide's Markdown in front of the author, byte for byte", () => {
    const log = recorder();
    const canvas = createCanvas(
      { run: log.run, selected: () => {} },
      { deckBase: "slides", bodyOf: () => "## Two\n\n*  a point" },
    );
    canvas.render(stateOf());

    const source = canvas.root.querySelector<HTMLTextAreaElement>(".slidx-canvas-source")!;
    expect(source.value).toBe("## Two\n\n*  a point");
  });

  it("sends the body as an operation when the author leaves it", () => {
    const log = recorder();
    const canvas = createCanvas(
      { run: log.run, selected: () => {} },
      { deckBase: "slides", bodyOf: () => "## Two" },
    );
    canvas.render(stateOf());

    const source = canvas.root.querySelector<HTMLTextAreaElement>(".slidx-canvas-source")!;
    source.value = "## Two\n\nA point.";
    source.dispatchEvent(new Event("blur"));

    expect(log.ops).toEqual([{ op: "setBody", slide: 1, body: "## Two\n\nA point." }]);
  });

  it("writes nothing when the author looked at the Markdown and changed nothing", () => {
    const log = recorder();
    const canvas = createCanvas(
      { run: log.run, selected: () => {} },
      { deckBase: "slides", bodyOf: () => "## Two" },
    );
    canvas.render(stateOf());

    canvas.root
      .querySelector<HTMLTextAreaElement>(".slidx-canvas-source")!
      .dispatchEvent(new Event("blur"));

    expect(log.ops).toEqual([]);
  });

  it("offers no line to edit on a page that is not a slide", () => {
    // Before the frame has loaded, and on a route the deck does not serve.
    // Editing what a slide's lines are is `typing.test.ts`.
    const log = recorder();
    const page = document.implementation.createHTMLDocument();
    page.body.innerHTML = "<p>Not a deck.</p>";

    attachEditing(
      page,
      1,
      { run: log.run, selected: () => {} },
      { body: () => "", blocks: () => [] },
    );

    expect(page.querySelector("[contenteditable]")).toBeNull();
  });
});

describe("the inspector", () => {
  const options = { bodyOf: () => "## Two\n\nThe result was 3.2x faster." };

  it("writes one frontmatter key at a time", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf());

    const budget = inspector.root.querySelector<HTMLInputElement>(
      '[data-group="slide"] [data-key="budget"]',
    )!;
    budget.value = "90s";
    budget.dispatchEvent(new Event("blur"));

    expect(log.ops).toEqual([{ op: "setField", slide: 1, key: "budget", value: "90s" }]);
  });

  it("writes the deck's own keys onto the slide that holds them", () => {
    // A deck and its opening slide share one frontmatter block, which is what
    // the parser already believes.
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf());

    const title = inspector.root.querySelector<HTMLInputElement>(
      '[data-group="deck"] [data-key="title"]',
    )!;
    expect(title.value).toBe("A");

    title.value = "Making Decks Fast";
    title.dispatchEvent(new Event("blur"));

    expect(log.ops).toEqual([
      { op: "setField", slide: 0, key: "title", value: "Making Decks Fast" },
    ]);
  });

  it("shows a key that holds a list without offering to retype it", () => {
    // `steps:` is a list, and a text box that committed what it holds would
    // replace an author's whole timeline with the string `[object Object]`.
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf();
    state.slides[1]!.frontmatter = { steps: [{ reveal: ".a" }, { hide: ".b" }] };
    inspector.render(state);

    const steps = inspector.root.querySelector<HTMLInputElement>(
      '[data-group="slide"] [data-key="steps"]',
    )!;
    expect(steps.value).toBe("2 entries");
    expect(steps.hasAttribute("readonly")).toBe(true);

    steps.value = "wrecked";
    steps.dispatchEvent(new Event("blur"));
    expect(log.ops).toEqual([]);
  });

  it("keeps a key slidx has never heard of rather than losing it", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf();
    state.slides[1]!.frontmatter = { sponsor: "Someone" };
    inspector.render(state);

    expect(
      inspector.root.querySelector<HTMLInputElement>('[data-group="slide"] [data-key="sponsor"]')!
        .value,
    ).toBe("Someone");
  });

  it("does not repeat the deck's keys under the slide that holds them", () => {
    // A deck and its opening slide share one block, so everything written in
    // it would otherwise appear twice, one heading apart.
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf({ selection: { slide: 0 } }));

    expect(inspector.root.querySelector('[data-group="slide"] [data-key="title"]')).toBeNull();
    expect(inspector.root.querySelector('[data-group="deck"] [data-key="title"]')).not.toBeNull();
  });

  it("turns a selection into a style mark rather than into markup", () => {
    // "Select text, add animation" is spelled `addMark` in a file. The range is
    // bytes of the slide's source body; the editor never writes the brackets.
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf({ selection: { slide: 1, text: "3.2x faster" } }));

    const classes = inspector.root.querySelector<HTMLInputElement>(
      '[data-group="selection"] input',
    )!;
    classes.value = "accent";
    const properties = inspector.root.querySelector<HTMLTextAreaElement>(
      '[aria-label="Style properties"]',
    )!;
    properties.value = "color=danger\nfont=IBM Plex";
    inspector.root.querySelector<HTMLElement>('[data-group="selection"] .slidx-add')!.click();

    expect(log.ops).toEqual([
      {
        op: "addMark",
        slide: 1,
        range: { start: 23, end: 34 },
        attributes: {
          key: undefined,
          classes: ["accent"],
          properties: { color: "danger", font: "IBM Plex" },
        },
      },
    ]);
  });

  it("updates and removes the style already wrapped around the selected phrase", () => {
    const log = recorder();
    const body = "## Two\n\nThe result was [3.2x faster]{#result .accent color=danger}.";
    const inspector = createInspector(log, {
      bodyOf: () => body,
      blocksOf: () => [
        {
          span: { start: 8, end: 68 },
          marks: [
            {
              span: { start: 22, end: 66 },
              words: { start: 23, end: 34 },
              key: "result",
              classes: ["accent"],
              properties: { color: "danger" },
            },
          ],
        },
      ],
    });
    inspector.render(
      stateOf({
        selection: { slide: 1, text: "3.2x faster", range: { start: 23, end: 34 } },
      }),
    );

    const selection = inspector.root.querySelector<HTMLElement>('[data-group="selection"]')!;
    const inputs = selection.querySelectorAll<HTMLInputElement>("input");
    expect(inputs[0]!.value).toBe("accent");
    expect(inputs[1]!.value).toBe("result");
    expect(
      selection.querySelector<HTMLTextAreaElement>('[aria-label="Style properties"]')!.value,
    ).toBe("color=danger");

    inputs[0]!.value = "hero strong";
    inputs[1]!.value = "";
    selection.querySelector<HTMLTextAreaElement>('[aria-label="Style properties"]')!.value =
      "color=brand\nfont=IBM Plex";
    selection.querySelector<HTMLElement>(".slidx-add")!.click();
    selection.querySelector<HTMLElement>(".slidx-remove-mark")!.click();

    expect(log.ops).toEqual([
      {
        op: "setMark",
        slide: 1,
        mark: 0,
        attributes: {
          key: undefined,
          classes: ["hero", "strong"],
          properties: { color: "brand", font: "IBM Plex" },
        },
      },
      { op: "removeMark", slide: 1, mark: 0 },
    ]);
  });

  it("refuses to nest a style when only part of an existing mark is selected", () => {
    const log = recorder();
    const inspector = createInspector(log, {
      bodyOf: () => "A [styled phrase]{.accent}.",
      blocksOf: () => [
        {
          span: { start: 0, end: 26 },
          marks: [
            {
              span: { start: 2, end: 25 },
              words: { start: 3, end: 16 },
              classes: ["accent"],
            },
          ],
        },
      ],
    });
    inspector.render(
      stateOf({ selection: { slide: 1, text: "styled", range: { start: 3, end: 9 } } }),
    );

    expect(
      inspector.root.querySelector('[data-group="selection"] .slidx-hint')!.textContent,
    ).toContain("whole styled phrase");
    expect(inspector.root.querySelector('[data-group="selection"] .slidx-add')).toBeNull();
    expect(log.ops).toEqual([]);
  });

  it("says a selection cannot be addressed rather than guessing at it", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf({ selection: { slide: 1, text: "words the source never had" } }));

    expect(
      inspector.root.querySelector('[data-group="selection"] .slidx-hint')!.textContent,
    ).toContain("cannot be addressed");
    expect(inspector.root.querySelector('[data-group="selection"] .slidx-add')).toBeNull();
  });

  it("sends notes as one block when the author leaves the field", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf());

    const notes = inspector.root.querySelector<HTMLTextAreaElement>(
      '[aria-label="Speaker notes"]',
    )!;
    expect(notes.value).toBe("said");

    notes.value = "Open with the outcome.";
    notes.dispatchEvent(new Event("blur"));

    expect(log.ops).toEqual([{ op: "setNotes", slide: 1, notes: "Open with the outcome." }]);
  });

  it("writes a flag as a flag rather than as the word for one", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf());

    const optional = inspector.root.querySelector<HTMLInputElement>(
      '[data-group="slide"] [data-key="optional"]',
    )!;
    optional.value = "true";
    optional.dispatchEvent(new Event("blur"));

    expect(log.ops).toEqual([{ op: "setField", slide: 1, key: "optional", value: true }]);
  });
});

describe("diagnostics", () => {
  it("puts the slide being edited first and says which slide the rest are about", () => {
    const diagnostics = createDiagnostics({ select: () => {} });
    diagnostics.render(
      stateOf({
        diagnostics: [
          { severity: "warning", code: "a11y/alt", message: "no alt text", slideIndex: 2 },
          { severity: "error", code: "contrast/ratio", message: "too low", slideIndex: 1 },
        ],
      }),
    );

    const rows = [...diagnostics.root.querySelectorAll(".slidx-finding")];
    expect(rows.map((row) => row.querySelector(".slidx-finding-where")!.textContent)).toEqual([
      "slide 2",
      "slide 3",
    ]);
  });

  it("jumps to the slide a finding is about", () => {
    const jumped: number[] = [];
    const diagnostics = createDiagnostics({ select: (at) => jumped.push(at) });
    diagnostics.render(
      stateOf({
        diagnostics: [{ severity: "warning", code: "a11y/alt", message: "no alt", slideIndex: 2 }],
      }),
    );

    diagnostics.root.querySelector<HTMLElement>(".slidx-finding")!.click();

    expect(jumped).toEqual([2]);
  });

  it("says a refusal in words instead of throwing it away", () => {
    const diagnostics = createDiagnostics({ select: () => {} });
    diagnostics.render(stateOf({ refusal: { error: "noSuchSlide", slide: 9 } }));

    expect(diagnostics.root.textContent).toContain("no such slide: 9");
  });

  it("stays out of the way when there is nothing to say", () => {
    const diagnostics = createDiagnostics({ select: () => {} });
    diagnostics.render(stateOf());

    expect(diagnostics.root.getAttribute("data-empty")).toBe("true");
  });

  it("puts a finding about something that has not happened yet above the rest", () => {
    // A block being dragged has a landing before it has a line in the file, and
    // a warning an author can act on by not letting go is worth more than the
    // same warning once they did.
    const diagnostics = createDiagnostics({ select: () => {} });
    diagnostics.render(
      stateOf({
        diagnostics: [
          { severity: "warning", code: "a11y/alt", message: "no alt text", slideIndex: 1 },
        ],
        foreseen: [
          { severity: "error", code: "overflow/clipped", message: "loses its right edge" },
        ],
      }),
    );

    const rows = [...diagnostics.root.querySelectorAll(".slidx-finding")];
    expect(rows[0]!.textContent).toContain("on landing");
    expect(rows[0]!.textContent).toContain("loses its right edge");
    expect(rows[1]!.textContent).toContain("no alt text");
  });
});
