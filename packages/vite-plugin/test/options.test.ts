/**
 * Option resolution.
 *
 * The shortest path has to be `slidx()` with nothing in it, so most of these
 * are about what happens when a field is absent — and about the several ways
 * people write the same value.
 */

import { describe, expect, it } from "vite-plus/test";

import { resolveOptions, slideFileName, slideRoute } from "../src/options";

describe("defaults", () => {
  it("needs no options at all", () => {
    const options = resolveOptions();

    expect(options.srcDir).toBe("slides");
    expect(options.base).toBe("slides");
    expect(options.separator).toBe("---");
    expect(options.extensions).toEqual([".md"]);
  });

  it("treats an empty object the same as nothing", () => {
    expect(resolveOptions({})).toEqual(resolveOptions());
  });

  it("stops a build on a blocking diagnostic unless told otherwise", () => {
    // A contrast failure cannot be fixed from the stage. The build is the last
    // place it is cheap.
    expect(resolveOptions().failOnDiagnostics).toBe(true);
    expect(resolveOptions({ failOnDiagnostics: false }).failOnDiagnostics).toBe(false);
  });
});

describe("normalising", () => {
  it("accepts a base with or without slashes", () => {
    for (const base of ["deck", "/deck", "deck/", "/deck/"]) {
      expect(resolveOptions({ base }).base).toBe("deck");
    }
  });

  it("allows the deck to sit at the site root", () => {
    expect(resolveOptions({ base: "/" }).base).toBe("");
    expect(slideRoute(resolveOptions({ base: "/" }), 0)).toBe("/");
  });

  it("accepts extensions with or without the dot", () => {
    expect(resolveOptions({ extensions: ["md", ".markdown"] }).extensions).toEqual([
      ".md",
      ".markdown",
    ]);
  });

  it("is case-insensitive about extensions", () => {
    expect(resolveOptions({ extensions: [".MD"] }).extensions).toEqual([".md"]);
  });

  it("drops duplicate extensions", () => {
    expect(resolveOptions({ extensions: ["md", ".md"] }).extensions).toEqual([".md"]);
  });

  it("falls back rather than accepting an empty extension list", () => {
    // An empty list would find no slides and report an empty deck, which looks
    // like the files are missing rather than like the config is wrong.
    expect(resolveOptions({ extensions: [] }).extensions).toEqual([".md"]);
    expect(resolveOptions({ extensions: ["  "] }).extensions).toEqual([".md"]);
  });

  it("falls back rather than accepting an empty srcDir", () => {
    expect(resolveOptions({ srcDir: "/" }).srcDir).toBe("slides");
  });
});

describe("routes", () => {
  it("puts the first slide at the base itself", () => {
    // `/slides/` rather than `/slides/1/`: the deck's URL should be the URL
    // people share.
    expect(slideRoute(resolveOptions(), 0)).toBe("/slides/");
  });

  it("numbers the rest from two", () => {
    expect(slideRoute(resolveOptions(), 1)).toBe("/slides/2/");
    expect(slideRoute(resolveOptions(), 9)).toBe("/slides/10/");
  });

  it("writes directory-style files so URLs need no extension", () => {
    expect(slideFileName(resolveOptions(), 0)).toBe("slides/index.html");
    expect(slideFileName(resolveOptions(), 1)).toBe("slides/2/index.html");
  });

  it("writes to the output root when the deck is the site", () => {
    const options = resolveOptions({ base: "/" });

    expect(slideFileName(options, 0)).toBe("index.html");
    expect(slideFileName(options, 1)).toBe("2/index.html");
  });
});
