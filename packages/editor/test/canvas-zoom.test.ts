/** The canvas viewport grows around the same rendered slide rather than redrawing it. */

import { afterEach, describe, expect, it } from "vite-plus/test";

import { createCanvas, startingZoom } from "../src/canvas";

afterEach(() => document.body.replaceChildren());

function canvas(storage?: Pick<Storage, "getItem" | "setItem">) {
  return createCanvas(
    { run() {}, selected() {} },
    { deckBase: "slides", bodyOf: () => "# One", storage },
  );
}

describe("canvas zoom", () => {
  it("starts fitted and rejects stale storage values", () => {
    expect(startingZoom(undefined)).toBe(1);
    expect(startingZoom({ getItem: () => "1.5" })).toBe(1.5);
    expect(startingZoom({ getItem: () => "3" })).toBe(1);
    expect(startingZoom({ getItem: () => "banana" })).toBe(1);
  });

  it("steps through deliberate levels and returns to fit", () => {
    const stored: string[] = [];
    const surface = canvas({ getItem: () => null, setItem: (_key, value) => stored.push(value) });
    const stage = surface.root.querySelector<HTMLElement>(".slidx-canvas-stage")!;
    const value = surface.root.querySelector<HTMLButtonElement>(".slidx-canvas-zoom-value")!;
    const out = surface.root.querySelector<HTMLButtonElement>(".slidx-canvas-zoom-out")!;
    const into = surface.root.querySelector<HTMLButtonElement>(".slidx-canvas-zoom-in")!;

    expect(surface.zoom()).toBe(1);
    expect(value.textContent).toBe("Fit");
    expect(out.disabled).toBe(true);

    into.click();
    into.click();
    expect(surface.zoom()).toBe(1.5);
    expect(value.textContent).toBe("150%");
    expect(stage.style.getPropertyValue("--slidx-e-canvas-zoom")).toBe("150%");

    value.click();
    expect(surface.zoom()).toBe(1);
    expect(stage.dataset.zoom).toBe("fit");
    expect(stored).toEqual(["1.25", "1.5", "1"]);
    surface.destroy?.();
  });

  it("holds at both ends and hides a meaningless control in source mode", () => {
    const surface = canvas();
    const zoom = surface.root.querySelector<HTMLElement>(".slidx-canvas-zoom")!;
    document.body.append(surface.root);

    surface.zoomOut();
    expect(surface.zoom()).toBe(1);
    surface.zoomIn();
    surface.zoomIn();
    surface.zoomIn();
    surface.zoomIn();
    expect(surface.zoom()).toBe(2);

    surface.showMarkdown();
    expect(zoom.hidden).toBe(true);
    expect(getComputedStyle(zoom).display).toBe("none");
    surface.showVisual();
    expect(zoom.hidden).toBe(false);
    expect(getComputedStyle(zoom).display).toBe("flex");
    surface.destroy?.();
  });
});
