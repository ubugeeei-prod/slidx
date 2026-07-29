/**
 * The resources page, and the link collection under it.
 *
 * This is the specification for the one thing an audience actually asks for
 * afterwards. Every talk has the moment where a URL appears on a projector and
 * forty people photograph it, so the page is worth generating only if it is
 * exhaustive and only if it is in the order people remember.
 *
 * The failure modes guarded here:
 *
 * - Listing example endpoints out of code blocks. `https://api.example.com/v1`
 *   on a resources page sends people somewhere that does not exist.
 * - Listing the same link twice because it was written two different ways, or
 *   once with a capital in the host.
 * - Sorting. Alphabetical order breaks the only thing that lets a reader match
 *   an entry against the slide they remember, so slide order is the order.
 * - Swallowing the next entry, when a label taken off a slide contains a
 *   bracket.
 */

import { describe, expect, it } from "vite-plus/test";

import { collectLinks } from "../src/links";
import { composeResources } from "../src/targets/resources";
import type { Composed, ResourcesPage } from "../src";
import { deck, slide, TALK, without } from "./support";

/** A deck whose repo is absent, so a test is about the slides alone. */
const SLIDES_ONLY = without(TALK, "repo");

function linksOf(slides: ReturnType<typeof slide>[]): Array<{ url: string; label: string }> {
  return collectLinks(deck(SLIDES_ONLY, { slides })).map(({ url, label }) => ({ url, label }));
}

function urlsOf(slides: ReturnType<typeof slide>[]): string[] {
  return collectLinks(deck(SLIDES_ONLY, { slides })).map((link) => link.url);
}

function fieldsOf(result: Composed<unknown>): string[] {
  return result.ok ? [] : result.reasons.map((reason) => reason.field);
}

function page(source: Parameters<typeof composeResources>[0]): ResourcesPage {
  const result = composeResources(source);
  if (!result.ok) throw new Error(`blocked: ${fieldsOf(result).join(", ")}`);
  return result.value;
}

describe("finding links", () => {
  it("reads an inline link, keeping the author's words as the label", () => {
    expect(linksOf([slide(0, { content: "See [the docs](https://slidx.dev/docs)." })])).toEqual([
      { url: "https://slidx.dev/docs", label: "the docs" },
    ]);
  });

  it("reads a link with a title attribute", () => {
    expect(urlsOf([slide(0, { content: '[docs](https://slidx.dev "The docs")' })])).toEqual([
      "https://slidx.dev",
    ]);
  });

  it("reads an autolink", () => {
    expect(urlsOf([slide(0, { content: "<https://slidx.dev>" })])).toEqual(["https://slidx.dev"]);
  });

  it("reads a reference definition, labelled with its reference", () => {
    expect(linksOf([slide(0, { content: "[docs]: https://slidx.dev/docs" })])).toEqual([
      { url: "https://slidx.dev/docs", label: "docs" },
    ]);
  });

  it("reads a bare URL, without the sentence's full stop", () => {
    expect(urlsOf([slide(0, { content: "See https://slidx.dev/docs." })])).toEqual([
      "https://slidx.dev/docs",
    ]);
  });

  it("keeps parentheses that belong to the URL", () => {
    // Wikipedia and MDN both publish paths with balanced brackets in them.
    const content = "https://en.wikipedia.org/wiki/Deck_(cards)";

    expect(urlsOf([slide(0, { content })])).toEqual(["https://en.wikipedia.org/wiki/Deck_(cards)"]);
  });

  it("drops the bracket that closed the sentence rather than the URL", () => {
    const content = "(see https://slidx.dev/docs)";

    expect(urlsOf([slide(0, { content })])).toEqual(["https://slidx.dev/docs"]);
  });

  it("falls back to the URL itself when there is no link text", () => {
    expect(linksOf([slide(0, { content: "https://slidx.dev/docs" })])).toEqual([
      { url: "https://slidx.dev/docs", label: "slidx.dev/docs" },
    ]);
  });

  it("reads links out of speaker notes too", () => {
    // A link the speaker read aloud and never put on a slide is exactly the
    // one someone comes looking for afterwards.
    expect(urlsOf([slide(0, { notes: ["Mentioned https://slidx.dev/docs here."] })])).toEqual([
      "https://slidx.dev/docs",
    ]);
  });

  it("attributes a link on the slide before the same link in its notes", () => {
    const slides = [
      slide(0, {
        content: "[on screen](https://slidx.dev/a)",
        notes: ["[in the notes](https://slidx.dev/b)"],
      }),
    ];

    expect(urlsOf(slides)).toEqual(["https://slidx.dev/a", "https://slidx.dev/b"]);
  });
});

