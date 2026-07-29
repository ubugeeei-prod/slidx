/**
 * The post that announces the slides.
 *
 * This is the specification for the only target with a hard character budget,
 * and the budget is what makes it worth specifying. The rule the tests exist to
 * pin down:
 *
 * **The link and the hashtag survive; the description is what gets cut.** A
 * post that loses a clause is a slightly worse post. A post that loses its URL
 * announced nothing, and a post that loses its hashtag is invisible to everyone
 * following the conference — both are worse failures than a sentence ending in
 * an ellipsis.
 *
 * The other failure modes guarded here:
 *
 * - Counting characters as UTF-16 units, which rejects a post with an emoji in
 *   it that visibly fits.
 * - Emitting a post with no link because the deck has no `url`, which is a
 *   dangling announcement rather than a missing one.
 * - Cutting a description to a four-word stub, which costs the characters that
 *   made the rest readable and reads as a bug.
 */

import { describe, expect, it } from "vitest";

import { composeSocial } from "../src/targets/social";
import { countCharacters } from "../src/text";
import type { Composed, SocialPost } from "../src";
import { characters, deck, DESCRIPTION, PDF, TALK, without } from "./support";

const URL = "https://slidx.dev/talks/zero-js";
const TAIL = `${URL} #slidxconf`;

function fieldsOf(result: Composed<unknown>): string[] {
  return result.ok ? [] : result.reasons.map((reason) => reason.field);
}

function messagesOf(result: Composed<unknown>): string {
  return result.ok ? "" : result.reasons.map((reason) => reason.message).join("\n");
}

/** Composes, failing the test rather than the type check when blocked. */
function post(...args: Parameters<typeof composeSocial>): SocialPost {
  const result = composeSocial(...args);
  if (!result.ok) throw new Error(`blocked: ${messagesOf(result)}`);
  return result.value;
}

describe("composing", () => {
  it("puts the talk first, the description next, and the link last", () => {
    expect(post(deck()).text).toBe(
      "Zero-JavaScript Slides — SlidxConf 2026\n\n" +
        "Why a deck should be plain HTML, and what it costs to keep it that way.\n\n" +
        TAIL,
    );
  });

  it("says how long it is, in the characters it counted", () => {
    const written = post(deck());

    expect(written.length).toBe(countCharacters(written.text));
    expect(written.limit).toBe(280);
    expect(written.truncated).toBe(false);
  });

  it("uses only the deck's own words", () => {
    // No "Slides are up!". Boilerplate here is slidx's voice inserted into the
    // timeline of an author who wrote their deck in another language.
    const text = post(deck()).text;

    expect(text.startsWith("Zero-JavaScript Slides")).toBe(true);
    expect(text.endsWith(TAIL)).toBe(true);
  });

  it("drops the event from the lead when the deck names none", () => {
    expect(post(deck(without(TALK, "event"))).text.startsWith("Zero-JavaScript Slides\n\n")).toBe(
      true,
    );
  });

  it("ends with the bare url when the deck has no hashtag", () => {
    expect(post(deck(without(TALK, "hashtag"))).text.endsWith(URL)).toBe(true);
  });

  it("writes a hashtag the way a platform stores one", () => {
    expect(post(deck({ ...TALK, hashtag: "#Slidx Conf" })).text.endsWith("#slidx-conf")).toBe(true);
  });

  it("attaches the card the build produced", () => {
    const card = { kind: "card" as const, path: "dist/card.png" };

    expect(post(deck(TALK, { artifacts: [PDF, card] })).image).toBe("dist/card.png");
  });

  it("has no image when the build made none", () => {
    expect("image" in post(deck())).toBe(false);
  });

  it("plans the same post every time", () => {
    // A post that differed between runs could not be reviewed before it was
    // sent, which is the only reason to compose it ahead of time.
    expect(post(deck()).text).toBe(post(deck()).text);
  });
});

