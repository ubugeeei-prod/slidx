/** The reversible workspace mode that lets the slide use the full editing field. */

import { afterEach, describe, expect, it } from "vite-plus/test";

import { createWorkspaceFocus } from "../src/workspace-focus";

afterEach(() => {
  document.body.replaceChildren();
  window.localStorage.clear();
});

describe("canvas focus", () => {
  it("gives the canvas the panel grid and restores it without removing a surface", () => {
    let changed = 0;
    const focus = createWorkspaceFocus({ changed: () => (changed += 1) });
    const editor = document.createElement("main");
    editor.className = "slidx-editor";
    const outline = document.createElement("section");
    outline.className = "slidx-outline";
    const canvas = document.createElement("section");
    canvas.className = "slidx-canvas";
    const frame = document.createElement("iframe");
    frame.className = "slidx-canvas-frame";
    canvas.append(frame);
    editor.append(focus.root, outline, canvas);
    document.body.append(editor);
    focus.connect();

    focus.trigger.click();

    expect(focus.active()).toBe(true);
    expect(editor.getAttribute("data-canvas-focus")).toBe("true");
    expect(focus.trigger.getAttribute("aria-pressed")).toBe("true");
    expect(focus.trigger.getAttribute("aria-label")).toBe("Restore workspace");
    expect(document.activeElement).toBe(frame);
    expect(outline.isConnected).toBe(true);
    expect(canvas.isConnected).toBe(true);

    focus.exit();

    expect(focus.active()).toBe(false);
    expect(editor.getAttribute("data-canvas-focus")).toBe("false");
    expect(focus.trigger.getAttribute("aria-pressed")).toBe("false");
    expect(document.activeElement).toBe(focus.trigger);
    expect(changed).toBe(2);
  });

  it("does nothing before its trigger belongs to an editor", () => {
    const focus = createWorkspaceFocus();

    focus.toggle();

    expect(focus.active()).toBe(false);
    expect(focus.trigger.getAttribute("aria-pressed")).toBe("false");
  });

  it("keeps one compact side panel beside the canvas and remembers the choice", () => {
    const storage = window.localStorage;
    const focus = createWorkspaceFocus({ storage });
    const editor = document.createElement("main");
    editor.className = "slidx-editor";
    editor.append(focus.root);
    document.body.append(editor);
    focus.connect();

    expect(focus.panel()).toBe("outline");
    expect(editor.getAttribute("data-workspace-panel")).toBe("outline");
    expect(focus.root.querySelector('[data-panel="outline"]')?.getAttribute("aria-pressed")).toBe(
      "true",
    );

    focus.togglePanel("inspector");

    expect(focus.panel()).toBe("inspector");
    expect(editor.getAttribute("data-workspace-panel")).toBe("inspector");
    expect(storage.getItem("slidx.editor.workspace-panel")).toBe("inspector");
    expect(focus.root.querySelector('[data-panel="inspector"]')?.getAttribute("aria-label")).toBe(
      "Hide inspector panel",
    );

    focus.togglePanel("inspector");

    expect(focus.panel()).toBe("canvas");
    expect(editor.getAttribute("data-workspace-panel")).toBe("canvas");
  });

  it("opens a requested panel directly from full-canvas focus", () => {
    const focus = createWorkspaceFocus();
    const editor = document.createElement("main");
    editor.className = "slidx-editor";
    const canvas = document.createElement("section");
    canvas.className = "slidx-canvas";
    canvas.append(document.createElement("iframe"));
    editor.append(focus.root, canvas);
    document.body.append(editor);
    focus.connect();
    focus.toggle();

    focus.togglePanel("outline");

    expect(focus.active()).toBe(false);
    expect(focus.panel()).toBe("outline");
    expect(editor.getAttribute("data-canvas-focus")).toBe("false");
    expect(document.activeElement).not.toBe(focus.trigger);
  });
});
