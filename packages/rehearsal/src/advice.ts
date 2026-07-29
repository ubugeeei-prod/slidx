/**
 * The part of the report a speaker can act on.
 *
 * "You are six minutes over" is the fact a stopwatch already gave them, and it
 * is not a plan. An overrun is almost never spread evenly: three slides ate
 * five of the six minutes and seventeen slides were fine. Naming those three is
 * the difference between a report that gets a talk cut and a report that gets
 * closed.
 *
 * So this module answers one question — *which slides is the overrun actually
 * on* — and then says it in a sentence. Without it the package would produce a
 * table of twenty numbers and leave the speaker to do the arithmetic that made
 * the table worth building.
 *
 * It also refuses to name slides when there is nothing to name. An overrun
 * genuinely spread across a whole deck is a deck that is too long, and pointing
 * at its three worst slides would send a speaker to trim thirty seconds off
 * each while the real answer is to drop a section. Advice that is confidently
 * aimed at the wrong thing is worse than a table.
 */

import { formatDelta, formatList, formatSpan } from "./duration";
import type { RehearsalTotals, SlideReport } from "./report";

export interface DominantSlide {
  id: string;
  /** 1-based deck position, which is how the advice refers to it. */
  index: number;
  /** How far past its budget this slide ran. */
  deltaMs: number;
  /** This slide's share of everything that ran long, 0–1. */
  share: number;
}

/**
 * How much of the overrun the named slides must cover.
 *
 * Three quarters, because the point of naming them is that fixing them fixes
 * the talk. A set of slides covering half an overrun leaves the speaker still
 * over after doing all the work, which teaches them to ignore the advice.
 */
const DOMINANT_SHARE = 0.75;

/**
 * The most slides worth naming.
 *
 * Three is about what a speaker holds in their head walking away from a
 * rehearsal. Past that the advice is a list, and the per-slide table is already
 * a better list than a sentence can be.
 */
const MAX_DOMINANT = 3;

/**
 * Below this, no slide is to blame.
 *
 * If the three worst slides do not even cover half the overrun, the deck is
 * long rather than three of its slides being long, and the honest report says
 * so. Naming three slides anyway would be true and misleading at once.
 */
const SPREAD_SHARE = 0.5;

/**
 * The fewest slides that account for most of the overrun, worst first.
 *
 * Only slides whose verdict is already `over` are candidates: a slide inside
 * tolerance is inside tolerance, and letting a stack of six-second drifts
 * accumulate into a named culprit would blame a slide for the noise floor.
 */
export function dominantSlides(slides: readonly SlideReport[]): DominantSlide[] {
  const long = slides
    .filter((slide) => slide.verdict === "over" && slide.deltaMs !== undefined)
    .map((slide) => ({ id: slide.id, index: slide.index, deltaMs: slide.deltaMs ?? 0 }))
    // Ties break on deck order, so the same rehearsal always names the same
    // slides in the same order — a report that reshuffled could not be diffed
    // against last week's.
    .sort((left, right) => right.deltaMs - left.deltaMs || left.index - right.index);

  const longMs = long.reduce((total, slide) => total + slide.deltaMs, 0);
  if (longMs === 0) return [];

  const picked: typeof long = [];
  let covered = 0;

  for (const slide of long) {
    if (picked.length >= MAX_DOMINANT) break;

    picked.push(slide);
    covered += slide.deltaMs;

    if (covered >= DOMINANT_SHARE * longMs) break;
  }

  if (covered < SPREAD_SHARE * longMs) return [];

  return picked.map((slide) => ({ ...slide, share: slide.deltaMs / longMs }));
}

/**
 * The whole report as one sentence, or two when the rehearsal did not finish.
 *
 * Assembled here rather than in the presenter view so that a CLI, a summary
 * page and the stage all say the same thing — a speaker who reads two different
 * verdicts on the same rehearsal trusts neither.
 */
