/**
 * When to try again.
 *
 * This is the specification for the schedule a dropped connection retries on.
 * A conference network drops every socket in the room at once, so the failure
 * modes it guards are about what two hundred laptops do next:
 *
 * - A fixed interval, which turns one blip into a synchronised stampede.
 * - Jitter that can reach zero, which is the tight retry loop the delay exists
 *   to prevent.
 * - No ceiling, so a channel that came back after ten minutes is not noticed
 *   for an hour.
 * - A schedule that stays escalated after a successful reconnect, so the next
 *   blip starts at half a minute.
 *
 * Randomness is injected, so a delay is asserted exactly rather than as a
 * range.
 */

import { describe, expect, it } from "vitest";

import { createBackoff } from "../src/backoff";

/** The midpoint of the jitter window, which makes each delay its own ceiling. */
const midpoint = () => 1;

describe("the retry schedule", () => {
  it("doubles the window each time", () => {
    const backoff = createBackoff({ baseMs: 100, maxMs: 10_000, random: midpoint });

    expect([backoff.next(), backoff.next(), backoff.next()]).toEqual([100, 200, 400]);
  });

  it("stops doubling at the ceiling", () => {
    const backoff = createBackoff({ baseMs: 100, maxMs: 400, random: midpoint });
    backoff.next();
    backoff.next();
    backoff.next();

    expect(backoff.next()).toBe(400);
    expect(backoff.next()).toBe(400);
  });

  it("never returns less than half the window", () => {
    // Full jitter can return nearly zero, which reintroduces the tight loop.
    const backoff = createBackoff({ baseMs: 1_000, maxMs: 10_000, random: () => 0 });

    expect(backoff.next()).toBe(500);
  });

  it("spreads the retries across the window", () => {
    // Two clients with different randomness must not come back at the same
    // instant, which is the whole point of jittering at all.
    const early = createBackoff({ baseMs: 1_000, maxMs: 10_000, random: () => 0.1 });
    const late = createBackoff({ baseMs: 1_000, maxMs: 10_000, random: () => 0.9 });

    expect(early.next()).toBe(550);
    expect(late.next()).toBe(950);
  });

  it("returns to the start after a connection succeeds", () => {
    const backoff = createBackoff({ baseMs: 100, maxMs: 10_000, random: midpoint });
    backoff.next();
    backoff.next();
    backoff.reset();

    expect(backoff.next()).toBe(100);
    expect(backoff.attempts()).toBe(1);
  });

  it("counts the attempts, so a caller can say how long it has been trying", () => {
    const backoff = createBackoff({ random: midpoint });
    expect(backoff.attempts()).toBe(0);

    backoff.next();
    backoff.next();
    expect(backoff.attempts()).toBe(2);
  });
});
