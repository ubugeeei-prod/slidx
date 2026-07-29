/**
 * The rehearsal, diffed against the plan.
 *
 * A pure function of a recording: same recording, same report, no clock and no
 * storage. That is what lets a report be recomputed after a reload and compared
 * with the one from last week, which is the only way a speaker finds out
 * whether the cuts they made worked.
 *
 * The answer is per slide, and that is the entire point. "You ran six minutes
 * over" is what a stopwatch already said, and it is the one fact that does not
 * tell a speaker what to cut.
 *
 * Two judgements here are worth stating, because getting either wrong produces
 * a number that reads authoritative and is wrong:
 *
 * - **A partial budget total is not reported.** If any slide in the comparison
 *   declares no budget, the total is absent rather than a sum of the slides
 *   that happen to have one. `slidx_core` makes the same call for the same
 *   reason: a partial sum looks like a whole one.
 * - **An unfinished rehearsal is diffed against the slides it reached.** A
 *   speaker who gave up on slide 5 of 20 is not "fifteen minutes under"; that
 *   number describes the fifteen slides they did not give. A finished
 *   rehearsal is diffed against the whole deck, including slides skipped on the
 *   day, because those are still in the talk they intend to give.
 */

import { adviseOn, dominantSlides, type DominantSlide } from "./advice";
import type { RecordedSlide, RehearsalRecording, RehearsalStatus } from "./rehearsal";

/** How one slide went against its budget. */
export type SlideVerdict =
  /** Within tolerance. */
  | "on-budget"
  | "over"
  | "under"
  /** Never visited. Reported before any budget comparison — it was not given. */
  | "skipped"
  /** Visited, but the deck declares nothing to compare it against. */
  | "unbudgeted";

/** How the talk went against its slot. */
export type TotalVerdict = "on-budget" | "over" | "under" | "unbudgeted";

/** Which slides the total budget covers. */
export type BudgetBasis =
  /** Every slide in the deck. Used once the speaker says the talk is over. */
  | "deck"
  /** Only the slides visited, because the rest were never reached. */
  | "reached";

export interface SlideReport {
  id: string;
  /** 1-based deck position — "slide 9" is how a speaker refers to a slide. */
  index: number;
  actualMs: number;
  /** More than one visit means the speaker came back to it. */
  visits: number;
  budgetMs?: number;
  /** `actual − budget`. Positive is over. Absent when the slide declares nothing. */
  deltaMs?: number;
  verdict: SlideVerdict;
}

export interface RehearsalTotals {
  actualMs: number;
  /** Absent when any slide in the basis declares no budget, because a partial sum misleads. */
  budgetMs?: number;
  deltaMs?: number;
  verdict: TotalVerdict;
  basis: BudgetBasis;
  slidesVisited: number;
  slidesTotal: number;
}

export interface RehearsalReport {
  status: RehearsalStatus;
  /** False while recording and after abandoning, so a partial report reads as one. */
  complete: boolean;
  slides: SlideReport[];
  totals: RehearsalTotals;
  /** The few slides that account for most of an overrun, worst first. Empty otherwise. */
  dominant: DominantSlide[];
  /** One sentence naming what to do about it. */
  advice: string;
}

export function buildReport(recording: RehearsalRecording): RehearsalReport {
  const complete = recording.status === "finished";
  const slides = recording.slides.map((slide, position) =>
    reportSlide(slide, position + 1, recording.toleranceMs),
  );

  const totals = totalsFor(slides, complete, recording.totalToleranceMs);
  const dominant = dominantSlides(slides);

  return {
    status: recording.status,
    complete,
    slides,
    totals,
    dominant,
    advice: adviseOn(totals, dominant, slides),
  };
}

function reportSlide(slide: RecordedSlide, index: number, toleranceMs: number): SlideReport {
  const base = {
    id: slide.id,
    index,
    actualMs: slide.actualMs,
    visits: slide.visits,
    ...(slide.budgetMs === undefined ? {} : { budgetMs: slide.budgetMs }),
  };

  if (slide.budgetMs === undefined) {
    return { ...base, verdict: slide.visits === 0 ? "skipped" : "unbudgeted" };
  }

  const deltaMs = slide.actualMs - slide.budgetMs;

  // "Skipped" outranks any budget comparison. A slide that was never opened did
  // not run short; it did not run at all.
  if (slide.visits === 0) return { ...base, deltaMs, verdict: "skipped" };

  return {
    ...base,
    deltaMs,
    verdict: Math.abs(deltaMs) <= toleranceMs ? "on-budget" : deltaMs > 0 ? "over" : "under",
  };
}

function totalsFor(
  slides: readonly SlideReport[],
  complete: boolean,
  totalToleranceMs: number,
): RehearsalTotals {
  const basis: BudgetBasis = complete ? "deck" : "reached";
  const counted = basis === "deck" ? slides : slides.filter((slide) => slide.visits > 0);

  const actualMs = slides.reduce((total, slide) => total + slide.actualMs, 0);
  const budgetMs = counted.every((slide) => slide.budgetMs !== undefined)
    ? counted.reduce((total, slide) => total + (slide.budgetMs ?? 0), 0)
    : undefined;

  const shared = {
    actualMs,
    basis,
    slidesVisited: slides.filter((slide) => slide.visits > 0).length,
    slidesTotal: slides.length,
  };

  // No slides in the basis at all — a rehearsal abandoned before the first
  // visit. `every` over an empty list is true, so this has to be said
  // explicitly or the report claims a budget of zero and a verdict of
  // "on-budget" for a rehearsal nobody gave.
  if (budgetMs === undefined || counted.length === 0) {
    return { ...shared, verdict: "unbudgeted" };
  }

  const deltaMs = actualMs - budgetMs;

  return {
    ...shared,
    budgetMs,
    deltaMs,
    verdict: Math.abs(deltaMs) <= totalToleranceMs ? "on-budget" : deltaMs > 0 ? "over" : "under",
  };
}