export function adviseOn(
  totals: RehearsalTotals,
  dominant: readonly DominantSlide[],
  slides: readonly SlideReport[],
): string {
  const parts = [verdictSentence(totals, dominant, slides)];

  // A partial rehearsal is labelled even when its numbers look fine, because
  // "on budget" for a third of a talk is the most misleading thing this package
  // could say.
  if (totals.basis === "reached" && totals.slidesVisited < totals.slidesTotal) {
    parts.push(
      `Only ${totals.slidesVisited} of ${totals.slidesTotal} slides were reached, so this covers part of the talk.`,
    );
  }

  return parts.join(" ");
}

function verdictSentence(
  totals: RehearsalTotals,
  dominant: readonly DominantSlide[],
  slides: readonly SlideReport[],
): string {
  if (slides.length === 0) return "Nothing was recorded.";
  if (totals.slidesVisited === 0) return "No slide was reached, so there is nothing to compare.";
  if (totals.verdict === "unbudgeted") return unbudgetedSentence(dominant, slides);

  const against = `${formatSpan(totals.actualMs)} against a ${formatSpan(totals.budgetMs ?? 0)} budget`;

  if (totals.verdict === "over") {
    if (dominant.length === 0) {
      return `${capitalise(formatDelta(totals.deltaMs ?? 0))}: ${against}, spread across the deck rather than sitting on a few slides.`;
    }

    // One culprit gets its own numbers rather than a share. "Slide 7 took 4m
    // against a 1m budget" is the whole finding, and dividing it into a
    // percentage of an overrun only adds arithmetic.
    if (dominant.length === 1) {
      return `${slideName(dominant[0], slides)} — that is where the ${formatSpan(totals.deltaMs ?? 0)} went.`;
    }

    return `${formatSpan(dominantMs(dominant))} of the ${formatSpan(totals.deltaMs ?? 0)} you are over is on slides ${slideNumbers(dominant)}.`;
  }

  // Under or on budget overall, which does not mean every slide was: a slide
  // that ran three minutes long inside a talk that finished early is still the
  // slide that will sink the talk on a day the room asks questions.
  const overall = `The talk ran ${against} — ${formatDelta(totals.deltaMs ?? 0)}.`;

  if (dominant.length === 0) return overall;

  return dominant.length === 1
    ? `${overall} ${slideName(dominant[0], slides)}.`
    : `${overall} Slides ${slideNumbers(dominant)} still ran ${formatSpan(dominantMs(dominant))} over between them.`;
}

/**
 * What to say when there is no total to compare against.
 *
 * The per-slide numbers can still be worth reading — a deck that budgets three
 * of its slides has three slides worth diffing — so the absence of a total is
 * reported as the missing input it is rather than as a failure.
 */
function unbudgetedSentence(
  dominant: readonly DominantSlide[],
  slides: readonly SlideReport[],
): string {
  if (dominant.length === 0) {
    return "No total to compare against: add `budget:` to every slide to see how the talk fits its slot.";
  }

  const named =
    dominant.length === 1
      ? slideName(dominant[0], slides)
      : `Slides ${slideNumbers(dominant)} ran ${formatSpan(dominantMs(dominant))} over between them`;

  return `No total to compare against, but ${lowerFirst(named)}.`;
}

/** One slide, with the two numbers that make the case against it. */
function slideName(slide: DominantSlide | undefined, slides: readonly SlideReport[]): string {
  if (!slide) return "";

  const detail = slides.find((entry) => entry.index === slide.index);
  const budgetMs = detail?.budgetMs ?? 0;

  return `Slide ${slide.index} took ${formatSpan(detail?.actualMs ?? 0)} against a ${formatSpan(budgetMs)} budget`;
}

/** The sum of what the named slides ran over, which is what cutting them buys back. */
function dominantMs(dominant: readonly DominantSlide[]): number {
  return dominant.reduce((total, slide) => total + slide.deltaMs, 0);
}

function slideNumbers(dominant: readonly DominantSlide[]): string {
  return formatList(dominant.map((slide) => String(slide.index)));
}

function capitalise(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function lowerFirst(text: string): string {
  return text.charAt(0).toLowerCase() + text.slice(1);
}
