/**
 * The Speaker Deck payload.
 *
 * This is the specification for the target where a mistake is most expensive:
 * the upload is a large file over a slow link, and a field two characters over
 * the cap fails *after* it has gone up, to someone who has just come off stage.
 *
 * So the failure modes guarded here are all about finding out early:
 *
 * - Every documented cap is checked before anything is handed over.
 * - A title the author wrote is never silently shortened to fit; a slug slidx
 *   derived is, because nobody chose it.
 * - Everything that is wrong is reported in one pass. Learning about the
 *   missing PDF only after fixing the title is two trips instead of one.
 */

import { describe, expect, it } from "vite-plus/test";

import { composeSpeakerDeck } from "../src/targets/speakerdeck";
import type { Composed } from "../src/types";
import { characters, deck, DESCRIPTION, PDF, TALK, without } from "./support";

/** The fields a composition named, or nothing when it succeeded. */
function fieldsOf(result: Composed<unknown>): string[] {
  return result.ok ? [] : result.reasons.map((reason) => reason.field);
}

/** Everything a composition said, as one string to assert against. */
function messagesOf(result: Composed<unknown>): string {
  return result.ok ? "" : result.reasons.map((reason) => reason.message).join("\n");
}

describe("a complete deck", () => {
  it("maps the frontmatter onto Speaker Deck's fields", () => {
    const result = composeSpeakerDeck(deck());

    expect(result.ok).toBe(true);
    if (!result.ok) return;

    expect(result.value.title).toBe("Zero-JavaScript Slides");
    expect(result.value.description).toBe(DESCRIPTION);
    expect(result.value.pdf).toBe("dist/deck.pdf");
  });

  it("derives the slug from the title", () => {
    const result = composeSpeakerDeck(deck());

    expect(result.ok && result.value.slug).toBe("zero-javascript-slides");
  });

  it("carries the talk date, which the deck page shows", () => {
    const result = composeSpeakerDeck(deck());

    expect(result.ok && result.value.date).toBe("2026-07-29");
  });

  it("omits the date rather than inventing today's", () => {
    // A plan that read a clock would not be diffable, and a deck given today's
    // date for last month's talk is simply wrong.
    const result = composeSpeakerDeck(deck(without(TALK, "date")));

    expect(result.ok && "date" in result.value).toBe(false);
  });

  it("publishes without a description, which the platform does not require", () => {
    const result = composeSpeakerDeck(deck(without(TALK, "description")));

    expect(result.ok && result.value.description).toBe("");
  });
});

describe("the slug", () => {
  it("uses the author's when they pinned one", () => {
    // A URL is an address other people have already written down. Reshaping it
    // breaks a link that exists.
    const result = composeSpeakerDeck(deck({ ...TALK, slug: "zero-js" }));

    expect(result.ok && result.value.slug).toBe("zero-js");
  });

  it("reports an authored slug the platform will not accept", () => {
    const result = composeSpeakerDeck(deck({ ...TALK, slug: "Zero JS!" }));

    expect(fieldsOf(result)).toEqual(["slug"]);
  });

  it("reports an authored slug that is too long rather than cutting it", () => {
    const result = composeSpeakerDeck(deck({ ...TALK, slug: characters(101) }));

    expect(fieldsOf(result)).toEqual(["slug"]);
    expect(messagesOf(result)).toContain("101 characters");
  });

  it("reports a title with no Latin characters to derive from", () => {
    // Falling back to a slide index would produce an address that means
    // nothing and changes when the deck is edited.
    const result = composeSpeakerDeck(deck({ ...TALK, title: "日本語のスライド" }));

    expect(fieldsOf(result)).toEqual(["slug"]);
    expect(messagesOf(result)).toContain("`slug:`");
  });

  it("accepts a Japanese title once a slug is pinned", () => {
    const result = composeSpeakerDeck(
      deck({ ...TALK, title: "日本語のスライド", slug: "nihongo" }),
    );

    expect(result.ok).toBe(true);
    expect(result.ok && result.value.title).toBe("日本語のスライド");
  });
});

