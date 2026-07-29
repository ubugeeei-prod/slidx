/**
 * One room's state, and every rule about changing it.
 *
 * This is the half that cannot be bypassed, so it is the half that decides. It
 * re-checks the caps the protocol module already applied, because the only
 * check that counts is the last one before the write. It holds questions back
 * for the speaker unless the speaker said otherwise at the moment the room was
 * opened. It forgets everything when the room ends.
 *
 * Storage and the clock are injected. A Durable Object is not something you can
 * construct in a test, and a room whose rules can only be exercised by
 * deploying it is a room whose rules are not really tested. The Cloudflare
 * runtime supplies both of these in production; a Map and a counter supply them
 * in the suite, and the code cannot tell the difference.
 */

import {
  checkName,
  checkText,
  emptyTally,
  LIMITS,
  type ClientMessage,
  type ModerationMode,
  type ReactionKind,
  type RejectionReason,
  type RoomEndReason,
  type RoomSnapshot,
} from "./protocol";
import type { Participant } from "./participant";
import {
  pendingQuestions,
  publishedQuestions,
  QUESTION_PREFIX,
  questionId,
  questionKey,
  type StoredQuestion,
} from "./questions";

const HOUR_MS = 60 * 60_000;

/**
 * How long a room lives.
 *
 * A talk, plus the hallway conversation after it. The room is not a forum: at
 * the end its storage is deleted, so a question asked into it is gone by the
 * next morning whether or not anybody remembers to clear it. That is the
 * default because the alternative default — keeping it — is a decision nobody
 * in the room was asked to consent to.
 *
 * The maximum exists because the lifetime is caller-supplied input, and an
 * unbounded one turns an open Worker into free permanent hosting.
 */
export const ROOM_LIFETIME = {
  defaultMs: 3 * HOUR_MS,
  minimumMs: 5 * 60_000,
  maximumMs: 12 * HOUR_MS,
} as const;

/**
 * The slice of Durable Object storage this room uses.
 *
 * Written out rather than imported from the platform types so the room can be
 * driven by a Map in a test. Cloudflare's `DurableObjectStorage` satisfies it
 * as-is; the extra options its methods accept are ones this room never passes.
 */
export interface RoomStorage {
  get<T>(key: string): Promise<T | undefined>;
  put<T>(key: string, value: T): Promise<void>;
  delete(key: string): Promise<boolean>;
  list<T>(options?: { prefix?: string }): Promise<Map<string, T>>;
  deleteAll(): Promise<void>;
}

export interface OpenOptions {
  /** The speaker's bearer token for this room. Not an account; not stored anywhere else. */
  hostKey: string;
  /** Defaults to `held`. There is no path that reaches `open` without saying so. */
  moderation?: ModerationMode;
  lifetimeMs?: number;
}

export type OpenOutcome =
  | { ok: true; snapshot: RoomSnapshot }
  | { ok: false; reason: "weak-host-key" | "taken" };

export type SubmitOutcome =
  | { ok: true; effect: "published" | "held" | "counted" }
  | { ok: false; reason: RejectionReason };

export type HostOutcome = { ok: true } | { ok: false; reason: "forbidden" | "unknown-question" };

export interface Room {
  open(options: OpenOptions): Promise<OpenOutcome>;
  /** What everyone in the room sees. Null when there is no room. */
  snapshot(): Promise<RoomSnapshot | null>;
  /**
   * What the speaker sees, including the queue.
   *
   * Takes the key rather than trusting a caller that says it already checked.
   * The pending queue is the one piece of state that must not leak, so the
   * check belongs at the point of access.
   */
  hostSnapshot(hostKey: string): Promise<RoomSnapshot | null>;
  submit(message: ClientMessage, participant: Participant): Promise<SubmitOutcome>;
  approve(questionId: string, hostKey: string): Promise<HostOutcome>;
  dismiss(questionId: string, hostKey: string): Promise<HostOutcome>;
  end(hostKey: string): Promise<HostOutcome>;
  /** Discards the room if its time is up. Returns why it ended, or null. */
  sweep(): Promise<RoomEndReason | null>;
  /** When the room next needs attention, for the platform's alarm. Null when closed. */
  endsAt(): Promise<number | null>;
}

