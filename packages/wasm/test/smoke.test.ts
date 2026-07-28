/**
 * The WebAssembly module, exercised the way a consumer will use it.
 *
 * The Rust side is already tested natively; what these check is the
 * *boundary* — that the module loads in Node, that camelCase field names
 * survive serde, and that the shapes the plugin depends on are really there.
 * A native test cannot catch a field that fails to cross.
 */

import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

import { beforeAll, describe, expect, it } from "vitest";

import init, { buildDeck, themeCss, themeNames, version } from "../dist/slidx.js";

/**
 * Locates the built wasm relative to wherever the runner started.
 *
 * `import.meta.url` is not a file URL under Vite's module runner, so the
 * usual `new URL(..., import.meta.url)` does not work here. The runner may
 * start at the workspace root or at this package, so both are tried.
 */
function wasmPath(): string {
  const candidates = ["packages/wasm/dist/slidx_bg.wasm", "dist/slidx_bg.wasm"];
  const found = candidates.map((path) => resolve(process.cwd(), path)).find(existsSync);

  if (!found) {
    throw new Error("no built wasm — run `node scripts/build-wasm.mjs` first");
  }
  return found;
}

beforeAll(async () => {
  // `--target web` fetches its own wasm by URL. Node can read the file
  // directly, and handing over the bytes avoids depending on how a given
  // Node version resolves fetch against a file path.
  await init({ module_or_path: readFileSync(wasmPath()) });
});

describe("building a deck", () => {
  it("returns one entry per slide", () => {
    const result = buildDeck("# One\n\n---\n\n# Two\n");

    expect(result.slides).toHaveLength(2);
    expect(result.slides.map((slide) => slide.title)).toEqual(["One", "Two"]);
  });

  it("renders a complete page per slide", () => {
    const [slide] = buildDeck("# One\n").slides;

    expect(slide.html).toMatch(/^<!doctype html>/);
    expect(slide.html).toContain("</html>");
  });

  it("names fields in camelCase on this side of the boundary", () => {
    // serde renames on the Rust side; if that ever stops working the plugin
    // reads `undefined` everywhere and the failure is far from the cause.
    const [slide] = buildDeck("- a <!-- step -->\n").slides;

    expect(slide).toHaveProperty("stopCount");
    expect(slide).not.toHaveProperty("stop_count");
  });

  it("counts the stops the PDF exporter needs", () => {
    const [slide] = buildDeck("- a <!-- step -->\n- b <!-- step -->\n").slides;
    expect(slide.stopCount).toBe(3);
  });

  it("carries speaker notes without rendering them", () => {
    const [slide] = buildDeck("# One\n\n<!-- notes: out loud -->\n").slides;

    expect(slide.notes).toEqual(["out loud"]);
    expect(slide.html).not.toContain("out loud");
  });

  it("skips the HTML when asked to parse only", () => {
    const [slide] = buildDeck("# One\n", { parseOnly: true }).slides;

    expect(slide.html).toBeUndefined();
    expect(slide.title).toBe("One");
  });

  it("reports problems instead of throwing", () => {
    // A deck edited minutes before a talk has to render something.
    const result = buildDeck("---\nnot: [valid\n---\n\n# Still here\n");

    expect(result.slides).toHaveLength(1);
    expect(result.diagnostics.length).toBeGreaterThan(0);
    expect(result.diagnostics[0]).toHaveProperty("code");
  });

  it("points a diagnostic at the slide it is about", () => {
    const result = buildDeck('# One\n\n---\n\n---\nsteps:\n  - reveal: ""\n---\n\n# Two\n');
    expect(result.diagnostics.some((finding) => finding.slideIndex === 1)).toBe(true);
  });

  it("applies the theme it is given over the deck's own", () => {
    const source = "---\ntheme: minimal\n---\n\n# One\n";

    expect(buildDeck(source, { theme: "terminal" }).slides[0].html).not.toBe(
      buildDeck(source).slides[0].html,
    );
  });

  it("throws only when the options themselves are malformed", () => {
    expect(() => buildDeck("# One\n", { parseOnly: "yes" })).toThrow(/invalid options/);
  });
});

describe("themes", () => {
  it("lists the built-in names", () => {
    expect(themeNames()).toContain("minimal");
  });

  it("renders custom properties a shell can consume", () => {
    expect(themeCss("minimal")).toContain("--slidx-color-text:");
  });

  it("falls back rather than failing on an unknown name", () => {
    expect(themeCss("nope")).toContain("--slidx-color-text:");
  });
});

describe("the module itself", () => {
  it("reports the version it was built from", () => {
    expect(version()).toMatch(/^\d+\.\d+\.\d+$/);
  });
});