describe("the budget", () => {
  it("keeps a description that fits, whole", () => {
    const written = post(deck());

    expect(written.text).toContain(DESCRIPTION);
    expect(written.truncated).toBe(false);
  });

  it("cuts the description rather than the link", () => {
    const written = post(deck({ ...TALK, description: characters(500) }));

    expect(written.text).toContain(URL);
    expect(written.text.endsWith(TAIL)).toBe(true);
    expect(written.truncated).toBe(true);
  });

  it("lands exactly on the budget rather than under it", () => {
    // Characters left unspent are characters of the author's description
    // thrown away for nothing.
    expect(post(deck({ ...TALK, description: characters(500) })).length).toBe(280);
  });

  it("keeps the hashtag when the description is cut", () => {
    // A post that loses its hashtag is invisible to the conference following
    // it, which is most of the reason to post at all.
    expect(post(deck({ ...TALK, description: characters(500) })).text).toContain("#slidxconf");
  });

  it("keeps the title whole when the description is cut", () => {
    expect(
      post(deck({ ...TALK, description: characters(500) })).text.startsWith(
        "Zero-JavaScript Slides — SlidxConf 2026",
      ),
    ).toBe(true);
  });

  it("cuts on a word boundary", () => {
    const written = post(deck({ ...TALK, description: "word ".repeat(60) }));
    const body = written.text.split("\n\n")[1] ?? "";

    expect(body.endsWith("word…")).toBe(true);
  });

  it("cuts by length when the description has no word boundaries", () => {
    // Japanese has no spaces to cut on, and a rule that needed one would throw
    // away the whole budget.
    const written = post(deck({ ...TALK, description: "これは日本語の説明文です".repeat(40) }));

    expect(written.length).toBe(280);
    expect(written.text).toContain(TAIL);
  });

  it("drops the description entirely rather than leaving a stub", () => {
    const written = post(deck(), { limit: 100 });

    expect(written.text).not.toContain("Why a deck");
    expect(written.text).not.toContain("…");
    expect(written.truncated).toBe(true);
  });

  it.each([90, 120, 180, 280, 400])("never exceeds a budget of %i", (limit) => {
    const written = post(deck({ ...TALK, description: characters(900) }), { limit });

    expect(written.length).toBeLessThanOrEqual(limit);
    expect(written.text).toContain(URL);
    expect(written.text).toContain("#slidxconf");
  });
});

describe("counting", () => {
  it("counts an emoji in the title as one character", () => {
    // Counting UTF-16 units would make this post one over a budget it fits.
    const source = deck(without({ ...TALK, title: "🎤 Zero-JavaScript Slides" }, "description"));
    const written = post(source);

    expect(written.length).toBe(85);
    expect(written.text.length).toBe(86);
  });

  it("accepts a post that is exactly the budget", () => {
    const source = deck(without({ ...TALK, title: "🎤 Zero-JavaScript Slides" }, "description"));

    expect(composeSocial(source, { limit: 85 }).ok).toBe(true);
  });

  it("reports a post one character over", () => {
    const source = deck(without({ ...TALK, title: "🎤 Zero-JavaScript Slides" }, "description"));

    expect(fieldsOf(composeSocial(source, { limit: 84 }))).toEqual(["title"]);
  });
});

describe("what is missing", () => {
  it("reports a deck with no url rather than posting a dangling announcement", () => {
    const result = composeSocial(deck(without(TALK, "url")));

    expect(fieldsOf(result)).toEqual(["url"]);
    expect(messagesOf(result)).toContain("`url:`");
  });

  it("reports a deck with no title", () => {
    expect(fieldsOf(composeSocial(deck(without(TALK, "title"))))).toEqual(["title"]);
  });

  it("reports both at once", () => {
    expect(fieldsOf(composeSocial(deck(without(TALK, "title", "url"))))).toEqual(["title", "url"]);
  });

  it("names the url when it alone will not fit the budget", () => {
    const result = composeSocial(deck(), { limit: 30 });

    expect(fieldsOf(result)).toEqual(["url"]);
    expect(messagesOf(result)).toContain("42 characters");
  });

  it("names the title when the title is what overflows", () => {
    // The URL cannot usefully be shortened by the author; the title can, so
    // that is the field the message names.
    const result = composeSocial(deck({ ...TALK, title: characters(300) }));

    expect(fieldsOf(result)).toEqual(["title"]);
    expect(messagesOf(result)).toContain("shorten `title`");
  });
});
