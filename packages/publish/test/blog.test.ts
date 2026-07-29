/**
 * The blog scaffold.
 *
 * This is the specification for the target that is a draft rather than a
 * publication. The write-up does not get written because it starts from an
 * empty file at the end of a long day — but the prose already exists, in the
 * speaker notes, in order. So the scaffold's whole claim is that every word in
 * it is a word the author already wrote.
 *
 * The failure modes guarded here:
 *
 * - Emitting headings with nothing under them, which looks like work that has
 *   been done and is worse than no file at all.
 * - Unquoted YAML, which breaks on the colon in most conference talk titles —
 *   and breaks inside whatever reads the draft, long after this package ran.
 * - Losing slide order, which is the only structure a draft starts with.
 */

import { describe, expect, it } from "vite-plus/test";

import { composeBlog } from "../src/targets/blog";
import type { BlogScaffold, Composed } from "../src";
import { deck, slide, TALK, without } from "./support";

const NOTES = [
  slide(0, { title: "Why plain HTML", notes: ["A deck is a document."] }),
  slide(1, { notes: ["Nothing to hydrate."] }),
];

function fieldsOf(result: Composed<unknown>): string[] {
  return result.ok ? [] : result.reasons.map((reason) => reason.field);
}

function scaffold(source: Parameters<typeof composeBlog>[0]): BlogScaffold {
  const result = composeBlog(source);
  if (!result.ok) throw new Error(`blocked: ${fieldsOf(result).join(", ")}`);
  return result.value;
}

describe("the draft", () => {
  it("assembles frontmatter, the deck's summary, and one section per slide", () => {
    expect(scaffold(deck(TALK, { slides: NOTES })).markdown).toBe(
      [
        "---",
        'title: "Zero-JavaScript Slides"',
        'date: "2026-07-29"',
        'event: "SlidxConf 2026"',
        'slides: "https://slidx.dev/talks/zero-js"',
        'tags: ["rust", "slides"]',
        "---",
        "",
        "Why a deck should be plain HTML, and what it costs to keep it that way.",
        "",
        "## Why plain HTML",
        "",
        "A deck is a document.",
        "",
        "## Slide 2",
        "",
        "Nothing to hydrate.",
        "",
      ].join("\n"),
    );
  });

  it("uses the slide's own heading", () => {
    expect(scaffold(deck(TALK, { slides: NOTES })).sections[0]?.heading).toBe("Why plain HTML");
  });

  it("names an untitled slide by its position, as a placeholder to replace", () => {
    expect(scaffold(deck(TALK, { slides: NOTES })).sections[1]?.heading).toBe("Slide 2");
  });

  it("records which slide each section came from", () => {
    const sections = scaffold(deck(TALK, { slides: NOTES })).sections;

    expect(sections.map((section) => section.slide)).toEqual([0, 1]);
  });

  it("skips slides with no notes rather than emitting an empty heading", () => {
    // A title slide and a section divider belong to the talk, not to the
    // write-up.
    const slides = [slide(0, { title: "Title slide" }), ...NOTES];

    expect(scaffold(deck(TALK, { slides })).sections).toHaveLength(2);
  });

  it("joins several notes on one slide into paragraphs", () => {
    const slides = [slide(0, { title: "Why", notes: ["First point.", "Second point."] })];

    expect(scaffold(deck(TALK, { slides })).sections[0]?.body).toBe(
      "First point.\n\nSecond point.",
    );
  });

  it("follows slide order however the slides arrive", () => {
    const slides = [
      slide(1, { notes: ["Nothing to hydrate."] }),
      slide(0, { title: "Why plain HTML", notes: ["A deck is a document."] }),
    ];
    const sections = scaffold(deck(TALK, { slides })).sections;

    expect(sections.map((section) => section.slide)).toEqual([0, 1]);
  });

  it("writes nothing the author did not", () => {
    const body = scaffold(deck(TALK, { slides: NOTES })).sections[0]?.body;

    expect(body).toBe("A deck is a document.");
  });

  it("quotes the title, because most talk titles contain a colon", () => {
    // Unquoted, this is not YAML, and the failure surfaces in whatever static
    // site generator reads the draft rather than here.
    const source = deck({ ...TALK, title: "slidx: slides that ship nothing" }, { slides: NOTES });

    expect(scaffold(source).markdown).toContain('title: "slidx: slides that ship nothing"');
  });

  it("omits frontmatter keys the deck has no value for", () => {
    const source = deck(without(TALK, "event", "url", "tags"), { slides: NOTES });

    expect(scaffold(source).markdown).not.toContain("event:");
    expect(scaffold(source).markdown).not.toContain("slides:");
    expect(scaffold(source).markdown).not.toContain("tags:");
  });

  it("composes the same draft every time", () => {
    const source = deck(TALK, { slides: NOTES });

    expect(scaffold(source).markdown).toBe(scaffold(source).markdown);
  });
});

describe("the file name", () => {
  it("leads with the date so a directory of drafts sorts by talk", () => {
    expect(scaffold(deck(TALK, { slides: NOTES })).path).toBe(
      "2026-07-29-zero-javascript-slides.md",
    );
  });

  it("keeps Japanese, because this is a file rather than a URL", () => {
    const source = deck({ ...TALK, title: "日本語のスライド" }, { slides: NOTES });

    expect(scaffold(source).path).toBe("2026-07-29-日本語のスライド.md");
  });

  it("drops the date prefix for a deck that has no date", () => {
    const source = deck(without(TALK, "date"), { slides: NOTES });

    expect(scaffold(source).path).toBe("zero-javascript-slides.md");
  });
});

describe("what is missing", () => {
  it("reports a deck with no title", () => {
    const result = composeBlog(deck(without(TALK, "title"), { slides: NOTES }));

    expect(fieldsOf(result)).toEqual(["title"]);
  });

  it("reports a deck whose slides carry no notes", () => {
    // A scaffold of empty headings looks like a draft that has been started.
    const result = composeBlog(deck(TALK, { slides: [slide(0, { title: "Why" })] }));

    expect(fieldsOf(result)).toEqual(["notes"]);
  });

  it("reports a deck with no slides at all", () => {
    expect(fieldsOf(composeBlog(deck()))).toEqual(["notes"]);
  });

  it("reports both a missing title and missing notes at once", () => {
    expect(fieldsOf(composeBlog(deck(without(TALK, "title"))))).toEqual(["title", "notes"]);
  });
});
