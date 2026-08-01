import { page, userEvent } from "@vitest/browser/context";
import { afterEach, beforeAll, describe, expect, it } from "vite-plus/test";

import { createInspector } from "../src/inspector";
import { createOutline } from "../src/outline";
import { createPanelResize } from "../src/panel-resize";
import { applyStyles } from "../src/styles";
import type { EditOp } from "../src/operations";
import type { EditorState } from "../src/session";

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
    ],
    activeTheme: "",
    themeLocked: false,
    themes: [],
    transitions: [],
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

function frame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

function nextLoad(target: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(
      () => reject(new Error(`iframe did not load ${target.getAttribute("src")}`)),
      3_000,
    );
    target.addEventListener(
      "load",
      () => {
        window.clearTimeout(timeout);
        resolve();
      },
      { once: true },
    );
  });
}

function roundedWidth(element: Element): number {
  return Math.round(element.getBoundingClientRect().width);
}

beforeAll(() => applyStyles(document));

afterEach(() => {
  document.body.replaceChildren();
});

describe("editor chrome in Chromium", () => {
  it("resizes both panel grid tracks without consuming the canvas", async () => {
    const editor = document.createElement("main");
    editor.className = "slidx-editor";
    editor.style.width = "1200px";
    editor.style.height = "640px";

    const outline = document.createElement("section");
    outline.className = "slidx-outline";
    const canvas = document.createElement("section");
    canvas.className = "slidx-canvas";
    const inspector = document.createElement("section");
    inspector.className = "slidx-inspector";
    const resize = createPanelResize();
    editor.append(outline, canvas, inspector, resize.root);
    document.body.append(editor);
    resize.render(stateOf());
    await frame();

    expect(roundedWidth(outline)).toBe(232);
    expect(roundedWidth(inspector)).toBe(296);
    expect(roundedWidth(canvas)).toBe(672);

    const inspectorSeparator = page.getByRole("separator", {
      name: "Resize inspector panel",
    });
    await expect.element(inspectorSeparator).toHaveAttribute("aria-orientation", "vertical");
    await expect.element(inspectorSeparator).toHaveAttribute("aria-valuemax", "520");
    await inspectorSeparator.click();
    await userEvent.keyboard("{End}");
    await frame();

    await expect.element(inspectorSeparator).toHaveAttribute("aria-valuenow", "520");
    expect(editor.style.getPropertyValue("--slidx-e-inspector-width")).toBe("520px");
    expect(roundedWidth(inspector)).toBe(520);
    expect(roundedWidth(canvas)).toBe(448);

    await inspectorSeparator.dblClick();
    const outlineSeparator = page.getByRole("separator", { name: "Resize slide panel" });
    await outlineSeparator.click();
    await userEvent.keyboard("{End}");
    await frame();

    await expect.element(outlineSeparator).toHaveAttribute("aria-valuenow", "480");
    expect(editor.style.getPropertyValue("--slidx-e-outline-width")).toBe("480px");
    expect(roundedWidth(outline)).toBe(480);
    expect(roundedWidth(inspector)).toBe(296);
    expect(roundedWidth(canvas)).toBe(424);
    expect(roundedWidth(canvas)).toBeGreaterThanOrEqual(360);
  });

  it("keeps lazy outline previews mounted across editor state changes", async () => {
    const outline = createOutline(
      {
        select() {},
        run() {},
      },
      {
        preview: (slide) =>
          `/browser-outline-fixture/${slide === 0 ? "" : `${slide + 1}/`}?outline=1`,
      },
    );
    const initial = stateOf();
    outline.render(initial);
    outline.root.style.width = "272px";
    outline.root.style.height = "760px";

    const frames = [...outline.root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")];
    const initialLoad = nextLoad(frames[0]!);
    document.body.append(outline.root);
    await initialLoad;

    expect(frames).toHaveLength(3);
    expect(frames.map((preview) => preview.loading)).toEqual(["lazy", "lazy", "lazy"]);
    expect(frames.map((preview) => preview.tabIndex)).toEqual([-1, -1, -1]);
    expect(frames.map((preview) => preview.getAttribute("aria-hidden"))).toEqual([
      "true",
      "true",
      "true",
    ]);
    expect(getComputedStyle(frames[0]!).pointerEvents).toBe("none");
    await expect.element(page.getByRole("button", { name: "2 Two" })).toBeVisible();

    const preview = frames[0]!.closest<HTMLElement>(".slidx-outline-thumbnail")!;
    const previewBox = preview.getBoundingClientRect();
    expect(previewBox.width).toBeGreaterThan(150);
    expect(Math.abs(previewBox.width / previewBox.height - 16 / 9)).toBeLessThan(0.02);

    const sources = frames.map((iframe) => iframe.getAttribute("src"));
    outline.render({
      ...initial,
      selection: { slide: 2 },
      viewers: [{ id: "guest", label: "Guest", slide: 0, local: false, canEdit: true }],
    });
    await frame();
    await frame();

    const kept = [...outline.root.querySelectorAll<HTMLIFrameElement>(".slidx-outline-frame")];
    expect(kept).toEqual(frames);
    expect(kept.map((iframe) => iframe.getAttribute("src"))).toEqual(sources);
    expect(
      outline.root.querySelectorAll(".slidx-outline-row")[2]!.getAttribute("aria-current"),
    ).toBe("true");

    outline.render({ ...initial, source: `${initial.source}\n` });
    await frame();

    expect(frames[0]!.getAttribute("src")).toBe("/browser-outline-fixture/?outline=1");
    expect(outline.root.querySelector(".slidx-outline-frame")).toBe(frames[0]);
  });

  it("switches inspector targets and applies a visual text choice", async () => {
    const ops: EditOp[] = [];
    const body = "The result was 3.2x faster.";
    const inspector = createInspector({ run: (op) => ops.push(op) }, { bodyOf: () => body });
    inspector.root.style.width = "520px";
    inspector.root.style.height = "760px";
    inspector.render(stateOf());
    document.body.append(inspector.root);

    const slideTab = page.getByRole("tab", { name: "Slide" });
    const deckTab = page.getByRole("tab", { name: "Deck" });
    await expect.element(page.getByRole("tablist", { name: "Inspector target" })).toBeVisible();
    await expect.element(slideTab).toHaveAttribute("aria-selected", "true");

    let panels = [...inspector.root.querySelectorAll<HTMLElement>("[role=tabpanel]")];
    expect(panels).toHaveLength(4);
    expect(panels.filter((panel) => getComputedStyle(panel).display !== "none")).toEqual([
      panels[2],
    ]);

    await deckTab.click();
    await expect.element(deckTab).toHaveAttribute("aria-selected", "true");
    panels = [...inspector.root.querySelectorAll<HTMLElement>("[role=tabpanel]")];
    expect(panels.filter((panel) => getComputedStyle(panel).display !== "none")).toEqual([
      panels[3],
    ]);

    await userEvent.keyboard("{ArrowLeft}");
    await expect.element(slideTab).toHaveAttribute("aria-selected", "true");

    inspector.render(
      stateOf({
        selection: { slide: 1, text: "3.2x", range: { start: 15, end: 19 } },
      }),
    );
    const textTab = page.getByRole("tab", { name: "Text" });
    await expect.element(textTab).toHaveAttribute("aria-selected", "true");
    await expect.element(page.getByRole("group", { name: "Text typeface" })).toBeVisible();
    await expect.element(page.getByRole("group", { name: "Text weight" })).toBeVisible();
    await expect.element(page.getByRole("group", { name: "Text tone" })).toBeVisible();

    const mono = page.getByRole("button", { name: "Mono" });
    const accent = page.getByRole("button", { name: "Accent" });
    await mono.click();

    expect(getComputedStyle(mono.element()).fontFamily).toContain("ui-monospace");
    expect(roundedWidth(accent.element())).toBeGreaterThanOrEqual(28);
    expect(Math.round(accent.element().getBoundingClientRect().height)).toBeGreaterThanOrEqual(28);
    const swatch = accent.element().querySelector<HTMLElement>(".slidx-text-tone-swatch")!;
    expect(getComputedStyle(swatch).backgroundColor).not.toBe("rgba(0, 0, 0, 0)");
    expect(ops).toEqual([
      {
        op: "addMark",
        slide: 1,
        range: { start: 15, end: 19 },
        attributes: {
          key: undefined,
          classes: ["code"],
          properties: {},
        },
      },
    ]);
  });
});
