/**
 * The Docswell payload.
 *
 * The same deck and the same PDF as Speaker Deck, and deliberately not the same
 * payload. This file exists to hold that difference still: Docswell names the
 * blurb an overview, addresses a deck by a path with a minimum length, and
 * takes fewer, shorter tags.
 *
 * The failure mode guarded here is the tempting refactor. One shared payload
 * would need one set of limits — the intersection — and an author would
 * silently lose three thousand characters of a Speaker Deck description to a
 * cap that belongs to a site they were not publishing to. Several tests below
 * check the two targets *disagree* about the same deck, which is what makes
 * that refactor fail rather than pass quietly.
 */

import { describe, expect, it } from "vitest";

import { composeDocswell } from "../src/targets/docswell";
import { composeSpeakerDeck } from "../src/targets/speakerdeck";
import type { Composed } from "../src/types";
import { characters, deck, DESCRIPTION, TALK, without } from "./support";

function fieldsOf(result: Composed<unknown>): string[] {
  return result.ok ? [] : result.reasons.map((reason) => reason.field);
}

function messagesOf(result: Composed<unknown>): string {
  return result.ok ? "" : result.reasons.map((reason) => reason.message).join("\n");
}

describe("a complete deck", () => {
  it("maps the frontmatter onto Docswell's own field names", () => {
    const result = composeDocswell(deck());

    expect(result.ok).toBe(true);
    if (!result.ok) return;

    expect(result.value.title).toBe("Zero-JavaScript Slides");
    expect(result.value.overview).toBe(DESCRIPTION);
    expect(result.value.file).toBe("dist/deck.pdf");
  });

  it("derives the path from the title", () => {
    const result = composeDocswell(deck());

    expect(result.ok && result.value.path).toBe("zero-javascript-slides");
  });

  it("uses the author's path when they pinned one", () => {
    const result = composeDocswell(deck({ ...TALK, slug: "zero-js" }));

    expect(result.ok && result.value.path).toBe("zero-js");
  });

  it("shows where the talk was given", () => {
    const result = composeDocswell(deck());

    expect(result.ok && result.value.presentedAt).toBe("SlidxConf 2026");
  });

  it("omits the venue line for a deck that names no event", () => {
    const result = composeDocswell(deck(without(TALK, "event")));

    expect(result.ok && "presentedAt" in result.value).toBe(false);
  });
});

describe("limits that are not Speaker Deck's", () => {
  it("reports an overview that Speaker Deck would have accepted", () => {
    const source = deck({ ...TALK, description: characters(2000) });

    expect(composeSpeakerDeck(source).ok).toBe(true);
    expect(fieldsOf(composeDocswell(source))).toEqual(["description"]);
  });

  it("accepts an overview at exactly the limit", () => {
    const result = composeDocswell(deck({ ...TALK, description: characters(1000) }));

    expect(result.ok).toBe(true);
  });

  it("reports a tag list Speaker Deck would have accepted", () => {
    const tags = Array.from({ length: 12 }, (_, index) => `tag-${index}`);
    const source = deck({ ...TALK, tags });

    expect(composeSpeakerDeck(source).ok).toBe(true);
    expect(fieldsOf(composeDocswell(source))).toEqual(["tags"]);
  });

  it("reports a tag longer than Docswell stores but short enough elsewhere", () => {
    const source = deck({ ...TALK, tags: [characters(25)] });

    expect(composeSpeakerDeck(source).ok).toBe(true);
    expect(fieldsOf(composeDocswell(source))).toEqual(["tags"]);
  });

  it("fits a derived path to the shorter limit, on a word boundary", () => {
    const title = "How We Made A Presentation Framework That Ships No JavaScript At All";
    const result = composeDocswell(deck({ ...TALK, title }));

    expect(result.ok && result.value.path).toBe("how-we-made-a-presentation-framework-that-ships");
  });

  it("reports a title too short to make a path out of", () => {
    // Two characters is a valid title and not a valid Docswell path. Padding
    // it would invent an address; saying so does not.
    const result = composeDocswell(deck({ ...TALK, title: "Go" }));

    expect(fieldsOf(result)).toEqual(["slug"]);
    expect(messagesOf(result)).toContain("at least 3 characters");
  });

  it("names Docswell in its messages, not the other platform", () => {
    const result = composeDocswell(deck({ ...TALK, description: characters(2000) }));

    expect(messagesOf(result)).toContain("Docswell");
    expect(messagesOf(result)).not.toContain("Speaker Deck");
  });
});

describe("what is missing", () => {
  it("reports a deck built without a PDF", () => {
    const result = composeDocswell(deck(TALK, { artifacts: [] }));

    expect(fieldsOf(result)).toEqual(["pdf"]);
  });

  it("reports a missing title", () => {
    const result = composeDocswell(deck(without(TALK, "title")));

    expect(fieldsOf(result)).toContain("title");
  });
});
