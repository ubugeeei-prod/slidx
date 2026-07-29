/**
 * When to try again.
 *
 * A conference network drops every socket in the room at the same instant, and
 * a room is two hundred laptops. Retrying on a fixed interval turns one blip
 * into a synchronised stampede that keeps the room down; retrying without a
 * ceiling means a channel that came back after ten minutes is not noticed for
 * an hour.
 *
 * The jitter is *equal* rather than full: half the ceiling plus a random half.
 * Full jitter can return a delay of nearly zero, which reintroduces the tight
 * retry loop the delay exists to prevent, and the spread from half a window is
 * already enough to break the stampede.
 */

export interface Backoff {
  /** The delay before the next attempt, in milliseconds. */
  next(): number;
  /** Called after a connection succeeds, so the next outage starts fresh. */
  reset(): void;
  attempts(): number;
}

export interface BackoffOptions {
  baseMs?: number;
  maxMs?: number;
  /** Injected so a test can assert an exact delay rather than a range. */
  random?: () => number;
}

/** Long enough not to hammer, short enough that a blink is invisible. */
const BASE_MS = 500;

/**
 * The ceiling.
 *
 * Half a minute is about as long as a speaker will wait before deciding the
 * Q&A is broken and carrying on without it. A larger ceiling saves the Worker
 * nothing that matters and costs the room the reconnection it was waiting for.
 */
const MAX_MS = 30_000;

export function createBackoff(options: BackoffOptions = {}): Backoff {
  const baseMs = options.baseMs ?? BASE_MS;
  const maxMs = options.maxMs ?? MAX_MS;
  const random = options.random ?? Math.random;

  let attempt = 0;

  return {
    next() {
      const ceiling = Math.min(maxMs, baseMs * 2 ** attempt);
      attempt += 1;

      const half = ceiling / 2;
      return Math.round(half + random() * half);
    },

    reset() {
      attempt = 0;
    },

    attempts: () => attempt,
  };
}
