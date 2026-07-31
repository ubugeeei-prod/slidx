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
const timing = {
  notes: [],
  stopCount: 1,
  estimatedSeconds: 0,
  optional: false,
  style: {},
  frontmatter: {},
};

function stateOf(over: Partial<EditorState> = {}): EditorState {
  return {
    source: "# One\n\n---\n\n# Two\n\n---\n\n# Three",
    spans: [],
    layouts: [
      {
        id: "full",
        summary: "One region, the whole slide.",
        areas: ["body"],
        columns: "1fr",
        rows: "1fr",
      },
      {
        id: "aside",
        summary: "A main region beside supporting content.",
        areas: ["main side"],
        columns: "2fr 1fr",
        rows: "1fr",
      },
    ],
    slides: [
      { ...timing, id: "one", index: 0, title: "One", frontmatter: { title: "A" } },
      { ...timing, id: "two", index: 1, title: "Two", notes: ["said"] },
      { ...timing, id: "three", index: 2, title: "Three" },
    ],
    diagnostics: [],
    selection: { slide: 1 },
    viewers: [],
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

function taskTab(root: ParentNode, name: "Selection" | "Slide" | "Deck"): HTMLButtonElement {
  return [...root.querySelectorAll<HTMLButtonElement>('[role="tab"]')].find(
    (tab) => tab.textContent === name,
  )!;
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
    expect(rows.map((row) => row.querySelector(".slidx-outline-number")!.textContent)).toEqual([
      "1",
      "2",
      "3",
    ]);
  });

  it("previews every slide through the deck's own lazy, non-interactive page", () => {
    const outline = createOutline(recorder(), { deckBase: "talk" });
    outline.render(stateOf());

    const frames = [...outline.root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")];
    expect(frames.map((frame) => frame.getAttribute("src"))).toEqual([
      "/talk/",
      "/talk/2/",
      "/talk/3/",
    ]);
    for (const frame of frames) {
      expect(frame.getAttribute("loading")).toBe("lazy");
      expect(frame.getAttribute("tabindex")).toBe("-1");
      expect(frame.getAttribute("aria-hidden")).toBe("true");
    }
  });

  it("keeps preview documents mounted across selection and presence updates", () => {
    const outline = createOutline(recorder());
    const state = stateOf();
    outline.render(state);
    const frames = [...outline.root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")];
    const sources = frames.map((frame) => frame.getAttribute("src"));

    outline.render({
      ...state,
      selection: { slide: 2 },
      viewers: [{ id: "seat-2", label: "guest", slide: 0, local: false, canEdit: true }],
    });

    const kept = [...outline.root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")];
    expect(kept).toEqual(frames);
    expect(kept.map((frame) => frame.getAttribute("src"))).toEqual(sources);
    expect(
      outline.root.querySelectorAll(".slidx-outline-row")[2]!.getAttribute("aria-current"),
    ).toBe("true");
  });

  it("refreshes the kept preview nodes when the source changes", () => {
    const outline = createOutline(recorder());
    const state = stateOf();
    outline.render(state);
    const frames = [...outline.root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")];

    outline.render({ ...state, source: `${state.source}\n` });

    const refreshed = [...outline.root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")];
    expect(refreshed).toEqual(frames);
    expect(refreshed.map((frame) => frame.getAttribute("src"))).toEqual([
      "/slides/?outline=1",
      "/slides/2/?outline=1",
      "/slides/3/?outline=1",
    ]);
  });

  it("reorders keyed rows instead of recreating their preview frames", () => {
    const outline = createOutline(recorder());
    const state = stateOf();
    outline.render(state);
    const byId = new Map(
      state.slides.map((slide, index) => [
        slide.id,
        outline.root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")[index]!,
      ]),
    );
    const slides = [state.slides[2]!, state.slides[0]!, state.slides[1]!].map((slide, index) => ({
      ...slide,
      index,
    }));

    outline.render({ ...state, source: `${state.source}\n`, slides });

    expect([...outline.root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")]).toEqual([
      byId.get("three"),
      byId.get("one"),
      byId.get("two"),
    ]);
    expect(
      [...outline.root.querySelectorAll(".slidx-outline-title")].map((title) => title.textContent),
    ).toEqual(["Three", "One", "Two"]);
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

  it("adds, duplicates, and removes slides through whole-slide operations", () => {
    const log = recorder();
    const outline = createOutline(log);
    outline.render(stateOf());

    outline.root.querySelector<HTMLElement>(".slidx-add")!.click();
    outline.root.querySelectorAll<HTMLElement>(".slidx-outline-duplicate")[1]!.click();
    outline.root.querySelectorAll<HTMLElement>(".slidx-outline-remove")[0]!.click();

    expect(log.ops).toEqual([
      { op: "insertSlide", at: 3, body: "## New slide" },
      { op: "duplicateSlide", slide: 1 },
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

  it("forwards keys and clipboard events from the preview and lets them go on teardown", () => {
    const log = recorder();
    const canvas = createCanvas(
      { run: log.run, selected: () => {} },
      { deckBase: "slides", bodyOf: () => "## Two" },
    );
    document.body.append(canvas.root);
    const frame = canvas.root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    let heard = 0;
    let copied = 0;
    let pasted = 0;

    canvas.listen(() => {
      heard += 1;
    });
    canvas.listenClipboard(
      () => {
        copied += 1;
      },
      () => {
        pasted += 1;
      },
    );
    frame.contentDocument!.dispatchEvent(new KeyboardEvent("keydown", { key: "PageDown" }));
    frame.contentDocument!.dispatchEvent(new ClipboardEvent("copy"));
    frame.contentDocument!.dispatchEvent(new ClipboardEvent("paste"));
    canvas.destroy?.();
    frame.contentDocument!.dispatchEvent(new KeyboardEvent("keydown", { key: "PageDown" }));
    frame.contentDocument!.dispatchEvent(new ClipboardEvent("copy"));
    frame.contentDocument!.dispatchEvent(new ClipboardEvent("paste"));
    canvas.root.remove();

    expect(heard).toBe(1);
    expect(copied).toBe(1);
    expect(pasted).toBe(1);
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

  it("presents Selection, Slide, and Deck as one accessible task at a time", () => {
    const inspector = createInspector(recorder(), options);
    inspector.render(stateOf());

    const tablist = inspector.root.querySelector('[role="tablist"]')!;
    const tabs = [...tablist.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
    const panels = [...inspector.root.querySelectorAll<HTMLElement>('[role="tabpanel"]')];

    expect(tablist.getAttribute("aria-label")).toBe("Inspector sections");
    expect(tablist.getAttribute("aria-orientation")).toBe("horizontal");
    expect(tabs.map((tab) => tab.textContent)).toEqual(["Selection", "Slide", "Deck"]);
    expect(tabs.map((tab) => tab.getAttribute("aria-selected"))).toEqual([
      "false",
      "true",
      "false",
    ]);
    expect(tabs.map((tab) => tab.getAttribute("tabindex"))).toEqual(["-1", "0", "-1"]);
    expect(panels.map((panel) => panel.dataset.group)).toEqual(["selection", "slide", "deck"]);
    expect(panels.map((panel) => panel.hidden)).toEqual([true, false, true]);

    for (let index = 0; index < tabs.length; index += 1) {
      expect(tabs[index]!.getAttribute("aria-controls")).toBe(panels[index]!.id);
      expect(panels[index]!.getAttribute("aria-labelledby")).toBe(tabs[index]!.id);
    }
  });

  it("keeps an explicitly chosen task across ordinary renders", () => {
    const inspector = createInspector(recorder(), options);
    const state = stateOf();
    inspector.render(state);
    taskTab(inspector.root, "Deck").click();

    inspector.render({
      ...state,
      viewers: [{ id: "seat-2", label: "guest", slide: 0, local: false, canEdit: true }],
    });

    expect(taskTab(inspector.root, "Deck").getAttribute("aria-selected")).toBe("true");
    expect(inspector.root.querySelector<HTMLElement>('[data-group="deck"]')!.hidden).toBe(false);
  });

  it("brings a new text selection forward and safely returns to the slide", () => {
    const inspector = createInspector(recorder(), options);
    const state = stateOf();
    inspector.render(state);

    const selected = {
      ...state,
      selection: { slide: 1, text: "3.2x faster", range: { start: 23, end: 34 } },
    };
    inspector.render(selected);
    expect(taskTab(inspector.root, "Selection").getAttribute("aria-selected")).toBe("true");

    taskTab(inspector.root, "Slide").click();
    inspector.render(selected);
    expect(taskTab(inspector.root, "Slide").getAttribute("aria-selected")).toBe("true");

    inspector.render({
      ...selected,
      selection: { slide: 1, text: "result", range: { start: 12, end: 18 } },
    });
    expect(taskTab(inspector.root, "Selection").getAttribute("aria-selected")).toBe("true");

    inspector.render(state);
    expect(taskTab(inspector.root, "Slide").getAttribute("aria-selected")).toBe("true");
  });

  it("moves and activates task tabs with arrow, Home, and End keys", () => {
    const inspector = createInspector(recorder(), options);
    inspector.render(stateOf());
    document.body.append(inspector.root);

    const slide = taskTab(inspector.root, "Slide");
    slide.focus();
    slide.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    expect(document.activeElement).toBe(taskTab(inspector.root, "Deck"));

    taskTab(inspector.root, "Deck").dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }),
    );
    expect(document.activeElement).toBe(taskTab(inspector.root, "Selection"));

    taskTab(inspector.root, "Selection").dispatchEvent(
      new KeyboardEvent("keydown", { key: "End", bubbles: true }),
    );
    expect(document.activeElement).toBe(taskTab(inspector.root, "Deck"));

    taskTab(inspector.root, "Deck").dispatchEvent(
      new KeyboardEvent("keydown", { key: "Home", bubbles: true }),
    );
    expect(document.activeElement).toBe(taskTab(inspector.root, "Selection"));
    expect(taskTab(inspector.root, "Selection").getAttribute("aria-selected")).toBe("true");

    inspector.root.remove();
  });

  it("does not commit controls from an inactive task", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf());

    const title = inspector.root.querySelector<HTMLInputElement>(
      '[data-group="deck"] [data-key="title"]',
    )!;
    title.value = "unfinished";
    title.dispatchEvent(new Event("blur"));

    taskTab(inspector.root, "Deck").click();
    const budget = inspector.root.querySelector<HTMLInputElement>(
      '[data-group="slide"] [data-key="budget"]',
    )!;
    const notes = inspector.root.querySelector<HTMLTextAreaElement>(
      '[data-group="slide"] [aria-label="Speaker notes"]',
    )!;
    budget.value = "unfinished";
    notes.value = "unfinished";
    budget.dispatchEvent(new Event("blur"));
    notes.dispatchEvent(new Event("blur"));

    document.body.append(inspector.root);
    title.focus();
    inspector.render(
      stateOf({ selection: { slide: 1, text: "3.2x faster", range: { start: 23, end: 34 } } }),
    );

    expect(log.ops).toEqual([]);
    inspector.root.remove();
  });

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

  it("offers pipeline layouts visually and writes the choice into Markdown style", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf();
    state.slides[1]!.frontmatter = { layout: "aside" };
    inspector.render(state);

    const choices = [
      ...inspector.root.querySelectorAll<HTMLButtonElement>(
        '[data-group="slide"] .slidx-layout-choice',
      ),
    ];
    expect(choices.map((choice) => choice.dataset.layout)).toEqual(["", "full", "aside"]);
    expect(choices[0]!.textContent).toContain("Inherited · aside");
    expect(choices[0]!.getAttribute("aria-pressed")).toBe("true");
    expect(inspector.root.querySelector('[data-group="slide"] .slidx-layout-field')).not.toBeNull();
    expect(document.querySelectorAll("style[data-slidx-layout-picker]")).toHaveLength(1);

    inspector.render(state);
    expect(document.querySelectorAll("style[data-slidx-layout-picker]")).toHaveLength(1);

    inspector.root.querySelectorAll<HTMLButtonElement>(".slidx-layout-choice")[1]!.click();

    expect(log.ops).toEqual([{ op: "setStyle", slide: 1, property: "layout", value: "full" }]);
  });

  it("can remove an explicit layout without reviving a second visual writer", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf({ selection: { slide: 0 } });
    state.slides[0]!.style = { layout: "aside" };
    state.slides[0]!.frontmatter = { title: "A", layout: "full" };
    inspector.render(state);

    expect(
      inspector.root
        .querySelector('[data-group="slide"] [data-layout="aside"]')!
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      inspector.root.querySelector('[data-group="slide"] input[data-key="layout"]'),
    ).toBeNull();
    expect(inspector.root.querySelector('[data-group="deck"] input[data-key="layout"]')).toBeNull();

    inspector.root
      .querySelector<HTMLButtonElement>('[data-group="slide"] [data-layout=""]')!
      .click();

    expect(log.ops).toEqual([{ op: "setStyle", slide: 0, property: "layout" }]);
  });

  it("keeps an installed custom layout visible when it is already in Markdown", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf();
    state.slides[1]!.style = { layout: "conference-grid" };
    inspector.render(state);

    const custom = inspector.root.querySelector<HTMLButtonElement>(
      '[data-group="slide"] [data-layout="conference-grid"]',
    )!;
    expect(custom).not.toBeNull();
    expect(custom.getAttribute("aria-pressed")).toBe("true");
    expect(custom.title).toContain("custom layout");
  });

  it("writes the deck's own keys onto the slide that holds them", () => {
    // A deck and its opening slide share one frontmatter block, which is what
    // the parser already believes.
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf());
    taskTab(inspector.root, "Deck").click();

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

  it("offers theme-aware font, size, and colour choices without losing custom properties", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf({ selection: { slide: 1, text: "3.2x faster" } }));

    const selection = inspector.root.querySelector<HTMLElement>('[data-group="selection"]')!;
    const properties = selection.querySelector<HTMLTextAreaElement>(
      '[aria-label="Style properties"]',
    )!;
    properties.value = "tracking=tight";

    selection
      .querySelector<HTMLButtonElement>('[data-property="font"][data-value="mono"]')!
      .click();
    selection
      .querySelector<HTMLButtonElement>('[data-property="size"][data-value="heading-2"]')!
      .click();
    selection
      .querySelector<HTMLButtonElement>('[data-property="color"][data-value="accent"]')!
      .click();

    expect(properties.value).toBe("tracking=tight\nfont=mono\nsize=heading-2\ncolor=accent");
    expect(
      selection
        .querySelector('[data-property="font"][data-value="mono"]')!
        .getAttribute("aria-pressed"),
    ).toBe("true");

    selection.querySelector<HTMLElement>(".slidx-add")!.click();
    expect(log.ops.at(-1)).toMatchObject({
      op: "addMark",
      attributes: {
        properties: { tracking: "tight", font: "mono", size: "heading-2", color: "accent" },
      },
    });
  });

  it("can return a visual property to its inherited value", () => {
    const log = recorder();
    const inspector = createInspector(log, {
      bodyOf: () => "A [styled phrase]{font=mono color=muted}.",
      blocksOf: () => [
        {
          span: { start: 0, end: 43 },
          marks: [
            {
              span: { start: 2, end: 42 },
              words: { start: 3, end: 16 },
              properties: { font: "mono", color: "muted" },
            },
          ],
        },
      ],
    });
    inspector.render(
      stateOf({ selection: { slide: 1, text: "styled phrase", range: { start: 3, end: 16 } } }),
    );

    const selection = inspector.root.querySelector<HTMLElement>('[data-group="selection"]')!;
    expect(
      selection
        .querySelector('[data-property="font"][data-value="mono"]')!
        .getAttribute("aria-pressed"),
    ).toBe("true");
    selection.querySelector<HTMLButtonElement>('[data-property="font"][data-value=""]')!.click();
    selection.querySelector<HTMLElement>(".slidx-add")!.click();

    expect(log.ops.at(-1)).toMatchObject({
      op: "setMark",
      attributes: { properties: { color: "muted" } },
    });
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
