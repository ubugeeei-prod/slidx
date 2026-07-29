/**
 * Recordings built by hand.
 *
 * A report is a pure function of a recording, so the tests for it are clearest
 * when the recording is stated outright rather than acted out through a
 * recorder and a fake clock. `rehearsal.test.ts` is where the recorder's own
 * behaviour is specified; everything downstream starts from a literal.
 */

import {
  TOLERANCE_MS,
  TOTAL_TOLERANCE_MS,
  type RecordedSlide,
  type RehearsalRecording,
  type RehearsalStatus,
} from "../src/rehearsal";

/** A slide's totals, with the tedious parts defaulted. */
export function slide(
  id: string,
  actualMs: number,
  budgetMs?: number,
  visits = actualMs > 0 ? 1 : 0,
): RecordedSlide {
  return { id, actualMs, visits, ...(budgetMs === undefined ? {} : { budgetMs }) };
}

/** A slide the speaker never opened. */
export function skipped(id: string, budgetMs?: number): RecordedSlide {
  return slide(id, 0, budgetMs, 0);
}

export function recording(
  slides: RecordedSlide[],
  status: RehearsalStatus = "finished",
): RehearsalRecording {
  return {
    version: 1,
    status,
    toleranceMs: TOLERANCE_MS,
    totalToleranceMs: TOTAL_TOLERANCE_MS,
    slides,
  };
}

/** Minutes, because every budget in a real deck is written in them. */
export function minutes(count: number): number {
  return count * 60_000;
}
