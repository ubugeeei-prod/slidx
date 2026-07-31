/** The editor's side-panel separators, driven by pointer and keyboard. */

import { afterEach, describe, expect, it } from "vite-plus/test";

import { createPanelResize } from "../src/panel-resize";

function mounted(saved: Record<string, string> = {}) {
  const values = new Map(Object.entries(saved));
  const writes: Array<[string, string]> = [];
  const surface = createPanelResize({
    storage: {
      getItem: (key) => values.get(key) ?? null,
      setItem: (key, value) => {
        values.set(key, value);
        writes.push([key, value]);
      },
    },
  });
  const editor = document.createElement("div");
  editor.className = "slidx-editor";
  Object.defineProperty(editor, "getBoundingClientRect", {
    value: () => ({ left: 10, right: 1210, width: 1200, top: 0, bottom: 800, height: 800 }),
  });
  editor.append(surface.root);
  document.body.append(editor);
  surface.render({} as never);

  return {
    editor,
    writes,
    outline: surface.root.querySelector<HTMLElement>('[data-panel="outline"]')!,
    inspector: surface.root.querySelector<HTMLElement>('[data-panel="inspector"]')!,
  };
}

function pointer(kind: string, x: number): Event {
  const event = new MouseEvent(kind, { bubbles: true, cancelable: true, clientX: x });
  Object.defineProperty(event, "pointerId", { value: 1 });
  return event;
}

afterEach(() => document.body.replaceChildren());

describe("side panel resizing", () => {
  it("restores finite widths without writing them into the deck", () => {
    const opened = mounted({
      "slidx.editor.outline-width": "320",
      "slidx.editor.inspector-width": "not-a-number",
    });

    expect(opened.editor.style.getPropertyValue("--slidx-e-outline-width")).toBe("320px");
    expect(opened.editor.style.getPropertyValue("--slidx-e-inspector-width")).toBe("296px");
    expect(opened.writes).toEqual([]);
  });

  it("drags either separator and persists the viewing preference", () => {
    const opened = mounted();

    opened.outline.dispatchEvent(pointer("pointerdown", 310));
    expect(document.activeElement).toBe(opened.outline);
    opened.outline.dispatchEvent(pointer("pointermove", 354));
    opened.outline.dispatchEvent(pointer("pointerup", 354));
    opened.inspector.dispatchEvent(pointer("pointerdown", 860));
    opened.inspector.dispatchEvent(pointer("pointerup", 860));

    expect(opened.editor.style.getPropertyValue("--slidx-e-outline-width")).toBe("344px");
    expect(opened.editor.style.getPropertyValue("--slidx-e-inspector-width")).toBe("350px");
    expect(opened.writes.at(-1)).toEqual(["slidx.editor.inspector-width", "350"]);
  });

  it("keeps a usable canvas between the two panels", () => {
    const opened = mounted();

    opened.outline.dispatchEvent(pointer("pointerdown", 1200));
    opened.outline.dispatchEvent(pointer("pointerup", 1200));
    opened.inspector.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));
    expect(opened.inspector.getAttribute("aria-valuenow")).toBe("240");

    opened.inspector.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));

    const left = Number.parseFloat(opened.editor.style.getPropertyValue("--slidx-e-outline-width"));
    const right = Number.parseFloat(
      opened.editor.style.getPropertyValue("--slidx-e-inspector-width"),
    );
    // End asks for 520, and the canvas floor answers with 360.
    expect(right).toBe(360);
    expect(left + right + 360).toBeLessThanOrEqual(1200);
  });

  it("supports arrows, Home, End and announces the resulting width", () => {
    const opened = mounted();

    opened.outline.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }),
    );
    expect(opened.outline.getAttribute("aria-valuenow")).toBe("248");

    opened.inspector.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true }),
    );
    expect(opened.inspector.getAttribute("aria-valuetext")).toBe("312 pixels");

    opened.outline.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));
    expect(opened.outline.getAttribute("aria-valuenow")).toBe("176");
  });

  it("notifies canvas overlays after a panel changes size", () => {
    const opened = mounted();
    let layouts = 0;
    window.addEventListener("resize", () => (layouts += 1), { once: true });

    opened.outline.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }),
    );

    expect(layouts).toBe(1);
  });
});
