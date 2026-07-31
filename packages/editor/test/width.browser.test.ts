/**
 * The renderer/editor width boundary, in a browser that performs layout.
 *
 * The ordinary editor tests inject rectangles, which is the right boundary for
 * gesture arithmetic but cannot prove which rectangle CSS actually produced.
 * This one renders the production WASM page in the canvas iframe, lets Chromium
 * compute it, and then hands that geometry to the real resize surface.
 */

import { page, userEvent } from "vite-plus/test/browser/context";
import { afterEach, beforeAll, describe, expect, it } from "vite-plus/test";

import init, { buildDeck } from "../../wasm/dist/slidx.js";
import { WIDTH_ATTRIBUTE, readGeometry } from "../src/geometry";
import type { EditOp } from "../src/operations";
import { createResize } from "../src/resize";
import type { EditorState } from "../src/session";

beforeAll(async () => {
  await init();
  await page.viewport(1_400, 900);
});

afterEach(() => document.body.replaceChildren());

describe("block widths at the browser boundary", () => {
  it("feeds intrinsic and explicit full geometry into the resize surface", async () => {
    const built = buildDeck("# Browser geometry\n\nFit.\n\n{width=full}\nExplicitly full.\n");
    const html = built.slides[0]?.html;
    if (!html) throw new Error("the fixture deck did not render");

    const frame = document.createElement("iframe");
    frame.className = "slidx-canvas-frame";
    frame.style.cssText = "display:block;border:0;width:960px;height:540px";
    frame.srcdoc = html;

    const loaded = new Promise<void>((resolve) => {
      frame.addEventListener("load", () => resolve(), { once: true });
    });
    document.body.append(frame);
    await loaded;
    await frame.contentDocument?.fonts.ready;

    const geometry = readGeometry(frame);
    if (!geometry) throw new Error("the editor could not read the rendered slide");

    const region = geometry.regions[0];
    const fitted = geometry.blocks[1];
    const full = geometry.blocks[2];
    if (!region || !fitted || !full) throw new Error("the fixture geometry is incomplete");

    const preview = frame.contentDocument!;
    const fittedElement = preview.querySelector<HTMLElement>("[data-slidx-block='1']")!;
    const fullElement = preview.querySelector<HTMLElement>("[data-slidx-block='2']")!;

    expect(fittedElement.hasAttribute(WIDTH_ATTRIBUTE)).toBe(false);
    expect(fitted.width).toBe("fit");
    expect(fitted.rect.width).toBeGreaterThan(0);
    expect(fitted.rect.width / region.rect.width).toBeLessThan(0.25);
    expect(
      Number.parseFloat(frame.contentWindow!.getComputedStyle(fittedElement).width),
    ).toBeCloseTo(fitted.rect.width, 1);

    expect(fullElement.getAttribute(WIDTH_ATTRIBUTE)).toBe("full");
    expect(full.width).toBe("full");
    expect(full.rect.width).toBeCloseTo(region.rect.width, 1);
    expect(Number.parseFloat(frame.contentWindow!.getComputedStyle(fullElement).width)).toBeCloseTo(
      region.rect.width,
      1,
    );

    const operations: EditOp[] = [];
    const resize = createResize({
      run: (operation) => operations.push(operation),
      foresee: () => {},
    });
    document.body.append(resize.root);
    resize.render(state());

    const handle = resize.root.querySelector<HTMLButtonElement>(
      ".slidx-resize-grip[data-block='1']",
    );
    if (!handle) throw new Error("the fitted block has no resize handle");

    const handleRect = handle.getBoundingClientRect();
    expect(handleRect.left + handleRect.width / 2).toBeCloseTo(
      fitted.rect.left + fitted.rect.width,
      1,
    );

    handle.focus();
    await userEvent.keyboard("{ArrowRight}");

    // The actual fitted box is narrower than the smallest named share, so the
    // first wider keyboard step is `quarter`, not a guess made from missing DOM
    // geometry and not the old full-region default.
    expect(operations).toEqual([{ op: "setBlockWidth", slide: 0, block: 1, width: "quarter" }]);
  });
});

function state(): EditorState {
  return {
    source: "",
    spans: [],
    slides: [],
    layouts: [],
    diagnostics: [],
    selection: { slide: 0 },
    viewers: [],
    canUndo: false,
    canRedo: false,
  };
}
