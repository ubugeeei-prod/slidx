/**
 * The wire, and what is not allowed onto it.
 *
 * This is the specification for every message the audience channel carries. It
 * is the file the rest of the package is checked against, because both halves
 * import these functions and neither is allowed a private opinion about what a
 * question is.
 *
 * The failure modes it guards are the ones an anonymous public channel actually
 * meets:
 *
 * - A frame large enough to be a denial of service, refused before it is parsed.
 * - A "question" that is really two hundred newlines, or a bidi override that
 *   reverses what a moderator approved after they approved it.
 * - Invisible padding used to slip past a length cap.
 * - A cap measured in the wrong unit, which would silently give a Japanese deck
 *   half the room an English one gets.
 * - A room slug that is really a path, a case variant, or a way to allocate
 *   unbounded storage on somebody else's Worker.
 */

import { describe, expect, it } from "vitest";

import {
  checkName,
  checkText,
  isRoomSlug,
  LIMITS,
  parseServerFrame,
  REACTION_KINDS,
  sanitizeText,
  textLength,
  validateFrame,
  type RoomSnapshot,
} from "../src/protocol";

const frame = (value: unknown) => validateFrame(JSON.stringify(value));

const question = (text: string, name?: string) =>
  frame({ type: "question", text, ...(name === undefined ? {} : { name }) });

describe("frame size", () => {
  it("refuses a frame past the byte cap", () => {
    const oversized = JSON.stringify({ type: "question", text: "x".repeat(LIMITS.frameBytes) });

    expect(validateFrame(oversized)).toEqual({ ok: false, reason: "too-large" });
  });

  it("measures bytes rather than characters", () => {
    // 1,000 CJK characters is 3,000 bytes of UTF-8 but only 1,000 UTF-16 units.
    // A cap that counted units would let it through and then hold it in memory.
    const wide = JSON.stringify({ type: "question", text: "質".repeat(1_000) });

    expect(wide.length).toBeLessThan(LIMITS.frameBytes);
    expect(validateFrame(wide)).toEqual({ ok: false, reason: "too-large" });
  });
});

describe("frame shape", () => {
  it("refuses something that is not JSON", () => {
    expect(validateFrame("not json")).toEqual({ ok: false, reason: "malformed" });
  });

  it("refuses an array, which a typeof check would call an object", () => {
    expect(frame([{ type: "question", text: "hi" }])).toEqual({ ok: false, reason: "malformed" });
  });

  it("refuses null", () => {
    expect(frame(null)).toEqual({ ok: false, reason: "malformed" });
  });

  it("refuses a message type it does not know", () => {
    // Refused rather than ignored: a client talking to a room that cannot
    // understand it should hear so, not watch its questions disappear.
    expect(frame({ type: "shout", text: "hi" })).toEqual({ ok: false, reason: "malformed" });
  });
});

describe("a question", () => {
  it("is accepted with just text", () => {
    expect(question("How does the parser handle nested fences?")).toEqual({
      ok: true,
      value: { type: "question", text: "How does the parser handle nested fences?" },
    });
  });

  it("is refused when the text is missing", () => {
    expect(frame({ type: "question" })).toEqual({ ok: false, reason: "malformed" });
  });

  it("is refused when the text is not a string", () => {
    expect(frame({ type: "question", text: 42 })).toEqual({ ok: false, reason: "malformed" });
  });

  it("is refused when nothing survives normalising", () => {
    expect(question("   \n\t  ")).toEqual({ ok: false, reason: "empty" });
  });

  it("is refused past the character cap", () => {
    expect(question("a".repeat(LIMITS.questionText + 1))).toEqual({
      ok: false,
      reason: "too-long",
    });
  });

  it("is refused rather than truncated", () => {
    // Silently publishing half of what somebody wrote puts words on a screen
    // that nobody said.
    const outcome = question("a".repeat(LIMITS.questionText + 40));

    expect(outcome.ok).toBe(false);
  });

  it("counts the cap in code points, so an emoji question gets the same room", () => {
    const emoji = "🙂".repeat(LIMITS.questionText);

    expect(textLength(emoji)).toBe(LIMITS.questionText);
    expect(question(emoji).ok).toBe(true);
    expect(question(`${emoji}🙂`)).toEqual({ ok: false, reason: "too-long" });
  });
});

describe("normalising", () => {
  it("collapses a wall of newlines into one line", () => {
    // Otherwise a question is a way to push everything else off the screen.
    expect(sanitizeText("why\n\n\n\n\nthough")).toBe("why though");
  });

  it("strips control characters", () => {
    expect(sanitizeText("hello\u0000wor\u0007ld")).toBe("hello wor ld");
  });

  it("strips bidi overrides", () => {
    // A right-to-left override reverses everything after it, so what reaches
    // the projector is not what the moderator read before approving it.
    expect(sanitizeText("safe \u202Ereversed")).toBe("safe reversed");
  });

  it("strips zero-width padding used to pass a length cap", () => {
    expect(sanitizeText("a\u200B\uFEFFb")).toBe("a b");
  });

  it("keeps the joiners that ordinary writing needs", () => {
    // ZWJ builds emoji sequences and ZWNJ is load-bearing in Persian and Indic
    // text. Stripping them would corrupt real writing to prevent nothing.
    expect(sanitizeText("\u{1F469}\u200D\u{1F4BB}")).toBe("\u{1F469}\u200D\u{1F4BB}");
    expect(sanitizeText("\u0645\u06CC\u200C\u062E")).toBe("\u0645\u06CC\u200C\u062E");
  });

  it("composes decomposed text so the cap counts what is displayed", () => {
    const decomposed = "e\u0301".repeat(200);

    expect(textLength(sanitizeText(decomposed))).toBe(200);
    expect(checkText(decomposed, 200).ok).toBe(true);
  });

  it("trims, so a question padded with spaces is measured on its content", () => {
    expect(sanitizeText("  spaced  out  ")).toBe("spaced out");
  });
});

