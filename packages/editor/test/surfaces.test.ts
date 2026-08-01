/**
 * The three surfaces, judged by the operations they produce.
 *
 * Every one of these asserts an `EditOp` rather than a rendered string, because
 * an operation is the only thing the editor is allowed to make. A surface that
 * produced Markdown would pass a snapshot test and break the promise the whole
 * project is sold on.
 */

import { describe, expect, it } from "vite-plus/test";

import { attachEditing, createCanvas, routeFor, showsRoute } from "../src/canvas";
import { createDiagnostics } from "../src/diagnostics";
import { createInspector } from "../src/inspector";
import { applyThumbnailScheme, createOutline, recoverPreviewRoute } from "../src/outline";
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

const themes = [
  {
    id: "minimal",
    name: "Minimal",
    description: "Neutral and quiet.",
    light: {
      surface: "#ffffff",
      text: "#20242a",
      muted: "#737983",
      heading: "#101419",
      accent: "#1755aa",
      codeSurface: "#eef2f7",
      codeText: "#20242a",
    },
    dark: {
      surface: "#20242a",
      text: "#eaeff7",
      muted: "#a1a6ad",
      heading: "#ffffff",
      accent: "#b6d4ff",
      codeSurface: "#101419",
      codeText: "#eaeff7",
    },
    fontSans: "system-ui",
    fontMono: "monospace",
  },
  {
    id: "editorial",
    name: "Editorial",
    description: "Warm and prose-led.",
    light: {
      surface: "#fffaf6",
      text: "#291f19",
      muted: "#867a73",
      heading: "#1a120c",
      accent: "#844100",
      codeSurface: "#f4ebe4",
      codeText: "#291f19",
    },
    dark: {
      surface: "#291f19",
      text: "#f8ece4",
      muted: "#b0a59d",
      heading: "#ffffff",
      accent: "#ffc7a2",
      codeSurface: "#1a120c",
      codeText: "#f8ece4",
    },
    fontSans: "serif",
    fontMono: "monospace",
  },
];

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
    activeTheme: "minimal",
    themeLocked: false,
    themes,
    transitions: [
      {
        id: "none",
        name: "Cut",
        description: "Instant, with no captured animation.",
        moves: false,
      },
      {
        id: "fade",
        name: "Fade",
        description: "Blend softly between the two slides.",
        moves: false,
      },
      {
        id: "slide",
        name: "Slide",
        description: "Bring the next slide over the current one.",
        moves: true,
      },
      {
        id: "push",
        name: "Push",
        description: "Move both slides together to show progression.",
        moves: true,
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
    writing: false,
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

  it("shows the real deck routes as lazy visual thumbnails", () => {
    const log = recorder();
    const outline = createOutline(log, { preview: (slide) => `/slides/${slide + 1}/` });
    outline.render(stateOf());

    const frames = [...outline.root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")];

    expect(frames.map((frame) => frame.dataset.preview)).toEqual([
      "/slides/1/",
      "/slides/2/",
      "/slides/3/",
    ]);
    expect(frames.every((frame) => frame.getAttribute("loading") === "lazy")).toBe(true);
    expect(frames.every((frame) => frame.tabIndex === -1)).toBe(true);
  });

  it("keeps loaded thumbnails when only editor state changes", () => {
    const log = recorder();
    const outline = createOutline(log, { preview: (slide) => `/slides/${slide + 1}/` });
    const first = stateOf();
    outline.render(first);
    const frames = [...outline.root.querySelectorAll(".slidx-outline-frame")];

    outline.render(stateOf({ selection: { slide: 2 } }));

    expect([...outline.root.querySelectorAll(".slidx-outline-frame")]).toEqual(frames);
    expect(
      outline.root.querySelectorAll(".slidx-outline-row")[2]!.getAttribute("aria-current"),
    ).toBe("true");
  });

  it("brings a newly selected card into the visible part of a long outline", () => {
    const log = recorder();
    const outline = createOutline(log);
    outline.render(stateOf());
    const last = outline.root.querySelectorAll<HTMLElement>(".slidx-outline-row")[2]!;
    let reveals = 0;
    last.scrollIntoView = () => {
      reveals += 1;
    };

    outline.render(stateOf({ selection: { slide: 2 } }));
    outline.render(stateOf({ selection: { slide: 2 }, canUndo: true }));

    expect(reveals).toBe(1);
  });

  it("keeps thumbnail previews on the same paper palette as the canvas", () => {
    const page = document.implementation.createHTMLDocument();

    applyThumbnailScheme(page, "light");

    expect(page.documentElement.dataset.scheme).toBe("light");
  });

  it("keeps the overview palette synchronized with the canvas choice", () => {
    const page = document.implementation.createHTMLDocument();
    applyThumbnailScheme(page, "dark");
    expect(page.documentElement.dataset.scheme).toBe("dark");

    applyThumbnailScheme(page, "auto");
    expect(page.documentElement.hasAttribute("data-scheme")).toBe(false);
  });

  it("returns a miniature to the route its card owns after a document reload", () => {
    let assigned = "";
    const frame = {
      dataset: { preview: "/slides/2/" },
      ownerDocument: { baseURI: "http://localhost:5173/__slidx/" },
      contentDocument: { URL: "http://localhost:5173/slides/" },
      set src(value: string) {
        assigned = value;
      },
    } as unknown as HTMLIFrameElement;

    expect(recoverPreviewRoute(frame)).toBe(true);
    expect(assigned).toBe("/slides/2/");
  });

  it("leaves a miniature alone when only its query changed", () => {
    const frame = {
      dataset: { preview: "/slides/2/" },
      ownerDocument: { baseURI: "http://localhost:5173/__slidx/" },
      contentDocument: { URL: "http://localhost:5173/slides/2/?at=now" },
      set src(_value: string) {
        throw new Error("the matching frame must not navigate");
      },
    } as unknown as HTMLIFrameElement;

    expect(recoverPreviewRoute(frame)).toBe(false);
  });

  it("jumps to a slide when its row is clicked", () => {
    const log = recorder();
    const outline = createOutline(log);
    outline.render(stateOf());

    const open = outline.root.querySelectorAll<HTMLElement>(".slidx-outline-open")[2]!;
    expect(open.getAttribute("aria-label")).toBe("3 Three");
    open.click();

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

    outline.root.querySelector<HTMLElement>(".slidx-slide-add-toggle")!.click();
    outline.root.querySelector<HTMLElement>('[data-slide-kind="title-body"]')!.click();
    outline.root.querySelectorAll<HTMLElement>(".slidx-outline-duplicate")[1]!.click();
    outline.root.querySelectorAll<HTMLElement>(".slidx-outline-remove")[0]!.click();

    expect(log.ops).toEqual([
      { op: "createSlide", at: 2, kind: "title-body" },
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
    expect(rows[0]!.querySelector<HTMLElement>(".slidx-dot")!.hidden).toBe(true);
  });
});

describe("the canvas", () => {
  it("shows the deck's own page rather than a second renderer's", () => {
    // The preview is the route the build emits, produced by the same module.
    // Anything else would be a second answer about what a slide looks like.
    expect(routeFor("slides", 0)).toBe("/slides/");
    expect(routeFor("slides", 3)).toBe("/slides/4/");
    expect(routeFor("", 1)).toBe("/2/");
    expect(
      showsRoute(
        "http://localhost:5173/slides/2/?at=now",
        "/slides/2/",
        "http://localhost:5173/__slidx/",
      ),
    ).toBe(true);
    expect(
      showsRoute("http://localhost:5173/slides/", "/slides/2/", "http://localhost:5173/__slidx/"),
    ).toBe(false);
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

  it("forwards keys from the preview document and lets them go on teardown", () => {
    const log = recorder();
    const canvas = createCanvas(
      { run: log.run, selected: () => {} },
      { deckBase: "slides", bodyOf: () => "## Two" },
    );
    document.body.append(canvas.root);
    const frame = canvas.root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    let heard = 0;

    canvas.listen(() => {
      heard += 1;
    });
    frame.contentDocument!.dispatchEvent(new KeyboardEvent("keydown", { key: "PageDown" }));
    canvas.destroy?.();
    frame.contentDocument!.dispatchEvent(new KeyboardEvent("keydown", { key: "PageDown" }));
    canvas.root.remove();

    expect(heard).toBe(1);
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

  it("shows source without making it writable through a view-only link", () => {
    const log = recorder();
    const canvas = createCanvas(
      { run: log.run, selected: () => {} },
      { deckBase: "slides", bodyOf: () => "## Two" },
    );
    canvas.render(stateOf({ canEdit: false }));

    const source = canvas.root.querySelector<HTMLTextAreaElement>(".slidx-canvas-source")!;
    expect(source.readOnly).toBe(true);
    expect(canvas.root.querySelector<HTMLButtonElement>(".slidx-content-toggle")!.disabled).toBe(
      true,
    );

    source.value = "## Changed";
    source.dispatchEvent(new Event("blur"));
    expect(log.ops).toEqual([]);
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

  it("adds common content after the selected block through a semantic operation", () => {
    const log = recorder();
    const blocks = [
      { span: { start: 0, end: 6 }, marks: [] },
      { span: { start: 8, end: 13 }, marks: [] },
    ];
    const canvas = createCanvas(
      { run: log.run, selected: () => {} },
      { deckBase: "slides", bodyOf: () => "## Two\n\nPoint", blocksOf: () => blocks },
    );
    canvas.render(stateOf({ selection: { slide: 1, block: 0 } }));

    canvas.root.querySelector<HTMLButtonElement>(".slidx-content-toggle")!.click();
    canvas.root.querySelector<HTMLButtonElement>('[data-kind="quote"]')!.click();

    expect(log.ops).toEqual([{ op: "insertBlock", slide: 1, at: 1, kind: "quote" }]);
    canvas.destroy?.();
  });

  it("waits for an inserted placeholder before handing it the caret", async () => {
    let source = "## Two\n\nPoint";
    let blocks = [
      { span: { start: 0, end: 6 }, marks: [] },
      { span: { start: 8, end: 13 }, marks: [] },
    ];
    let release = () => {};
    const selected: number[] = [];
    const canvas = createCanvas(
      {
        run: () =>
          new Promise<void>((resolve) => {
            release = resolve;
          }),
        selected: () => {},
        selectedBlock: (block) => {
          if (block !== undefined) selected.push(block);
        },
      },
      { deckBase: "slides", bodyOf: () => source, blocksOf: () => blocks },
    );
    document.body.append(canvas.root);
    canvas.render(stateOf({ selection: { slide: 1, block: 1 } }));
    const frame = canvas.root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    frame.removeAttribute("src");
    frame.contentDocument!.body.innerHTML = `
      <div class="slidx-slide-body">
        <div data-slidx-block="0"><h2>Two</h2></div>
        <div data-slidx-block="1"><p>Point</p></div>
      </div>
    `;
    frame.dispatchEvent(new Event("load"));

    canvas.insertContent("quote");
    // The session can announce the edit before the live replacement has put
    // block two into the frame. This load must not consume the focus request.
    frame.dispatchEvent(new Event("load"));
    source = "## Two\n\nPoint\n\n> Key takeaway";
    blocks = [
      { span: { start: 0, end: 6 }, marks: [] },
      { span: { start: 8, end: 13 }, marks: [] },
      { span: { start: 15, end: 29 }, marks: [] },
    ];
    frame.contentDocument!.body.innerHTML = `
      <div class="slidx-slide-body">
        <div data-slidx-block="0"><h2>Two</h2></div>
        <div data-slidx-block="1"><p>Point</p></div>
        <div data-slidx-block="2"><blockquote><p>Key takeaway</p></blockquote></div>
      </div>
    `;
    release();
    await Promise.resolve();
    frame.dispatchEvent(new Event("load"));

    expect(frame.contentDocument!.activeElement?.textContent).toBe("Key takeaway");
    expect(frame.contentDocument!.getSelection()?.toString()).toBe("Key takeaway");
    expect(selected).toEqual([2]);

    frame.contentDocument!.body.tabIndex = -1;
    frame.contentDocument!.body.focus();
    frame.dispatchEvent(new Event("load"));
    expect(frame.contentDocument!.activeElement?.textContent).toBe("Key takeaway");

    frame.contentDocument!.activeElement!.dispatchEvent(
      new InputEvent("beforeinput", { bubbles: true, inputType: "insertText", data: "A" }),
    );
    frame.contentDocument!.body.focus();
    frame.dispatchEvent(new Event("load"));
    expect(frame.contentDocument!.activeElement).toBe(frame.contentDocument!.body);
    canvas.destroy?.();
    canvas.root.remove();
  });

  it("selects the first placeholder when a newly created slide finishes loading", () => {
    const log = recorder();
    const canvas = createCanvas(
      { run: log.run, selected: () => {} },
      {
        deckBase: "slides",
        bodyOf: () => "## New slide",
        blocksOf: () => [{ span: { start: 0, end: 12 }, marks: [] }],
      },
    );
    document.body.append(canvas.root);
    const frame = canvas.root.querySelector<HTMLIFrameElement>(".slidx-canvas-frame")!;
    frame.removeAttribute("src");
    frame.contentDocument!.body.innerHTML = `
      <div class="slidx-slide-body">
        <div data-slidx-block="0"><h2>New slide</h2></div>
      </div>
    `;

    canvas.focusFresh();
    frame.dispatchEvent(new Event("load"));

    expect(frame.contentDocument!.activeElement?.textContent).toBe("New slide");
    expect(frame.contentDocument!.getSelection()?.toString()).toBe("New slide");

    frame.contentDocument!.body.tabIndex = -1;
    frame.contentDocument!.body.focus();
    frame.dispatchEvent(new Event("load"));
    expect(frame.contentDocument!.activeElement?.textContent).toBe("New slide");

    frame.contentDocument!.activeElement!.dispatchEvent(
      new InputEvent("beforeinput", { bubbles: true, inputType: "insertText", data: "A" }),
    );
    frame.contentDocument!.body.focus();
    frame.dispatchEvent(new Event("load"));
    expect(frame.contentDocument!.activeElement).toBe(frame.contentDocument!.body);

    canvas.focusFresh();
    expect(frame.contentDocument!.activeElement?.textContent).toBe("New slide");
    expect(frame.contentDocument!.getSelection()?.toString()).toBe("New slide");
    canvas.destroy?.();
    canvas.root.remove();
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

  it("opens on the most specific target and keeps the chosen context until selection changes", () => {
    const inspector = createInspector(recorder(), options);
    const state = stateOf({ selection: { slide: 1, block: 0 } });
    inspector.render(state);

    expect(inspector.root.querySelector('[data-tab="block"]')!.getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(inspector.root.querySelector('[data-panel="block"]')!.hasAttribute("hidden")).toBe(
      false,
    );

    inspector.root.querySelector<HTMLButtonElement>('[data-tab="deck"]')!.click();
    inspector.render(state);
    expect(inspector.root.querySelector('[data-tab="deck"]')!.getAttribute("aria-selected")).toBe(
      "true",
    );

    inspector.render(stateOf({ selection: { slide: 2, block: 0 } }));
    expect(inspector.root.querySelector('[data-tab="block"]')!.getAttribute("aria-selected")).toBe(
      "true",
    );
  });

  it("edits a selected block through semantic position, size, colour, and identity operations", () => {
    const ops: EditOp[] = [];
    const inspector = createInspector(
      { run: (op) => ops.push(op) },
      {
        bodyOf: () =>
          "## Two\n\nThe result was 3.2x faster.\n\n<!-- notes: source-only context -->",
        blocksOf: () => [
          {
            span: { start: 0, end: 72 },
            attributes: {
              key: "result",
              classes: ["accent"],
              properties: { tone: "strong" },
            },
          },
        ],
        geometry: () => ({
          slide: { left: 0, top: 0, width: 800, height: 450 },
          safe: { left: 40, top: 40, width: 720, height: 370 },
          regions: [
            {
              name: "main",
              rect: { left: 40, top: 40, width: 460, height: 370 },
              blocks: [0],
              contentHeight: 80,
              gap: 16,
            },
            {
              name: "side",
              rect: { left: 516, top: 40, width: 244, height: 370 },
              blocks: [],
              contentHeight: 0,
              gap: 16,
            },
          ],
          blocks: [
            {
              index: 0,
              region: "main",
              rect: { left: 40, top: 40, width: 230, height: 80 },
              needsWidth: 0,
              width: "half",
            },
          ],
        }),
        visualOf: () => ({
          color: "rgb(22, 27, 34)",
          managedColor: false,
          palette: [
            { name: "text", label: "Text", color: "#161b22" },
            { name: "heading", label: "Heading", color: "#0d1218" },
            { name: "muted", label: "Muted", color: "#5f656e" },
            { name: "accent", label: "Accent", color: "#01489f" },
          ],
        }),
      },
    );
    const state = stateOf({ selection: { slide: 1, block: 0 } });
    state.slides[1]!.style = { layout: "aside" };
    inspector.render(state);

    expect(inspector.root.querySelector('[data-panel="block"]')!.textContent).toContain(
      "The result was 3.2x faster.",
    );
    expect(inspector.root.querySelector('[data-panel="block"]')!.textContent).not.toContain(
      "source-only context",
    );
    expect(inspector.root.querySelector('[data-region="main"]')!.getAttribute("aria-pressed")).toBe(
      "true",
    );
    expect(inspector.root.querySelector('[data-width="half"]')!.getAttribute("aria-pressed")).toBe(
      "true",
    );

    inspector.root.querySelector<HTMLButtonElement>('[data-region="side"]')!.click();
    inspector.root.querySelector<HTMLButtonElement>('[data-width="third"]')!.click();

    const theme = inspector.root.querySelector<HTMLButtonElement>('[data-theme-color="theme"]')!;
    const accent = inspector.root.querySelector<HTMLButtonElement>('[data-theme-color="accent"]')!;
    expect(theme.getAttribute("aria-pressed")).toBe("true");
    expect(inspector.root.querySelector(".slidx-block-color-hint")!.textContent).toContain(
      "adapt with the deck",
    );
    accent.click();

    const color = inspector.root.querySelector<HTMLInputElement>(".slidx-block-color-input")!;
    expect(color.value).toBe("#161b22");
    color.value = "#d946ef";
    color.dispatchEvent(new Event("change"));

    const identity = inspector.root.querySelector<HTMLElement>(".slidx-block-identity")!;
    const inputs = identity.querySelectorAll<HTMLInputElement>("input");
    expect([...inputs].map((input) => input.value)).toEqual(["result", "accent"]);
    expect(identity.querySelector<HTMLTextAreaElement>("textarea")!.value).toBe("tone=strong");
    inputs[0]!.value = "hero";
    inputs[1]!.value = "accent loud";
    identity.querySelector<HTMLTextAreaElement>("textarea")!.value = "tone=quiet\nweight=700";
    identity.querySelector<HTMLButtonElement>(".slidx-block-attributes")!.click();

    expect(ops).toEqual([
      { op: "moveBlock", slide: 1, block: 0, to: 0, region: "side" },
      { op: "setBlockWidth", slide: 1, block: 0, width: "third" },
      {
        op: "setBlockStyle",
        slide: 1,
        block: 0,
        property: "color",
        value: "var(--slidx-color-accent)",
      },
      {
        op: "setBlockStyle",
        slide: 1,
        block: 0,
        property: "color",
        value: "#d946ef",
      },
      {
        op: "setBlockAttributes",
        slide: 1,
        block: 0,
        attributes: {
          key: "hero",
          classes: ["accent", "loud"],
          properties: { tone: "quiet", weight: "700" },
        },
      },
    ]);
  });

  it("returns a fixed block color to the adaptive theme in one operation", () => {
    const log = recorder();
    const inspector = createInspector(log, {
      bodyOf: () => "## Two\n\nThe result was 3.2x faster.",
      blocksOf: () => [{ span: { start: 0, end: 38 } }],
      visualOf: () => ({
        color: "rgb(217, 70, 239)",
        managedColor: true,
        managedValue: "#d946ef",
        palette: [
          { name: "text", label: "Text", color: "#161b22" },
          { name: "heading", label: "Heading", color: "#0d1218" },
          { name: "muted", label: "Muted", color: "#5f656e" },
          { name: "accent", label: "Accent", color: "#01489f" },
        ],
      }),
    });
    inspector.render(stateOf({ selection: { slide: 1, block: 0 } }));

    expect(
      inspector.root.querySelector<HTMLDetailsElement>(".slidx-block-color-custom")!.open,
    ).toBe(true);
    inspector.root.querySelector<HTMLButtonElement>('[data-theme-color="theme"]')!.click();

    expect(log.ops).toEqual([{ op: "setBlockStyle", slide: 1, block: 0, property: "color" }]);
  });

  it("pins a block to an exact safe-area anchor and returns it to layout flow", () => {
    const log = recorder();
    let pinned = false;
    let rect = { left: 40, top: 40, width: 230, height: 80 };
    const inspector = createInspector(log, {
      bodyOf: () => "## Two",
      blocksOf: () => [{ span: { start: 0, end: 6 } }],
      geometry: () => ({
        slide: { left: 0, top: 0, width: 800, height: 450 },
        safe: { left: 40, top: 40, width: 720, height: 370 },
        regions: [
          {
            name: "main",
            rect: { left: 40, top: 40, width: 720, height: 370 },
            blocks: [0],
            contentHeight: 80,
            gap: 16,
          },
        ],
        blocks: [{ index: 0, region: "main", rect, needsWidth: 0, width: "full" }],
      }),
      visualOf: () => ({
        color: "rgb(22, 27, 34)",
        managedColor: false,
        managedFrame: pinned,
        palette: [],
      }),
    });
    const state = stateOf({ selection: { slide: 1, block: 0 } });
    inspector.render(state);

    const center = inspector.root.querySelector<HTMLButtonElement>(
      '[data-frame-anchor="middle-center"]',
    )!;
    const reset = () => inspector.root.querySelector<HTMLButtonElement>(".slidx-frame-reset")!;
    expect(center.getAttribute("aria-pressed")).toBe("false");
    expect(reset().disabled).toBe(true);

    center.click();
    expect(log.ops).toEqual([
      {
        op: "setBlockStyle",
        slide: 1,
        block: 0,
        property: "inset",
        value: "39.189% 34.028% 39.189% 34.028%",
      },
    ]);

    pinned = true;
    rect = { left: 285, top: 185, width: 230, height: 80 };
    inspector.render(state);
    expect(
      inspector.root
        .querySelector('[data-frame-anchor="middle-center"]')!
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(inspector.root.querySelector(".slidx-frame-position-state")!.textContent).toBe(
      "Pinned to safe area",
    );
    expect(reset().disabled).toBe(false);

    reset().click();
    expect(log.ops.at(-1)).toEqual({
      op: "setBlockStyle",
      slide: 1,
      block: 0,
      property: "inset",
    });
  });

  it("duplicates or removes the selected block from its action row", () => {
    const ops: EditOp[] = [];
    const selections: Array<number | undefined> = [];
    const inspector = createInspector(
      {
        run: (op) => ops.push(op),
        selectBlock: (block) => selections.push(block),
      },
      options,
    );
    inspector.render(stateOf({ selection: { slide: 2, block: 3 } }));

    inspector.root.querySelector<HTMLButtonElement>(".slidx-block-duplicate")!.click();
    inspector.root.querySelector<HTMLButtonElement>(".slidx-block-delete")!.click();

    expect(ops).toEqual([
      { op: "duplicateBlock", slide: 2, block: 3 },
      { op: "removeBlock", slide: 2, block: 3 },
    ]);
    expect(selections).toEqual([undefined]);
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

  it("offers only renderer-backed arrivals and inherits the deck default explicitly", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf();
    state.slides[0]!.frontmatter = { title: "A", transition: "fade" };
    inspector.render(state);

    const choices = [
      ...inspector.root.querySelectorAll<HTMLButtonElement>(
        '[data-group="slide"] .slidx-transition-choice',
      ),
    ];
    expect(choices.map((choice) => choice.dataset.transition)).toEqual([
      "inherit",
      "none",
      "fade",
      "slide",
      "push",
    ]);
    expect(choices[0]!.textContent).toContain("Deck default · Fade");
    expect(choices[0]!.getAttribute("aria-pressed")).toBe("true");
    expect(inspector.root.querySelector('[data-group="slide"] [data-key="transition"]')).toBeNull();

    choices[4]!.click();
    expect(log.ops).toEqual([{ op: "setField", slide: 1, key: "transition", value: "push" }]);
  });

  it("returns an authored arrival to inheritance by removing the field", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf();
    state.slides[1]!.frontmatter = { transition: "push" };
    inspector.render(state);

    const inherit = inspector.root.querySelector<HTMLButtonElement>(
      '[data-group="slide"] [data-transition="inherit"]',
    )!;
    expect(
      inspector.root
        .querySelector('[data-group="slide"] [data-transition="push"]')!
        .getAttribute("aria-pressed"),
    ).toBe("true");
    inherit.click();

    expect(log.ops).toEqual([{ op: "removeField", slide: 1, key: "transition" }]);
  });

  it("makes an unavailable authored arrival visible without pretending it is selected", () => {
    const inspector = createInspector(recorder(), options);
    const state = stateOf();
    state.slides[1]!.frontmatter = { transition: "spin" };
    inspector.render(state);

    expect(inspector.root.querySelector(".slidx-delivery-notice")!.textContent).toContain(
      "not available",
    );
    expect(
      [...inspector.root.querySelectorAll('[data-group="slide"] .slidx-transition-choice')].every(
        (choice) => choice.getAttribute("aria-pressed") === "false",
      ),
    ).toBe(true);
  });

  it("compares the authored budget with pipeline-resolved speaking time", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf();
    state.slides[1] = {
      ...state.slides[1]!,
      budgetSeconds: 60,
      estimatedSeconds: 75,
      frontmatter: { budget: "1:00" },
    };
    inspector.render(state);

    expect(
      inspector.root
        .querySelector('[data-group="slide"] [data-budget="60"]')!
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      inspector.root.querySelector<HTMLInputElement>('[data-group="slide"] [data-key="budget"]')!
        .value,
    ).toBe("1:00");
    expect(inspector.root.querySelector(".slidx-budget-status")!.textContent).toContain(
      "15s over budget",
    );

    inspector.root
      .querySelector<HTMLButtonElement>('[data-group="slide"] [data-budget="estimate"]')!
      .click();
    inspector.root
      .querySelector<HTMLButtonElement>('[data-group="slide"] [data-budget="120"]')!
      .click();

    expect(log.ops).toEqual([
      { op: "removeField", slide: 1, key: "budget" },
      { op: "setField", slide: 1, key: "budget", value: "2m" },
    ]);
  });

  it("keeps opening-slide delivery separate from the deck's transition policy", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf({ selection: { slide: 0 } }));

    const slide = inspector.root.querySelector('[data-group="slide"]')!;
    expect(slide.textContent).toContain("Timing");
    expect(slide.textContent).toContain("Safe to skip");
    expect(slide.querySelector(".slidx-transition-field")).toBeNull();

    inspector.root.querySelector<HTMLButtonElement>('[data-tab="deck"]')!.click();
    const deck = inspector.root.querySelector('[data-group="deck"]')!;
    expect(deck.textContent).toContain("Default transition");
    expect(deck.querySelector('[data-transition="none"]')!.getAttribute("aria-pressed")).toBe(
      "true",
    );
    deck.querySelector<HTMLButtonElement>('[data-transition="slide"]')!.click();

    expect(log.ops).toEqual([{ op: "setField", slide: 0, key: "transition", value: "slide" }]);
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

  it("chooses the deck theme from pipeline-provided visual miniatures", () => {
    const lightLog = recorder();
    const light = createInspector(lightLog, { ...options, scheme: () => "light" });
    light.render(stateOf());

    const choices = [
      ...light.root.querySelectorAll<HTMLButtonElement>('[data-group="deck"] [data-theme]'),
    ];
    expect(choices.map((choice) => choice.dataset.theme)).toEqual(["minimal", "editorial"]);
    expect(choices[0]!.getAttribute("aria-pressed")).toBe("true");
    expect(light.root.querySelector('[data-group="deck"] [data-key="theme"]')).toBeNull();
    expect(document.querySelectorAll("style[data-slidx-theme-picker]")).toHaveLength(1);

    const lightSurface =
      choices[0]!.querySelector<HTMLElement>(".slidx-theme-preview")!.style.backgroundColor;
    choices[1]!.click();
    expect(lightLog.ops).toEqual([{ op: "setField", slide: 0, key: "theme", value: "editorial" }]);

    const dark = createInspector(recorder(), { ...options, scheme: () => "dark" });
    dark.render(stateOf());
    const darkSurface = dark.root.querySelector<HTMLElement>(
      '[data-group="deck"] [data-theme="minimal"] .slidx-theme-preview',
    )!.style.backgroundColor;
    expect(darkSurface).not.toBe(lightSurface);
  });

  it("explains a build-configured theme instead of offering an overridden control", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf({ activeTheme: "editorial", themeLocked: true });
    state.slides[0]!.frontmatter = { title: "A", theme: "minimal" };
    inspector.render(state);

    const choices = [
      ...inspector.root.querySelectorAll<HTMLButtonElement>(
        '[data-group="deck"] .slidx-theme-choice',
      ),
    ];
    expect(choices.every((choice) => choice.disabled)).toBe(true);
    expect(
      inspector.root
        .querySelector('[data-group="deck"] [data-theme="editorial"]')!
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(inspector.root.querySelector(".slidx-theme-notice")!.textContent).toContain(
      "build configuration",
    );

    choices[0]!.click();
    expect(log.ops).toEqual([]);
  });

  it("offers a one-click repair when an authored theme is unavailable", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf();
    state.slides[0]!.frontmatter = { title: "A", theme: "missing-theme" };
    inspector.render(state);

    expect(inspector.root.querySelector(".slidx-theme-notice")!.textContent).toContain(
      "not available",
    );
    expect(
      [...inspector.root.querySelectorAll('[data-group="deck"] [data-theme]')].every(
        (choice) => choice.getAttribute("aria-pressed") === "false",
      ),
    ).toBe(true);

    inspector.root
      .querySelector<HTMLButtonElement>('[data-group="deck"] [data-theme="minimal"]')!
      .click();
    expect(log.ops).toEqual([{ op: "setField", slide: 0, key: "theme", value: "minimal" }]);
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

  it("offers theme-aware text presets without asking for attribute syntax", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf({ selection: { slide: 1, text: "3.2x faster" } }));

    const selection = inspector.root.querySelector<HTMLElement>('[data-group="selection"]')!;
    expect(selection.querySelector('[data-tone="theme"]')!.getAttribute("aria-pressed")).toBe(
      "true",
    );
    expect(
      selection
        .querySelector('[aria-label="Text weight"] [data-value="regular"]')!
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      selection
        .querySelector('[aria-label="Text typeface"] [data-value="theme"]')!
        .getAttribute("aria-pressed"),
    ).toBe("true");

    selection.querySelector<HTMLElement>('[data-tone="accent"]')!.click();

    expect(log.ops).toEqual([
      {
        op: "addMark",
        slide: 1,
        range: { start: 23, end: 34 },
        attributes: { key: undefined, classes: ["accent"], properties: {} },
      },
    ]);
  });

  it("changes one text appearance dimension without erasing theme extensions", () => {
    const log = recorder();
    const body =
      "## Two\n\nThe result was [3.2x faster]{#result .hero .accent color=danger font=serif}.";
    const inspector = createInspector(log, {
      bodyOf: () => body,
      blocksOf: () => [
        {
          span: { start: 8, end: body.length },
          marks: [
            {
              span: { start: 22, end: body.length - 1 },
              words: { start: 23, end: 34 },
              key: "result",
              classes: ["hero", "accent"],
              properties: { color: "danger", font: "serif" },
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
    expect(selection.querySelector('[data-tone="danger"]')!.getAttribute("aria-pressed")).toBe(
      "true",
    );
    selection.querySelector<HTMLElement>('[data-tone="muted"]')!.click();

    expect(log.ops).toEqual([
      {
        op: "setMark",
        slide: 1,
        mark: 0,
        attributes: {
          key: "result",
          classes: ["hero", "muted"],
          properties: { font: "serif" },
        },
      },
    ]);
  });

  it("locks every text style control while a deck write is in flight", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf({ writing: true, selection: { slide: 1, text: "3.2x faster" } }));

    const selection = inspector.root.querySelector<HTMLElement>('[data-group="selection"]')!;
    expect(
      [...selection.querySelectorAll<HTMLButtonElement>("button")].every(
        (button) => button.disabled,
      ),
    ).toBe(true);
    expect(
      [
        ...selection.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>("input, textarea"),
      ].every((input) => input.disabled),
    ).toBe(true);
    expect(log.ops).toEqual([]);
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

  it("writes a skip decision as a flag rather than as the word for one", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    inspector.render(stateOf());

    const optional = inspector.root.querySelector<HTMLButtonElement>(
      '[data-group="slide"] [data-key="optional"]',
    )!;
    expect(optional.getAttribute("aria-pressed")).toBe("false");
    optional.click();

    expect(log.ops).toEqual([{ op: "setField", slide: 1, key: "optional", value: true }]);
  });

  it("returns a skip decision to the core talk by removing the field", () => {
    const log = recorder();
    const inspector = createInspector(log, options);
    const state = stateOf();
    state.slides[1] = {
      ...state.slides[1]!,
      optional: true,
      frontmatter: { optional: true },
    };
    inspector.render(state);

    const optional = inspector.root.querySelector<HTMLButtonElement>(
      '[data-group="slide"] [data-key="optional"]',
    )!;
    expect(optional.getAttribute("aria-pressed")).toBe("true");
    optional.click();

    expect(log.ops).toEqual([{ op: "removeField", slide: 1, key: "optional" }]);
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

  it("jumps to the slide from the keyboard, which is what its roles promise", () => {
    // The row carries `role="button"` and `tabindex="0"`, so it is reachable by
    // Tab and announced as operable. Only `click` was bound, so a keyboard user
    // could focus a finding, press Enter, and watch nothing happen — the one
    // path this panel exists to offer, unavailable to the people the roles were
    // added for.
    for (const key of ["Enter", " "]) {
      const jumped: number[] = [];
      const diagnostics = createDiagnostics({ select: (at) => jumped.push(at) });
      diagnostics.render(
        stateOf({
          diagnostics: [
            { severity: "warning", code: "a11y/alt", message: "no alt", slideIndex: 2 },
          ],
        }),
      );

      const row = diagnostics.root.querySelector<HTMLElement>(".slidx-finding")!;
      const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true });
      row.dispatchEvent(event);

      expect(jumped, `${key} should jump`).toEqual([2]);
      expect(event.defaultPrevented, `${key} should not also scroll`).toBe(true);
    }
  });

  it("leaves keys it does not act on to the browser", () => {
    // Tab has to keep moving through the list, and a deck-level finding has no
    // slide to jump to and is not focusable at all.
    const jumped: number[] = [];
    const diagnostics = createDiagnostics({ select: (at) => jumped.push(at) });
    diagnostics.render(
      stateOf({
        diagnostics: [{ severity: "warning", code: "a11y/alt", message: "no alt", slideIndex: 2 }],
      }),
    );

    const row = diagnostics.root.querySelector<HTMLElement>(".slidx-finding")!;
    const event = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    row.dispatchEvent(event);

    expect(jumped).toEqual([]);
    expect(event.defaultPrevented).toBe(false);
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
