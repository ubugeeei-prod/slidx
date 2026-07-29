/**
 * What crosses the audience channel.
 *
 * Both halves of this package speak it, and both must agree exactly. The client
 * applies these caps so a participant is told about a too-long question while
 * they are still typing it; the room applies them because a client-side cap is
 * a suggestion — anything arriving over a socket was written by whoever wanted
 * to write it, with whatever tool they liked. One definition, enforced twice.
 *
 * Without this module the room would be trusting its input, which for an
 * anonymous public channel projected on a wall behind a speaker is the entire
 * risk.
 */

/** Bumped when a change would make an old client and a new room disagree. */
export const PROTOCOL_VERSION = 1;

/**
 * How a room treats a submitted question.
 *
 * `held` is the default everywhere it can be defaulted. Anonymous text on a
 * screen behind a speaker who is facing the other way is a well-known abuse
 * vector, and the speaker is the last person in the room able to notice.
 */
export type ModerationMode = "held" | "open";

/**
 * The reaction vocabulary, closed on purpose.
 *
 * A free-text or arbitrary-emoji reaction is a second text channel wearing a
 * costume, and it would need the same moderation queue questions do. A fixed
 * set is the one shape that needs no moderation at all, which is why reactions
 * can be live when questions cannot.
 */
export const REACTION_KINDS = ["clap", "agree", "confused", "love"] as const;

export type ReactionKind = (typeof REACTION_KINDS)[number];

export type ReactionTally = Readonly<Record<ReactionKind, number>>;

/**
 * Every cap, in one place, so the two halves cannot drift apart.
 *
 * Lengths are counted in code points rather than UTF-16 units: a cap measured
 * in units gives a Japanese or emoji-heavy question half the room an English
 * one gets, for no reason a participant could ever guess.
 */
export const LIMITS = {
  /**
   * Raw bytes of a single socket frame, checked before parsing.
   *
   * The parse is the expensive step, so the cheap check has to come first —
   * otherwise a ten-megabyte frame becomes a ten-megabyte parse before anyone
   * decides it was too big.
   */
  frameBytes: 2048,
  /** A question. About two sentences: long enough to ask, short enough to read aloud. */
  questionText: 280,
  /** An optional, client-supplied display name. Never verified, never required. */
  displayName: 32,
  /** Server-minted identifiers, bounded so an upvote cannot carry a payload. */
  identifier: 64,
  /** A room slug, which is deck frontmatter and therefore also untrusted input. */
  roomSlug: 64,
  /** Total questions a room will hold, published and pending together. */
  questionsPerRoom: 200,
  /** A host key shorter than this is not a secret, whatever it is called. */
  hostKeyMinimum: 16,
  hostKeyMaximum: 128,
} as const;

/** Why something was refused. Sent back to the one connection that sent it. */
export type RejectionReason =
  | "malformed"
  | "too-large"
  | "too-long"
  | "empty"
  | "unknown-reaction"
  | "unknown-question"
  | "duplicate-vote"
  | "rate-limited"
  | "room-full"
  | "room-closed";

/** Why a room stopped. Both outcomes discard the room's state. */
export type RoomEndReason = "expired" | "ended-by-speaker";

export type Validation<T> = { ok: true; value: T } | { ok: false; reason: RejectionReason };

const accept = <T>(value: T): Validation<T> => ({ ok: true, value });
const reject = <T>(reason: RejectionReason): Validation<T> => ({ ok: false, reason });

/** A question the speaker has let through, as everyone in the room sees it. */
export interface PublishedQuestion {
  id: string;
  text: string;
  /** Absent when the asker did not give one, which is the expected case. */
  name?: string;
  votes: number;
  /** When it was asked, so a queue can be read in order. */
  at: number;
}

/**
 * Everything a connection needs to render the room.
 *
 * The room sends whole snapshots rather than deltas. A Q&A room is tens of
 * items changing at human typing speed, so the bandwidth argument for deltas
 * does not apply, and snapshots remove the entire class of bug where the wall
 * behind the speaker and the speaker's own screen quietly disagree.
 */
