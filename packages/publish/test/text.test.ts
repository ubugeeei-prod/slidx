/**
 * Counting and cutting, which is where every cap in this package is decided.
 *
 * This is the specification for the two things that are easy to get subtly
 * wrong and impossible to notice until a platform rejects an upload:
 *
 * - A character is a code point, not a UTF-16 unit. Counting units means an
 *   emoji costs two and a title is rejected for a limit it visibly did not
 *   reach.
 * - Cutting is word-aware where words exist and length-aware where they do
 *   not. A Japanese sentence has no space to cut on, and a rule that insists
 *   on one throws away most of the budget.
 */

import { describe, expect, it } from "vitest";

import {
  asciiSlug,
  countCharacters,
  fileSlug,
  fitSlug,
  normalizeTag,
  tidyBlock,
  truncate,
  uniqueTags,
} from "../src/text";

describe("counting characters", () => {
  it("counts an emoji as the one character a person sees", () => {
    // String#length says 2, which is the number that would reject a title
    // that fits.
    expect(countCharacters("🎤")).toBe(1);
    expect("🎤".length).toBe(2);
  });

  it("counts ordinary text as its length", () => {
    expect(countCharacters("slidx")).toBe(5);
  });

  it("counts CJK characters singly", () => {
    expect(countCharacters("日本語")).toBe(3);
  });
});

describe("truncating", () => {
  it("leaves text that already fits", () => {
    expect(truncate("short", 10)).toBe("short");
  });

  it("leaves text that is exactly at the limit", () => {
    expect(truncate("exactly-10", 10)).toBe("exactly-10");
  });

  it("counts the ellipsis against the budget", () => {
    // An ellipsis that pushed the result one character over would defeat the
    // entire point of truncating.
    const cut = truncate("The quick brown fox jumps over the lazy dog", 20);

    expect(countCharacters(cut)).toBeLessThanOrEqual(20);
    expect(cut.endsWith("…")).toBe(true);
  });

  it("cuts on a word boundary rather than mid-word", () => {
    expect(truncate("The quick brown fox", 12)).toBe("The quick…");
  });

  it("cuts by length when the script has no word boundaries", () => {
    // Japanese has no spaces. Insisting on a boundary here would return almost
    // nothing.
    expect(truncate("これは日本語の説明文です", 6)).toBe("これは日本…");
  });

  it("does not strand a single character before the ellipsis", () => {
    expect(truncate("anything", 1)).toBe("…");
  });

  it("returns nothing for a budget of nothing", () => {
    expect(truncate("anything", 0)).toBe("");
  });
});

describe("slugs for someone else's URL", () => {
  it("lowercases and hyphenates ASCII", () => {
    expect(asciiSlug("Zero-JavaScript Slides")).toBe("zero-javascript-slides");
  });

  it("collapses runs of punctuation into one hyphen", () => {
    expect(asciiSlug("Rust: fast, and — safe?")).toBe("rust-fast-and-safe");
  });

  it("has no leading or trailing hyphen", () => {
    expect(asciiSlug("  ...Slides!  ")).toBe("slides");
  });

  it("yields nothing for a title with no Latin characters", () => {
    // Reported by the caller as a missing `slug`, rather than invented. An
    // address that means nothing is worse than one the author chooses.
    expect(asciiSlug("日本語のスライド")).toBe("");
  });
});

describe("slugs for the author's own disk", () => {
  it("keeps Japanese, because a local file is not a URL", () => {
    expect(fileSlug("日本語のスライド")).toBe("日本語のスライド");
  });

  it("case-folds and hyphenates like the ASCII form", () => {
    expect(fileSlug("Zero-JavaScript Slides")).toBe("zero-javascript-slides");
  });

  it("keeps digits from any script", () => {
    expect(fileSlug("Rust 2026 の話")).toBe("rust-2026-の話");
  });
});

describe("fitting a derived slug", () => {
  it("leaves one that fits", () => {
    expect(fitSlug("zero-javascript-slides", 30)).toBe("zero-javascript-slides");
  });

  it("cuts on a hyphen so no word is left in half", () => {
    expect(fitSlug("zero-javascript-slides", 20)).toBe("zero-javascript");
  });

  it("cuts by length when there is no hyphen to cut on", () => {
    expect(fitSlug("internationalization", 10)).toBe("internatio");
  });
});

describe("tags", () => {
  it("drops the hash, folds case, and hyphenates spaces", () => {
    expect(normalizeTag("#Slidx Conf")).toBe("slidx-conf");
  });

  it("treats two spellings of one tag as one", () => {
    // They are one tag wherever they are actually stored, so publishing both
    // would show a visible duplicate.
    expect(uniqueTags(["Rust", "rust"])).toEqual(["rust"]);
  });

  it("keeps the author's order", () => {
    expect(uniqueTags(["slides", "rust", "wasm"])).toEqual(["slides", "rust", "wasm"]);
  });

  it("drops empties rather than emitting a blank tag", () => {
    expect(uniqueTags(["rust", "  ", "#"])).toEqual(["rust"]);
  });
});

describe("tidying a block", () => {
  it("collapses runs of blank lines so composed Markdown diffs cleanly", () => {
    expect(tidyBlock("one\n\n\n\ntwo")).toBe("one\n\ntwo");
  });

  it("normalises line endings", () => {
    expect(tidyBlock("one\r\ntwo")).toBe("one\ntwo");
  });
});