export interface RoomOptions {
  slug: string;
  storage: RoomStorage;
  /** Injected so expiry is tested by arithmetic rather than by waiting three hours. */
  now?: () => number;
  /** How many connections are attached. The room does not track sockets itself. */
  present?: () => number;
}

interface RoomMeta {
  slug: string;
  hostKey: string;
  moderation: ModerationMode;
  openedAt: number;
  expiresAt: number;
  sequence: number;
}

const META_KEY = "meta";

export function createRoom(options: RoomOptions): Room {
  const { slug, storage } = options;
  const now = options.now ?? (() => Date.now());
  const present = options.present ?? (() => 0);

  /** Mirrors storage once loaded. A Durable Object is single-threaded, so this is safe. */
  let meta: RoomMeta | null = null;
  let questions = new Map<string, StoredQuestion>();
  let loaded = false;

  /**
   * Reactions are counted here and never written down.
   *
   * A reaction is a moment — applause during a demo — not a record, and it has
   * no meaning ten minutes later. Keeping the tally in memory means a room
   * under a burst of applause does no storage writes at all, and it means the
   * count resets if the platform recycles the object, which is a fair
   * description of what applause does anyway.
   */
  let reactions = emptyTally();

  async function load(): Promise<void> {
    if (loaded) return;

    meta = (await storage.get<RoomMeta>(META_KEY)) ?? null;
    questions = await storage.list<StoredQuestion>({ prefix: QUESTION_PREFIX });
    loaded = true;
  }

  async function discard(): Promise<void> {
    await storage.deleteAll();
    meta = null;
    questions = new Map();
    reactions = emptyTally();
    loaded = true;
  }

  /**
   * Ends the room if its time is up, before anything else looks at it.
   *
   * Called at the top of every operation rather than left to the platform's
   * alarm. An alarm that did not fire — a redeploy, a cold object, a bug —
   * would otherwise leave a room answering questions after the day it was
   * supposed to be gone, and expiry is a promise made to the people who asked
   * the questions.
   */
  async function sweep(): Promise<RoomEndReason | null> {
    await load();
    if (!meta || now() < meta.expiresAt) return null;

    await discard();
    return "expired";
  }

  /** Constant-time comparison: a key checked with `===` leaks its prefix by timing. */
  function keyMatches(candidate: string, expected: string): boolean {
    if (candidate.length !== expected.length) return false;

    let difference = 0;
    for (let index = 0; index < candidate.length; index += 1) {
      difference |= candidate.charCodeAt(index) ^ expected.charCodeAt(index);
    }

    return difference === 0;
  }

  function currentSnapshot(current: RoomMeta): RoomSnapshot {
    return {
      room: current.slug,
      moderation: current.moderation,
      present: present(),
      questions: publishedQuestions(questions.values()),
      reactions: { ...reactions },
      expiresAt: current.expiresAt,
    };
  }

  async function write(question: StoredQuestion): Promise<void> {
    questions.set(questionKey(question.id), question);
    await storage.put(questionKey(question.id), question);
  }

  function find(id: string): StoredQuestion | undefined {
    return questions.get(questionKey(id));
  }

  async function ask(
    message: Extract<ClientMessage, { type: "question" }>,
    current: RoomMeta,
  ): Promise<SubmitOutcome> {
    // Re-checked here even though the frame validator already did it. This is
    // the last gate before a write, and it is the only one an attacker cannot
    // choose to skip by not using our client.
    const text = checkText(message.text, LIMITS.questionText);
    if (!text.ok) return { ok: false, reason: text.reason };

    const name = checkName(message.name);
    if (!name.ok) return { ok: false, reason: name.reason };

    if (questions.size >= LIMITS.questionsPerRoom) return { ok: false, reason: "room-full" };

    current.sequence += 1;
    const id = questionId(current.sequence);

    await write({
      id,
      text: text.value,
      ...(name.value === undefined ? {} : { name: name.value }),
      votes: 0,
      at: now(),
      published: current.moderation === "open",
    });
    await storage.put(META_KEY, current);

    return { ok: true, effect: current.moderation === "open" ? "published" : "held" };
  }

  async function upvote(id: string, participant: Participant): Promise<SubmitOutcome> {
    const question = find(id);

    // A pending question is indistinguishable from one that never existed. The
    // alternative answers "that id is in the queue", which turns the upvote
    // endpoint into a way to enumerate what the speaker has not approved.
    if (!question || !question.published) return { ok: false, reason: "unknown-question" };
    if (participant.hasVoted(id)) return { ok: false, reason: "duplicate-vote" };

    participant.recordVote(id);
    question.votes += 1;
    await write(question);

    return { ok: true, effect: "counted" };
  }

  function react(kind: ReactionKind): SubmitOutcome {
    reactions[kind] += 1;
    return { ok: true, effect: "counted" };
  }

  async function authenticated(hostKey: string): Promise<RoomMeta | null> {
    await sweep();
    return meta && keyMatches(hostKey, meta.hostKey) ? meta : null;
  }

  return {
    async open(request) {
      const { hostKey } = request;

      // A short key is not a secret. Refusing it here is the only chance to
      // say so — after this it is the only thing standing between a stranger
      // and the approve button.
      if (
        hostKey.length < LIMITS.hostKeyMinimum ||
        hostKey.length > LIMITS.hostKeyMaximum ||
        hostKey.trim().length !== hostKey.length
      ) {
        return { ok: false, reason: "weak-host-key" };
      }

      await sweep();

      if (meta) {
        // Reopening is how a speaker's reloaded presenter view finds its room,
        // so the same key succeeds. A different key does not: a live room must
        // not be seizable by whoever asks for it second. The lifetime is not
        // extended either — a room that could be renewed by reconnecting would
        // never end.
        if (!keyMatches(hostKey, meta.hostKey)) return { ok: false, reason: "taken" };

        return { ok: true, snapshot: currentSnapshot(meta) };
      }

      const openedAt = now();
      const lifetimeMs = Math.min(
        ROOM_LIFETIME.maximumMs,
        Math.max(ROOM_LIFETIME.minimumMs, request.lifetimeMs ?? ROOM_LIFETIME.defaultMs),
      );

      meta = {
        slug,
        hostKey,
        // The safe mode is what you get by omission. Reaching the other one
        // requires writing it down, which is the point.
        moderation: request.moderation === "open" ? "open" : "held",
        openedAt,
        expiresAt: openedAt + lifetimeMs,
        sequence: 0,
      };

      await storage.put(META_KEY, meta);
      return { ok: true, snapshot: currentSnapshot(meta) };
    },

    async snapshot() {
      await sweep();
      return meta ? currentSnapshot(meta) : null;
    },

    async hostSnapshot(hostKey) {
      const current = await authenticated(hostKey);
      if (!current) return null;

      return { ...currentSnapshot(current), pending: pendingQuestions(questions.values()) };
    },

    async submit(message, participant) {
      await sweep();
      if (!meta) return { ok: false, reason: "room-closed" };

      // The allowance is spent before the message is judged, so a flood of
      // invalid questions costs the sender exactly as much as a flood of valid
      // ones. Charging only for well-formed input makes garbage free.
      if (!participant.allow(message.type)) return { ok: false, reason: "rate-limited" };

      switch (message.type) {
        case "question":
          return ask(message, meta);
        case "upvote":
          return upvote(message.questionId, participant);
        case "reaction":
          return react(message.kind);
      }
    },

    async approve(id, hostKey) {
      if (!(await authenticated(hostKey))) return { ok: false, reason: "forbidden" };

      const question = find(id);
      if (!question) return { ok: false, reason: "unknown-question" };

      question.published = true;
      await write(question);

      return { ok: true };
    },

    async dismiss(id, hostKey) {
      if (!(await authenticated(hostKey))) return { ok: false, reason: "forbidden" };

      const question = find(id);
      if (!question) return { ok: false, reason: "unknown-question" };

      // Deleted, not flagged. A rejected question kept in a hidden column is
      // an archive of what people said anonymously, which is the thing this
      // channel promised not to build.
      questions.delete(questionKey(id));
      await storage.delete(questionKey(id));

      return { ok: true };
    },

    async end(hostKey) {
      if (!(await authenticated(hostKey))) return { ok: false, reason: "forbidden" };

      await discard();
      return { ok: true };
    },

    sweep,

    async endsAt() {
      await load();
      return meta?.expiresAt ?? null;
    },
  };
}