export interface RoomSnapshot {
  room: string;
  moderation: ModerationMode;
  /**
   * How many connections are in the room.
   *
   * A count, never a list. A roster of who is present is exactly the personal
   * data this channel exists to avoid collecting, and the number is all a
   * speaker ever wanted from it.
   */
  present: number;
  questions: readonly PublishedQuestion[];
  reactions: ReactionTally;
  /** When the room stops accepting and its state is discarded. */
  expiresAt: number;
  /** Present only on a speaker's snapshot: the queue awaiting approval. */
  pending?: readonly PublishedQuestion[];
}

export type ClientMessage =
  | { type: "question"; text: string; name?: string }
  | { type: "upvote"; questionId: string }
  | { type: "reaction"; kind: ReactionKind };

export type ClientMessageType = ClientMessage["type"];

export type ServerMessage =
  | { type: "state"; state: RoomSnapshot }
  /** `held` tells the asker their question is queued, so they do not ask twice. */
  | { type: "accepted"; held: boolean }
  | { type: "rejected"; reason: RejectionReason }
  | { type: "closed"; reason: RoomEndReason };

const encoder = new TextEncoder();

/**
 * Characters that must not reach a projector.
 *
 * C0 and C1 controls truncate or corrupt a line in most renderers, and the
 * bidi overrides reverse everything after them — a question can be written so
 * that what appears on the wall is not what the moderator approved. Zero-width
 * space and the byte-order mark are here because they are invisible padding
 * used to slip past a length cap.
 *
 * ZWJ and ZWNJ are deliberately absent: they are load-bearing in emoji
 * sequences and in Persian and Indic text, and stripping them would corrupt
 * ordinary writing to defend against nothing.
 */
const UNSAFE_CHARACTERS = /[\p{Cc}\u200B\u200E\u200F\u202A-\u202E\u2066-\u2069\uFEFF]/gu;

/** Server-minted ids only, so an upvote cannot smuggle anything through. */
const IDENTIFIER = /^[A-Za-z0-9_-]+$/;

/**
 * A room slug, matching what the Rust slugifier emits.
 *
 * Non-ASCII letters are kept, because a deck written in Japanese deserves a
 * Japanese room name. What is excluded is what makes a slug dangerous when it
 * becomes a path segment and a Durable Object name: dots, slashes, whitespace,
 * and leading or trailing hyphens.
 */
const ROOM_SLUG = /^[\p{L}\p{N}]+(?:-[\p{L}\p{N}]+)*$/u;

/** Code points, not UTF-16 units. See LIMITS. */
export function textLength(text: string): number {
  return Array.from(text).length;
}

/**
 * The form a string takes before anything is done with it.
 *
 * Normalising first means the cap applies to what will actually be displayed:
 * without it, a question padded with two hundred newlines counts as two
 * hundred characters of nothing and pushes the rest of the queue off screen.
 */
export function sanitizeText(raw: string): string {
  return raw.normalize("NFC").replace(UNSAFE_CHARACTERS, " ").replace(/\s+/gu, " ").trim();
}

/** Sanitises, then enforces a cap. Over-long input is refused, never truncated. */
export function checkText(raw: unknown, cap: number): Validation<string> {
  if (typeof raw !== "string") return reject("malformed");

  const text = sanitizeText(raw);
  if (text.length === 0) return reject("empty");
  if (textLength(text) > cap) return reject("too-long");

  return accept(text);
}

/**
 * An optional display name.
 *
 * Optional in both directions: absent is valid, and a name that sanitises away
 * to nothing is treated as absent rather than as an error. Nobody should be
 * stopped from asking a question by the field they did not want to fill in.
 */
export function checkName(raw: unknown): Validation<string | undefined> {
  if (raw === undefined || raw === null) return accept(undefined);
  if (typeof raw !== "string") return reject("malformed");

  const name = sanitizeText(raw);
  if (name.length === 0) return accept(undefined);
  if (textLength(name) > LIMITS.displayName) return reject("too-long");

  return accept(name);
}