describe("caps", () => {
  it("accepts a title at exactly the limit", () => {
    const result = composeSpeakerDeck(deck({ ...TALK, title: characters(100) }));

    expect(result.ok).toBe(true);
  });

  it("reports a title one character over, naming the field and the number", () => {
    const result = composeSpeakerDeck(deck({ ...TALK, title: characters(101) }));

    expect(fieldsOf(result)).toContain("title");
    expect(messagesOf(result)).toContain("101 characters");
  });

  it("accepts a description at exactly the limit", () => {
    const result = composeSpeakerDeck(deck({ ...TALK, description: characters(4000) }));

    expect(result.ok).toBe(true);
  });

  it("reports a description one character over", () => {
    const result = composeSpeakerDeck(deck({ ...TALK, description: characters(4001) }));

    expect(fieldsOf(result)).toEqual(["description"]);
  });

  it("reports a PDF over the upload size, in the units the message uses", () => {
    const artifacts = [{ ...PDF, bytes: 120 * 1024 * 1024 }];
    const result = composeSpeakerDeck(deck(TALK, { artifacts }));

    expect(fieldsOf(result)).toEqual(["pdf"]);
    expect(messagesOf(result)).toContain("120MB");
  });

  it("accepts a PDF whose size the caller did not measure", () => {
    // Opening the file to find out would make planning an IO operation, and a
    // plan that touches the disk can fail for reasons that have nothing to do
    // with the deck.
    const artifacts = [{ kind: "pdf" as const, path: "dist/deck.pdf" }];

    expect(composeSpeakerDeck(deck(TALK, { artifacts })).ok).toBe(true);
  });
});

describe("tags", () => {
  it("keeps the author's tags first, in the order they wrote them", () => {
    const result = composeSpeakerDeck(deck());

    expect(result.ok && result.value.tags.slice(0, 2)).toEqual(["rust", "slides"]);
  });

  it("adds the hashtag and the event, which no author writes twice", () => {
    const result = composeSpeakerDeck(deck());

    expect(result.ok && result.value.tags).toEqual([
      "rust",
      "slides",
      "slidxconf",
      "slidxconf-2026",
    ]);
  });

  it("adds nothing when the deck names no talk", () => {
    const result = composeSpeakerDeck(deck(without(TALK, "hashtag", "event")));

    expect(result.ok && result.value.tags).toEqual(["rust", "slides"]);
  });

  it("reports more authored tags than the platform stores", () => {
    // Dropping the tail of a hand-written list would publish a deck tagged
    // with whatever happened to sort first.
    const tags = Array.from({ length: 21 }, (_, index) => `tag-${index}`);
    const result = composeSpeakerDeck(deck({ ...TALK, tags }));

    expect(fieldsOf(result)).toEqual(["tags"]);
    expect(messagesOf(result)).toContain("remove 1");
  });

  it("drops its own suggestions rather than the author's tags at the cap", () => {
    const tags = Array.from({ length: 20 }, (_, index) => `tag-${index}`);
    const result = composeSpeakerDeck(deck({ ...TALK, tags }));

    expect(result.ok).toBe(true);
    expect(result.ok && result.value.tags).toEqual(tags);
  });

  it("reports a tag longer than the platform allows", () => {
    const result = composeSpeakerDeck(deck({ ...TALK, tags: [characters(31)] }));

    expect(fieldsOf(result)).toEqual(["tags"]);
  });
});

describe("what is missing", () => {
  it("reports a missing title by naming the frontmatter key", () => {
    const result = composeSpeakerDeck(deck(without(TALK, "title")));

    expect(fieldsOf(result)).toContain("title");
    expect(messagesOf(result)).toContain("`title:`");
  });

  it("reports a deck built without a PDF, and how to build one", () => {
    const result = composeSpeakerDeck(deck(TALK, { artifacts: [] }));

    expect(fieldsOf(result)).toEqual(["pdf"]);
    expect(messagesOf(result)).toContain("`pdf: true`");
  });

  it("reports everything wrong at once", () => {
    // Two problems found in one pass is one trip back to the frontmatter.
    const result = composeSpeakerDeck(deck(without(TALK, "title"), { artifacts: [] }));

    expect(fieldsOf(result)).toEqual(["title", "slug", "pdf"]);
  });
});