describe("what is not a link", () => {
  it("ignores URLs inside a fenced code block", () => {
    const content = ["```js", 'fetch("https://api.example.com/v1");', "```"].join("\n");

    expect(urlsOf([slide(0, { content })])).toEqual([]);
  });

  it("ignores a fence closed by a longer marker", () => {
    const content = ["~~~", "https://api.example.com/v1", "~~~~", "https://slidx.dev"].join("\n");

    expect(urlsOf([slide(0, { content })])).toEqual(["https://slidx.dev"]);
  });

  it("ignores URLs inside an inline code span", () => {
    const content = "Run against `https://localhost:3000` while developing.";

    expect(urlsOf([slide(0, { content })])).toEqual([]);
  });

  it("ignores an image, which is an asset rather than a resource", () => {
    const content = "![diagram](https://cdn.example.com/diagram.png)";

    expect(urlsOf([slide(0, { content })])).toEqual([]);
  });

  it("ignores a mailto address", () => {
    expect(urlsOf([slide(0, { content: "[mail](mailto:me@example.com)" })])).toEqual([]);
  });

  it("ignores a link within the deck", () => {
    expect(urlsOf([slide(0, { content: "[slide two](../2/)" })])).toEqual([]);
  });
});

describe("ordering and duplicates", () => {
  it("lists links in slide order", () => {
    const slides = [
      slide(1, { content: "[b](https://slidx.dev/b)" }),
      slide(0, { content: "[a](https://slidx.dev/a)" }),
    ];

    expect(urlsOf(slides)).toEqual(["https://slidx.dev/a", "https://slidx.dev/b"]);
  });

  it("keeps the first label when a link appears twice", () => {
    // The first mention is where it was introduced, so its text is the one
    // most likely to say what it is.
    const slides = [
      slide(0, { content: "[the parser docs](https://slidx.dev/docs)" }),
      slide(1, { content: "[docs again](https://slidx.dev/docs)" }),
    ];

    expect(linksOf(slides)).toEqual([{ url: "https://slidx.dev/docs", label: "the parser docs" }]);
  });

  it("treats a host written with capitals as the same link", () => {
    const slides = [
      slide(0, { content: "https://slidx.dev/docs" }),
      slide(1, { content: "https://SLIDX.dev/docs" }),
    ];

    expect(urlsOf(slides)).toHaveLength(1);
  });

  it("treats two anchors of one page as two links", () => {
    // A fragment addresses a section. Collapsing them loses the part that made
    // each worth listing.
    const slides = [
      slide(0, { content: "https://slidx.dev/docs#steps" }),
      slide(1, { content: "https://slidx.dev/docs#themes" }),
    ];

    expect(urlsOf(slides)).toHaveLength(2);
  });

  it("puts the repository first, ahead of any slide", () => {
    const slides = [slide(0, { content: "[docs](https://slidx.dev/docs)" })];
    const links = collectLinks(deck(TALK, { slides }));

    expect(links[0]?.url).toBe("https://github.com/ubugeeei-prod/slidx");
    expect(links[0]?.slide).toBe(null);
  });

  it("does not list the repository twice when a slide links it too", () => {
    const slides = [slide(0, { content: "[repo](https://github.com/ubugeeei-prod/slidx)" })];

    expect(collectLinks(deck(TALK, { slides }))).toHaveLength(1);
  });

  it("does not list the deck's own url", () => {
    // A page of resources that links to the page it is part of is a loop.
    expect(collectLinks(deck(without(TALK, "repo"), { slides: [] }))).toEqual([]);
  });
});

describe("the page", () => {
  const SLIDES = [
    slide(0, { content: "See [the docs](https://slidx.dev/docs)." }),
    slide(1, { notes: ["And https://slidx.dev/themes"] }),
  ];

  it("is a list, in slide order, under a heading naming the deck", () => {
    expect(page(deck(SLIDES_ONLY, { slides: SLIDES })).markdown).toBe(
      [
        "# Resources — Zero-JavaScript Slides",
        "",
        "- [the docs](https://slidx.dev/docs)",
        "- [slidx.dev/themes](https://slidx.dev/themes)",
        "",
      ].join("\n"),
    );
  });

  it("stands on its own for a deck with no frontmatter at all", () => {
    // The one target that needs nothing from the author but the deck itself.
    expect(page(deck({}, { slides: SLIDES })).title).toBe("Resources");
  });

  it("escapes a bracket in a label rather than swallowing the next entry", () => {
    const slides = [slide(0, { content: "[draft [notes](https://slidx.dev/x)" })];

    expect(page(deck(SLIDES_ONLY, { slides })).markdown).toContain(
      "- [draft \\[notes](https://slidx.dev/x)",
    );
  });

  it("suggests a file name", () => {
    expect(page(deck(SLIDES_ONLY, { slides: SLIDES })).path).toBe("resources.md");
  });

  it("reports a deck that links to nothing, and how to fix it", () => {
    const result = composeResources(deck(SLIDES_ONLY, { slides: [slide(0, {})] }));

    expect(fieldsOf(result)).toEqual(["links"]);
  });

  it("writes the same page every time", () => {
    const source = deck(SLIDES_ONLY, { slides: SLIDES });

    expect(page(source).markdown).toBe(page(source).markdown);
  });
});
