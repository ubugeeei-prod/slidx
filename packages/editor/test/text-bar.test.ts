import { afterEach, describe, expect, it } from "vite-plus/test";

import type { BlockSpans } from "../src/client";
import type { EditOp } from "../src/operations";
import type { EditorState } from "../src/session";
import { createTextBar } from "../src/text-bar";

function stateOf(over: Partial<EditorState> = {}): EditorState {
  return {
    source: "",
    spans: [],
    slides: [],
    layouts: [],
    activeTheme: "minimal",
    themeLocked: false,
    themes: [],
    transitions: [],
    diagnostics: [],
    selection: { slide: 0 },
    viewers: [],
    canEdit: true,
    canUndo: false,
    canRedo: false,
    writing: false,
    ...over,
  };
}

function mountBar(body: string, blocks: readonly BlockSpans[] = []) {
  const ops: EditOp[] = [];
  let done = 0;
  const bar = createTextBar(
    {
      run: (op) => ops.push(op),
      done: () => {
        done += 1;
      },
    },
    { bodyOf: () => body, blocksOf: () => blocks },
  );
  const tools = document.createElement("div");
  tools.className = "slidx-canvas-tools";
  const title = document.createElement("h2");
  title.textContent = "Slide";
  const header = document.createElement("header");
  header.className = "slidx-panel-head";
  header.append(title, tools, bar.root);
  document.body.append(header);

  return { bar, done: () => done, header, ops, title, tools };
}

afterEach(() => document.body.replaceChildren());

describe("the selected text quick bar", () => {
  it("replaces canvas tools and applies a theme-aware tone to plain words", () => {
    const body = "## Two\n\nThe result was 3.2x faster.";
    const start = body.indexOf("3.2x faster");
    const mounted = mountBar(body);

    mounted.bar.render(
      stateOf({
        selection: {
          slide: 1,
          text: "3.2x faster",
          range: { start, end: start + "3.2x faster".length },
        },
      }),
    );

    expect(mounted.bar.root.hidden).toBe(false);
    expect(mounted.header.dataset.textTools).toBe("true");
    expect(mounted.title.textContent).toBe("Text");
    expect(
      mounted.bar.root.querySelector('[data-tone="theme"]')!.getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      mounted.bar.root.querySelector('[data-style="bold"]')!.getAttribute("aria-keyshortcuts"),
    ).toBe("Control+B Meta+B");

    mounted.bar.root.querySelector<HTMLButtonElement>('[data-tone="accent"]')!.click();

    expect(mounted.ops).toEqual([
      {
        op: "addMark",
        slide: 1,
        range: { start, end: start + "3.2x faster".length },
        attributes: { key: undefined, classes: ["accent"], properties: {} },
      },
    ]);
  });

  it("toggles one existing style without erasing the others", () => {
    const body = "## Two\n\nThe result was [3.2x faster]{#result .accent weight=bold font=serif}.";
    const start = body.indexOf("3.2x faster");
    const end = start + "3.2x faster".length;
    const mounted = mountBar(body, [
      {
        span: { start: 8, end: body.length },
        marks: [
          {
            span: { start: start - 1, end: body.indexOf("}.") + 1 },
            words: { start, end },
            key: "result",
            classes: ["accent"],
            properties: { weight: "bold", font: "serif" },
          },
        ],
      },
    ]);

    mounted.bar.render(
      stateOf({ selection: { slide: 1, text: "3.2x faster", range: { start, end } } }),
    );

    expect(
      mounted.bar.root.querySelector('[data-tone="accent"]')!.getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      mounted.bar.root.querySelector('[data-style="bold"]')!.getAttribute("aria-pressed"),
    ).toBe("true");

    mounted.bar.root.querySelector<HTMLButtonElement>('[data-style="bold"]')!.click();

    expect(mounted.ops).toEqual([
      {
        op: "setMark",
        slide: 1,
        mark: 0,
        attributes: {
          key: "result",
          classes: ["accent"],
          properties: { font: "serif" },
        },
      },
    ]);
  });

  it("finishes without writing and restores the canvas header", () => {
    const body = "Selected words";
    const mounted = mountBar(body);
    mounted.bar.render(
      stateOf({ selection: { slide: 0, text: body, range: { start: 0, end: body.length } } }),
    );

    mounted.bar.root.querySelector<HTMLButtonElement>(".slidx-text-bar-done")!.click();
    expect(mounted.done()).toBe(1);
    expect(mounted.ops).toEqual([]);

    mounted.bar.render(stateOf());
    expect(mounted.bar.root.hidden).toBe(true);
    expect(mounted.header.dataset.textTools).toBe("false");
    expect(mounted.title.textContent).toBe("Slide");
  });

  it("stays out of the way for an unsafe partial mark selection", () => {
    const body = "A [styled phrase]{.accent}.";
    const mounted = mountBar(body, [
      {
        span: { start: 0, end: body.length },
        marks: [
          {
            span: { start: 2, end: 25 },
            words: { start: 3, end: 16 },
            classes: ["accent"],
          },
        ],
      },
    ]);

    mounted.bar.render(
      stateOf({ selection: { slide: 0, text: "styled", range: { start: 3, end: 9 } } }),
    );

    expect(mounted.bar.root.hidden).toBe(true);
    expect(mounted.header.dataset.textTools).toBe("false");
    expect(mounted.title.textContent).toBe("Slide");
  });

  it("locks style writes in flight while keeping Done available", () => {
    const body = "Selected words";
    const mounted = mountBar(body);
    mounted.bar.render(
      stateOf({
        writing: true,
        selection: { slide: 0, text: body, range: { start: 0, end: body.length } },
      }),
    );

    expect(
      [
        ...mounted.bar.root.querySelectorAll<HTMLButtonElement>(
          ".slidx-text-bar-tone, .slidx-text-bar-toggle",
        ),
      ].every((button) => button.disabled),
    ).toBe(true);
    expect(
      mounted.bar.root.querySelector<HTMLButtonElement>(".slidx-text-bar-done")!.disabled,
    ).toBe(false);
  });
});
