/**
 * Turning three selected words into the byte range an operation names.
 *
 * The one place the editor bridges the rendered slide and the Markdown behind
 * it. It searches rather than maps, and the tests say exactly what that buys
 * and what it does not.
 */

import { describe, expect, it } from "vitest";

import { byteLength, sliceBytes } from "../src/bytes";
import { locateSelection, occurrenceInRendered } from "../src/selection";

describe("locating a selection in the Markdown behind it", () => {
  it("names the bytes the selected words occupy", () => {
    const body = "The result was 3.2x faster than before.";
    const found = locateSelection(body, "3.2x faster");

    expect(found).toEqual({ range: { start: 15, end: 26 }, text: "3.2x faster" });
    expect(body.slice(15, 26)).toBe("3.2x faster");
  });

  it("counts in bytes, not in the units a browser counts in", () => {
    // A deck written in Japanese diverges from a UTF-16 index on the first
    // character. A range that is out by two bytes cuts a character in half,
    // and the pipeline refuses it — correctly, and confusingly.
    const body = "結果は 3.2倍 速くなりました";
    const found = locateSelection(body, "3.2倍");

    expect(found).toEqual({ range: { start: 10, end: 16 }, text: "3.2倍" });
    expect(sliceBytes(body, 10, 16)).toBe("3.2倍");
  });

  it("picks the appearance the author selected, not the first one", () => {
    const body = "fast is fast because fast is measured.";

    expect(locateSelection(body, "fast", 1)).toEqual({
      range: { start: 8, end: 12 },
      text: "fast",
    });
    expect(locateSelection(body, "fast", 2)).toEqual({
      range: { start: 21, end: 25 },
      text: "fast",
    });
  });

  it("falls back to the first appearance when the source spells the others differently", () => {
    // One of them is already inside a mark, so the source has fewer plain
    // copies than the screen does. The first is what the author means far more
    // often than nothing is.
    const body = "Fast is [fast]{.accent} because fast is measured.";

    expect(locateSelection(body, "fast", 2)).toMatchObject({ range: { start: 9, end: 13 } });
  });

  it("says a selection cannot be addressed rather than guessing at it", () => {
    // A phrase the renderer produced and the source does not contain — a link's
    // text, a heading with its hashes stripped, a hard-wrapped line.
    expect(locateSelection("See the [resources page](/links).", "resources page here")).toEqual({
      problem: "not-found",
    });
    expect(locateSelection("Anything", "   ")).toEqual({ problem: "empty" });
  });
});

describe("which appearance was selected", () => {
  it("counts the copies before it in the text a reader sees", () => {
    const rendered = "fast, then fast, then fast";

    expect(occurrenceInRendered(rendered, "fast", 0)).toBe(0);
    expect(occurrenceInRendered(rendered, "fast", 11)).toBe(1);
    expect(occurrenceInRendered(rendered, "fast", 22)).toBe(2);
  });
});

describe("byte lengths", () => {
  it("measure what a file holds rather than what a string holds", () => {
    expect(byteLength("abc")).toBe(3);
    expect(byteLength("日本語")).toBe(9);
    expect("日本語".length).toBe(3);
  });

  it("give nothing back for a range that names nothing", () => {
    expect(sliceBytes("abc", 2, 2)).toBe("");
    expect(sliceBytes("abc", 5, 1)).toBe("");
  });
});
