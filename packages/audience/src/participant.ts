/**
 * What one connection is allowed to do.
 *
 * Everything here is per socket and lives only as long as that socket. That is
 * the design, not a limitation: the alternative — a durable record of what each
 * participant has done — means identifying participants, which means collecting
 * something about them. This channel would rather forget.
 *
 * The cost is that a reconnect resets the allowance. A token bucket was never
 * going to stop somebody determined to open fifty sockets, which is precisely
 * why the real defence against abuse is the moderation queue and not this file.
 * What this does stop is the ordinary case: a bored person with one browser tab
 * holding down a key.
 */

import type { ClientMessageType } from "./protocol";

/**
 * A bucket that refills over time.
 *
 * A bucket rather than a fixed window because the behaviour that matters is
 * bursty. Someone asks two questions in ten seconds because they thought of a
 * second one, and that should be fine; someone asking thirty is not asking.
 */
export interface TokenBucket {
  take(): boolean;
  available(): number;
}

export interface TokenBucketOptions {
  /** How many actions are allowed back to back. */
  capacity: number;
  /** How long one token takes to come back. */
  refillMs: number;
  now: () => number;
}

export function createTokenBucket(options: TokenBucketOptions): TokenBucket {
  const { capacity, refillMs, now } = options;

  let tokens = capacity;
  let last = now();

  const refill = () => {
    const elapsed = now() - last;
    if (elapsed <= 0) return;

    // Whole tokens only, and the remainder stays on the clock. Refilling
    // fractionally would let a caller poll in a tight loop and harvest the
    // rounding, which is the flood this exists to prevent.
    const earned = Math.floor(elapsed / refillMs);
    if (earned <= 0) return;

    tokens = Math.min(capacity, tokens + earned);
    last += earned * refillMs;
  };

  return {
    take() {
      refill();
      if (tokens <= 0) return false;

      tokens -= 1;
      return true;
    },

    available() {
      refill();
      return tokens;
    },
  };
}

/**
 * The allowance for each kind of action.
 *
 * The numbers differ because the actions differ in what they cost the room. A
 * question occupies a moderator and a line on a screen, so it is scarce. An
 * upvote is a vote and there are only so many questions to cast one on. A
 * reaction is applause: cheap, and mostly the point.
 */
const ALLOWANCE: Record<ClientMessageType, { capacity: number; refillMs: number }> = {
  question: { capacity: 3, refillMs: 30_000 },
  upvote: { capacity: 20, refillMs: 3_000 },
  reaction: { capacity: 10, refillMs: 2_000 },
};

export interface Participant {
  /** False once this connection has spent its allowance for that action. */
  allow(action: ClientMessageType): boolean;
  /** True when this connection has already upvoted that question. */
  hasVoted(questionId: string): boolean;
  recordVote(questionId: string): void;
}

export interface ParticipantOptions {
  /** Injected, so rate limiting is tested with arithmetic rather than sleeps. */
  now?: () => number;
}

export function createParticipant(options: ParticipantOptions = {}): Participant {
  const now = options.now ?? (() => Date.now());

  const buckets = new Map<ClientMessageType, TokenBucket>();
  for (const [action, limits] of Object.entries(ALLOWANCE)) {
    buckets.set(action as ClientMessageType, createTokenBucket({ ...limits, now }));
  }

  /**
   * Which questions this socket has already voted for.
   *
   * Held here rather than on the question, because storing it on the question
   * would mean storing something that identifies a participant for as long as
   * the room lives. A reconnect can vote again; that is a known and accepted
   * inaccuracy in a show of hands, which is all an upvote count is.
   */
  const voted = new Set<string>();

  return {
    allow: (action) => buckets.get(action)?.take() ?? false,
    hasVoted: (questionId) => voted.has(questionId),
    recordVote: (questionId) => {
      voted.add(questionId);
    },
  };
}
