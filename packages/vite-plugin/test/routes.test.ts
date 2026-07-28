/**
 * Mapping a URL to a slide.
 *
 * The rule is that the deck's base *is* the first slide's URL, so the link
 * someone shares is the link to the start. Everything after it is numbered the
 * way a person counts slides, from one.
 *
 * The other half is what this must *not* claim. A deck lives inside a project
 * that has assets, other plugins, and the dev client; a URL that is not a
 * slide has to fall through untouched rather than 404.
 */

import { describe, expect, it } from "vitest";

import { slideIndexFor } from "../src/index";

describe("slide URLs", () => {
  it("serves the first slide at the base", () => {
    expect(slideIndexFor("/slides/", "slides")).toBe(0);
    expect(slideIndexFor("/slides", "slides")).toBe(0);
    expect(slideIndexFor("/slides/index.html", "slides")).toBe(0);
  });

  it("numbers the rest from two, as a person counts them", () => {
    expect(slideIndexFor("/slides/2/", "slides")).toBe(1);
    expect(slideIndexFor("/slides/2/index.html", "slides")).toBe(1);
    expect(slideIndexFor("/slides/10/", "slides")).toBe(9);
  });

  it("ignores a query string", () => {
    // `?step=3` is a deep link into the slide, not a different slide.
    expect(slideIndexFor("/slides/2/?step=3", "slides")).toBe(1);
  });

  it("serves the deck at the site root when asked to", () => {
    expect(slideIndexFor("/", "")).toBe(0);
    expect(slideIndexFor("/2/", "")).toBe(1);
  });
});

describe("what is not a slide", () => {
  it("leaves other routes alone", () => {
    expect(slideIndexFor("/about", "slides")).toBeNull();
    expect(slideIndexFor("/", "slides")).toBeNull();
  });

  it("leaves assets alone", () => {
    // These sit under the base and must still fall through, or the deck breaks
    // its own images.
    expect(slideIndexFor("/slides/logo.png", "slides")).toBeNull();
    expect(slideIndexFor("/slides/assets/x.css", "slides")).toBeNull();
  });

  it("leaves the dev client alone", () => {
    expect(slideIndexFor("/@vite/client", "slides")).toBeNull();
    expect(slideIndexFor("/node_modules/.vite/deps/x.js", "slides")).toBeNull();
  });

  it("rejects slide one written as a number", () => {
    // There is exactly one URL per slide. Two would split links and analytics
    // for the same content.
    expect(slideIndexFor("/slides/1/", "slides")).toBeNull();
    expect(slideIndexFor("/slides/0/", "slides")).toBeNull();
  });

  it("does not match a base that is only a prefix of the path", () => {
    expect(slideIndexFor("/slideshow/2/", "slides")).toBeNull();
  });
});
