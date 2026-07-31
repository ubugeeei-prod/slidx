/**
 * Seeing the half of a theme this machine is not set to.
 *
 * A deck follows `prefers-color-scheme`, so in the editor it follows the
 * author's laptop — and an author in dark mode never sees what a lit lecture
 * theatre will project, while one in daylight never sees the other. Both
 * palettes are in the file and only ever one is on screen.
 *
 * The override was already in the output. `slidx_theme::css` emits the dark
 * palette twice — once under the media query, once under `[data-scheme]` — and
 * says the second exists so a presenter can override it at a venue. Nothing had
 * ever written that attribute.
 */

import { describe, expect, it } from "vite-plus/test";

import { applyScheme, startingScheme } from "../src/canvas";

/** A document standing in for the one inside the canvas frame. */
function page(): Document {
  return new DOMParser().parseFromString("<!doctype html><html><body></body></html>", "text/html");
}

describe("putting a scheme on the deck's own document", () => {
  it("writes the attribute the theme already listens for", () => {
    const d = page();
    applyScheme(d, "dark");

    expect(d.documentElement.getAttribute("data-scheme")).toBe("dark");
  });

  it("takes it off for auto rather than writing the word", () => {
    // Removing is the whole of what makes automatic mean automatic: the media
    // query decides when no attribute is present, and an attribute spelled
    // `auto` matches neither override — leaving the light palette showing in a
    // dark room.
    const d = page();
    applyScheme(d, "dark");
    applyScheme(d, "auto");

    expect(d.documentElement.hasAttribute("data-scheme")).toBe(false);
  });

  it("replaces one choice with the other rather than accumulating", () => {
    const d = page();
    applyScheme(d, "dark");
    applyScheme(d, "light");

    expect(d.documentElement.getAttribute("data-scheme")).toBe("light");
  });

  it("does nothing at all when there is no document yet", () => {
    // The frame has not loaded. This runs on every load and on every click, and
    // one of those can arrive first.
    expect(() => applyScheme(null, "dark")).not.toThrow();
    expect(() => applyScheme(undefined, "light")).not.toThrow();
  });
});

describe("what the canvas shows before anybody chooses", () => {
  it("starts light, because a slide is a sheet of paper", () => {
    // A deck follows the room, which is right for a deck and wrong for an
    // editor: there the "room" is the author's laptop, so an author in dark
    // mode was authoring against a palette no lecture theatre will project.
    expect(startingScheme(undefined)).toBe("light");
  });

  it("remembers a choice over the default", () => {
    expect(startingScheme({ getItem: () => "dark" })).toBe("dark");
    expect(startingScheme({ getItem: () => "auto" })).toBe("auto");
  });

  it("ignores a stored value that is not a scheme", () => {
    // Storage is shared with everything else on the origin and outlives any
    // version of this editor.
    expect(startingScheme({ getItem: () => "chartreuse" })).toBe("light");
  });
});