export function checkIdentifier(raw: unknown): Validation<string> {
  if (typeof raw !== "string") return reject("malformed");
  if (raw.length === 0 || raw.length > LIMITS.identifier) return reject("malformed");
  if (!IDENTIFIER.test(raw)) return reject("malformed");

  return accept(raw);
}

/** True for a slug safe to use as a path segment and a room name. */
export function isRoomSlug(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= LIMITS.roomSlug &&
    // Case-folded, because the slug becomes a Durable Object name and those are
    // case-sensitive: without this, `Talk` and `talk` are two rooms and half the
    // audience is in the empty one.
    value === value.toLowerCase() &&
    ROOM_SLUG.test(value)
  );
}

export function isReactionKind(value: unknown): value is ReactionKind {
  return typeof value === "string" && (REACTION_KINDS as readonly string[]).includes(value);
}

/** Objects only. An array or a null would otherwise pass a `typeof` check. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * A parsed message, checked field by field.
 *
 * Unknown message types are refused rather than ignored, so a client talking to
 * a room that does not understand it hears about it instead of silently
 * dropping every question the speaker never sees.
 */
export function validateClientMessage(value: unknown): Validation<ClientMessage> {
  if (!isRecord(value)) return reject("malformed");

  switch (value["type"]) {
    case "question": {
      const text = checkText(value["text"], LIMITS.questionText);
      if (!text.ok) return text;

      const name = checkName(value["name"]);
      if (!name.ok) return name;

      return accept<ClientMessage>({
        type: "question",
        text: text.value,
        ...(name.value === undefined ? {} : { name: name.value }),
      });
    }

    case "upvote": {
      const questionId = checkIdentifier(value["questionId"]);
      if (!questionId.ok) return questionId;

      return accept<ClientMessage>({ type: "upvote", questionId: questionId.value });
    }

    case "reaction": {
      if (!isReactionKind(value["kind"])) return reject("unknown-reaction");

      return accept<ClientMessage>({ type: "reaction", kind: value["kind"] });
    }

    default:
      return reject("malformed");
  }
}

/** The whole inbound path: size, then parse, then shape. In that order. */
export function validateFrame(raw: string): Validation<ClientMessage> {
  // UTF-8 is never shorter than UTF-16 unit count, so the cheap comparison
  // rules out oversized frames before anything is allocated to measure them.
  if (raw.length > LIMITS.frameBytes) return reject("too-large");
  if (encoder.encode(raw).length > LIMITS.frameBytes) return reject("too-large");

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return reject("malformed");
  }

  return validateClientMessage(parsed);
}

/**
 * A frame from the room, checked before the deck believes it.
 *
 * The URL of the channel comes from deck frontmatter, so a deck can be pointed
 * at something that is not a slidx room by a typo. The client renders what it
 * is sent, and rendering an arbitrary object as a question queue is how a typo
 * turns into a defect nobody can reproduce.
 */
export function parseServerFrame(raw: string): ServerMessage | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }

  if (!isRecord(parsed)) return null;

  switch (parsed["type"]) {
    case "state":
      return isSnapshot(parsed["state"]) ? { type: "state", state: parsed["state"] } : null;

    case "accepted":
      return typeof parsed["held"] === "boolean"
        ? { type: "accepted", held: parsed["held"] }
        : null;

    case "rejected":
      return typeof parsed["reason"] === "string"
        ? { type: "rejected", reason: parsed["reason"] as RejectionReason }
        : null;

    case "closed":
      return parsed["reason"] === "expired" || parsed["reason"] === "ended-by-speaker"
        ? { type: "closed", reason: parsed["reason"] }
        : null;

    default:
      return null;
  }
}

function isSnapshot(value: unknown): value is RoomSnapshot {
  return (
    isRecord(value) &&
    typeof value["room"] === "string" &&
    (value["moderation"] === "held" || value["moderation"] === "open") &&
    typeof value["present"] === "number" &&
    typeof value["expiresAt"] === "number" &&
    Array.isArray(value["questions"]) &&
    isRecord(value["reactions"])
  );
}

/** A tally with every kind at zero, so a renderer never meets an absent key. */
export function emptyTally(): Record<ReactionKind, number> {
  return { clap: 0, agree: 0, confused: 0, love: 0 };
}
