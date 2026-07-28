/**
 * The presenter's clock.
 *
 * Elapsed time is *derived* from the clock rather than accumulated from ticks:
 * `started + (now - resumedAt)`. That is the difference between a timer that
 * survives a laptop sleeping between rooms and one that quietly loses the
 * minutes it was asleep for — accumulating on an interval means every missed
 * frame is a missed millisecond.
 *
 * It also means the display can be redrawn at any rate, or not at all, without
 * affecting what the timer knows.
 */

/** How the talk is going, as a token a theme can style. */
export type TimerStatus = "untimed" | "on-time" | "nearly-done" | "over";

export interface TimerState {
  running: boolean;
  elapsedMs: number;
  /** Time left against the budget. Negative once over. Absent when untimed. */
  remainingMs?: number;
  /** True once the budget is spent. */
  overrun: boolean;
  status: TimerStatus;
}

export interface Timer {
  start(): void;
  pause(): void;
  toggle(): void;
  reset(): void;
  state(): TimerState;
}

export interface TimerOptions {
  /** Injected so tests need no sleeps, and so a mirror can share a clock. */
  now?: () => number;
  /** The slot length. Without one, the timer counts but does not judge. */
  budgetMs?: number | undefined;
}

/**
 * The warning arrives with this much left.
 *
 * Three minutes is about one slide plus a question. A warning that lands *as*
 * time expires is a warning that arrives too late to change anything, which is
 * the only thing a talk timer is for.
 */
const WARNING_MS = 3 * 60_000;

export function createTimer(options: TimerOptions = {}): Timer {
  const now = options.now ?? (() => Date.now());
  const budgetMs = options.budgetMs;

  /** Time banked from previous runs. */
  let banked = 0;
  /** When the current run started, or null while paused. */
  let resumedAt: number | null = null;

  const elapsed = () => (resumedAt === null ? banked : banked + (now() - resumedAt));

  return {
    start() {
      // Starting an already-running timer must not restart it: the shortcut is
      // easy to press twice and losing the talk's start is unrecoverable.
      if (resumedAt !== null) return;
      resumedAt = now();
    },

    pause() {
      if (resumedAt === null) return;
      banked += now() - resumedAt;
      resumedAt = null;
    },

    toggle() {
      if (resumedAt === null) {
        resumedAt = now();
        return;
      }
      banked += now() - resumedAt;
      resumedAt = null;
    },

    reset() {
      banked = 0;
      resumedAt = null;
    },

    state() {
      const elapsedMs = elapsed();

      if (budgetMs === undefined) {
        return { running: resumedAt !== null, elapsedMs, overrun: false, status: "untimed" };
      }

      const remainingMs = budgetMs - elapsedMs;

      return {
        running: resumedAt !== null,
        elapsedMs,
        remainingMs,
        overrun: remainingMs < 0,
        status: remainingMs < 0 ? "over" : remainingMs <= WARNING_MS ? "nearly-done" : "on-time",
      };
    },
  };
}

/**
 * A duration, as a presenter reads it at a glance.
 *
 * Hours appear only when there are hours: a workshop clock reading `90:00` is
 * harder to parse in a glance than `1:30:00`. An overrun is signed, because
 * `2:00` on its own is ambiguous and `-2:00` is not.
 */
export function formatDuration(ms: number): string {
  const sign = ms < 0 ? "-" : "";

  // Towards zero, so the display never shows a second that has not passed.
  const total = Math.floor(Math.abs(ms) / 1000);
  const seconds = total % 60;
  const minutes = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600);

  const pad = (value: number) => String(value).padStart(2, "0");

  return hours > 0
    ? `${sign}${hours}:${pad(minutes)}:${pad(seconds)}`
    : `${sign}${minutes}:${pad(seconds)}`;
}