describe("a display name", () => {
  it("is optional", () => {
    expect(checkName(undefined)).toEqual({ ok: true, value: undefined });
  });

  it("is treated as absent when it normalises away", () => {
    // Nobody should be stopped from asking by the field they did not want.
    expect(checkName("   ")).toEqual({ ok: true, value: undefined });
  });

  it("is refused past its own cap", () => {
    expect(checkName("n".repeat(LIMITS.displayName + 1))).toEqual({
      ok: false,
      reason: "too-long",
    });
  });

  it("is refused when it is not a string", () => {
    expect(checkName({ toString: "sneaky" })).toEqual({ ok: false, reason: "malformed" });
  });

  it("rides along with a question when it is given", () => {
    expect(question("why?", " Ada ")).toEqual({
      ok: true,
      value: { type: "question", text: "why?", name: "Ada" },
    });
  });
});

describe("an upvote", () => {
  it("carries an identifier and nothing else", () => {
    expect(frame({ type: "upvote", questionId: "000001" })).toEqual({
      ok: true,
      value: { type: "upvote", questionId: "000001" },
    });
  });

  it("refuses an identifier with a path in it", () => {
    expect(frame({ type: "upvote", questionId: "../meta" })).toEqual({
      ok: false,
      reason: "malformed",
    });
  });

  it("refuses an empty identifier", () => {
    expect(frame({ type: "upvote", questionId: "" })).toEqual({ ok: false, reason: "malformed" });
  });

  it("refuses an identifier long enough to be a payload", () => {
    expect(frame({ type: "upvote", questionId: "a".repeat(LIMITS.identifier + 1) })).toEqual({
      ok: false,
      reason: "malformed",
    });
  });
});

describe("a reaction", () => {
  it("accepts every kind in the vocabulary", () => {
    for (const kind of REACTION_KINDS) {
      expect(frame({ type: "reaction", kind })).toEqual({
        ok: true,
        value: { type: "reaction", kind },
      });
    }
  });

  it("refuses anything outside it", () => {
    // The closed vocabulary is why reactions need no moderation queue. An
    // arbitrary emoji is free text wearing a costume.
    expect(frame({ type: "reaction", kind: "🖕" })).toEqual({
      ok: false,
      reason: "unknown-reaction",
    });
  });
});

describe("a room slug", () => {
  it("accepts what the deck's slugifier emits", () => {
    expect(isRoomSlug("why-rust")).toBe(true);
    expect(isRoomSlug("2026-keynote")).toBe(true);
  });

  it("accepts a non-Latin slug, because a deck in Japanese deserves one", () => {
    expect(isRoomSlug("なぜrust")).toBe(true);
  });

  it("refuses anything that could be a path", () => {
    expect(isRoomSlug("../secrets")).toBe(false);
    expect(isRoomSlug("a/b")).toBe(false);
    expect(isRoomSlug("a.b")).toBe(false);
  });

  it("refuses stray or doubled hyphens", () => {
    expect(isRoomSlug("-talk")).toBe(false);
    expect(isRoomSlug("talk-")).toBe(false);
    expect(isRoomSlug("a--b")).toBe(false);
  });

  it("refuses a case variant, which would otherwise be a second room", () => {
    // Durable Object names are case-sensitive. Accepting both spellings puts
    // half the audience in an empty room.
    expect(isRoomSlug("Talk")).toBe(false);
  });

  it("refuses an empty or over-long slug", () => {
    expect(isRoomSlug("")).toBe(false);
    expect(isRoomSlug("a".repeat(LIMITS.roomSlug + 1))).toBe(false);
  });

  it("refuses anything that is not a string", () => {
    expect(isRoomSlug(7)).toBe(false);
    expect(isRoomSlug(null)).toBe(false);
  });
});

describe("frames from the room", () => {
  const snapshot: RoomSnapshot = {
    room: "a-talk",
    moderation: "held",
    present: 3,
    questions: [],
    reactions: { clap: 0, agree: 0, confused: 0, love: 0 },
    expiresAt: 1,
  };

  it("accepts a state snapshot", () => {
    expect(parseServerFrame(JSON.stringify({ type: "state", state: snapshot }))).toEqual({
      type: "state",
      state: snapshot,
    });
  });

  it("refuses a state that is not a snapshot", () => {
    // The endpoint comes from frontmatter, so a typo can point a deck at
    // something that is not a slidx room at all.
    expect(parseServerFrame(JSON.stringify({ type: "state", state: { room: 1 } }))).toBeNull();
  });

  it("refuses a frame that is not JSON", () => {
    expect(parseServerFrame("<html>")).toBeNull();
  });

  it("accepts the two ways a room can end and no others", () => {
    expect(parseServerFrame(JSON.stringify({ type: "closed", reason: "expired" }))).toEqual({
      type: "closed",
      reason: "expired",
    });
    expect(parseServerFrame(JSON.stringify({ type: "closed", reason: "bored" }))).toBeNull();
  });

  it("refuses a message type it does not know", () => {
    expect(parseServerFrame(JSON.stringify({ type: "redirect", to: "elsewhere" }))).toBeNull();
  });
});
