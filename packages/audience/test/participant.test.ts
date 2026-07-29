/**
 * What one connection is allowed to do.
 *
 * This is the specification for the cheap defence — the one that stops a bored
 * person with one browser tab, not the one that stops a determined attacker.
 * (That is the moderation queue, specified in room.test.ts.)
 *
 * The failure modes it guards:
 *
 * - A bucket that refills fractionally, which a caller can drain by polling in
 *   a tight loop and harvesting the rounding.
 * - A bucket that keeps accruing while idle and then permits an unbounded
 *   burst.
 * - One allowance shared across actions, so somebody who asked their questions
 *   can no longer applaud.
 *
 * The clock is injected, so none of this is measured by sleeping.
 */

import { describe, expect, it } from "vitest";

import { createParticipant, createTokenBucket } from "../src/participant";
import { clock } from "./support";

describe("a token bucket", () => {
  it("allows a burst up to its capacity", () => {
    const time = clock();
    const bucket = createTokenBucket({ capacity: 3, refillMs: 1_000, now: time.now });

    expect([bucket.take(), bucket.take(), bucket.take()]).toEqual([true, true, true]);
  });

  it("refuses once the burst is spent", () => {
    const time = clock();
    const bucket = createTokenBucket({ capacity: 2, refillMs: 1_000, now: time.now });
    bucket.take();
    bucket.take();

    expect(bucket.take()).toBe(false);
  });

  it("refills one token per interval", () => {
    const time = clock();
    const bucket = createTokenBucket({ capacity: 2, refillMs: 1_000, now: time.now });
    bucket.take();
    bucket.take();

    time.advance(1_000);
    expect(bucket.take()).toBe(true);
    expect(bucket.take()).toBe(false);
  });

  it("does not hand out a fraction of a token", () => {
    // Otherwise a caller polling in a tight loop harvests the rounding and the
    // limit stops being a limit.
    const time = clock();
    const bucket = createTokenBucket({ capacity: 1, refillMs: 1_000, now: time.now });
    bucket.take();

    time.advance(999);
    expect(bucket.take()).toBe(false);

    time.advance(1);
    expect(bucket.take()).toBe(true);
  });

  it("keeps the remainder on the clock rather than dropping it", () => {
    const time = clock();
    const bucket = createTokenBucket({ capacity: 2, refillMs: 1_000, now: time.now });
    bucket.take();
    bucket.take();

    // Two 600ms steps are one whole token, and a bucket that reset its clock
    // on every check would never notice.
    time.advance(600);
    bucket.take();
    time.advance(600);

    expect(bucket.take()).toBe(true);
  });

  it("never accrues past its capacity while idle", () => {
    const time = clock();
    const bucket = createTokenBucket({ capacity: 2, refillMs: 1_000, now: time.now });

    time.advance(60 * 60_000);

    expect(bucket.available()).toBe(2);
  });
});

describe("a participant", () => {
  it("has a separate allowance for each action", () => {
    // Somebody who used up their questions can still applaud.
    const time = clock();
    const participant = createParticipant({ now: time.now });

    while (participant.allow("question")) {
      // Drain it.
    }

    expect(participant.allow("reaction")).toBe(true);
  });

  it("remembers what it has voted for", () => {
    const participant = createParticipant();
    expect(participant.hasVoted("000001")).toBe(false);

    participant.recordVote("000001");
    expect(participant.hasVoted("000001")).toBe(true);
    expect(participant.hasVoted("000002")).toBe(false);
  });

  it("remembers nothing about anybody else", () => {
    // The memory lives on the socket rather than on the question, so nothing
    // that identifies a participant is stored with the room.
    const first = createParticipant();
    const second = createParticipant();
    first.recordVote("000001");

    expect(second.hasVoted("000001")).toBe(false);
  });
});
