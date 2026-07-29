/**
 * Whether the talk is going to fit.
 *
 * The timer answers "how long have I been talking". That is not the question a
 * speaker has on stage — the question is whether the time left is enough for
 * the slides left, and the clock cannot see the deck.
 *
 * Everything here is shaped by one risk: an indicator a speaker learns to
 * ignore is worse than no indicator, because it took up the one piece of
 * screen they can afford to glance at.
 *
 * **Being partway through a slide is on pace.** The expectation is a window,
 * not an instant — the whole of a slide's budget is time that slide is allowed
 * to take. Comparing against the moment of arrival would report "behind" for
 * most of every slide, which is the same as reporting nothing.
 *
 * **The basis is always reported.** "Three minutes behind" means one thing
 * when it came from the author's own per-slide budgets and another when it was
 * divided out of a slide count, and a reader who cannot tell them apart will
 * trust the second one as much as the first.
 *
 * **Only optional slides are ever offered.** The author decided what the
 * argument is. A tool that suggests cutting the core of it is a tool nobody
 * takes advice from twice.
 */

import { formatDuration } from "./timer";

/** How the talk is going against the deck, as a token a theme can style. */
export type Pace = "ahead" | "on-pace" | "behind" | "unknown";

/** Where the expectation came from. It changes what the number is worth. */
export type PaceBasis = "budgets" | "mixed" | "uniform" | "none";

/** One slide, reduced to what pacing reads from it. */
export interface PaceSlide {
  index: number;
  title?: string;
  /** The author's own budget for this slide, when they set one. */
  budgetMs?: number;
  /** Marked droppable by the author. The only slides that may be suggested. */
  optional?: boolean;
}

export interface PaceInput {
  slides: readonly PaceSlide[];
  /** Zero-based index of the slide on screen. */
  position: number;
  elapsedMs: number;
  /** The slot. Without one there is no pace, only a clock. */
  budgetMs?: number | undefined;
  running: boolean;
}

/** A slide that may be dropped, and what dropping it buys. */
export interface SkippableSlide {
  index: number;
  title?: string;
  budgetMs: number;
}

export interface PaceState {
  pace: Pace;
  basis: PaceBasis;
  /** Positive when behind, negative when ahead, zero on pace. */
  driftMs?: number;
  /** Earliest the deck expects the speaker to have reached this slide. */
  expectedFromMs?: number;
  /** Latest, which is the same instant plus this slide's own budget. */
  expectedToMs?: number;
  /** Optional slides still ahead, in deck order. */
  skippable: SkippableSlide[];
  /** What dropping every one of them would recover. */
  recoverableMs: number;
}

export interface PaceOptions {
  /** Overrides the derived dead zone. */
  toleranceMs?: number;
}

/**
 * The dead zone, as a share of the slot.
 *
 * Twenty seconds over is noise a speaker cannot act on, and a display that
 * flips to a warning for it is a display they stop reading. Five per cent is
 * about a minute in a twenty-minute slot.
 */
const TOLERANCE_SHARE = 0.05;

/**
 * Below which the share is too small to absorb ordinary variation.
 *
 * A five-minute lightning talk gets thirty seconds rather than fifteen: even
 * in a short slot, half a minute is inside the noise of one long sentence.
 */
const MINIMUM_TOLERANCE_MS = 30_000;

export function assessPace(input: PaceInput, options: PaceOptions = {}): PaceState {
  const { slides, position, elapsedMs, budgetMs, running } = input;

  const skippable = skippableAfter(slides, position, budgetMs);
  const recoverableMs = skippable.reduce((total, slide) => total + slide.budgetMs, 0);

  // No slot means no expectation to measure against. The timer still counts;
  // there is simply nothing to say about whether the count is a problem.
  if (budgetMs === undefined || budgetMs <= 0 || slides.length === 0) {
    return { pace: "unknown", basis: "none", skippable, recoverableMs };
  }

  const durations = expectedDurations(slides, budgetMs);
  const basis = basisOf(slides);

  const expectedFromMs = durations.slice(0, position).reduce(sum, 0);
  const expectedToMs = expectedFromMs + (durations[position] ?? 0);

  // A talk that has not started is not behind. Waiting for the room to settle
  // is not time the speaker has spent.
  if (!running && elapsedMs === 0) {
    return { pace: "unknown", basis, expectedFromMs, expectedToMs, skippable, recoverableMs };
  }

  const tolerance = options.toleranceMs ?? derivedTolerance(budgetMs);

  const { pace, driftMs } = compare(elapsedMs, expectedFromMs, expectedToMs, tolerance);

  return { pace, basis, driftMs, expectedFromMs, expectedToMs, skippable, recoverableMs };
}

function compare(
  elapsedMs: number,
  fromMs: number,
  toMs: number,
  tolerance: number,
): { pace: Pace; driftMs: number } {
  if (elapsedMs > toMs + tolerance) return { pace: "behind", driftMs: elapsedMs - toMs };
  if (elapsedMs < fromMs - tolerance) return { pace: "ahead", driftMs: elapsedMs - fromMs };

  return { pace: "on-pace", driftMs: 0 };
}

function derivedTolerance(budgetMs: number): number {
  return Math.max(MINIMUM_TOLERANCE_MS, budgetMs * TOLERANCE_SHARE);
}

/**
 * How long each slide is expected to take.
 *
 * Declared budgets are used as written; whatever slot time they leave is
 * shared evenly among the slides that declared nothing. A deck with budgets on
 * only the slides the author worried about is the common case, and falling
 * back to a slide count there would throw away the numbers they did write.
 *
 * The remainder is floored at zero. Budgets that already exceed the slot are a
 * real problem and `slidx_lint` is the thing that reports it — pacing should
 * not report it a second time in a place the speaker cannot act on it.
 */
function expectedDurations(slides: readonly PaceSlide[], budgetMs: number): number[] {
  const declared = slides.reduce((total, slide) => total + (slide.budgetMs ?? 0), 0);
  const undeclared = slides.filter((slide) => slide.budgetMs === undefined).length;
  const share = undeclared === 0 ? 0 : Math.max(0, budgetMs - declared) / undeclared;

  return slides.map((slide) => slide.budgetMs ?? share);
}

function basisOf(slides: readonly PaceSlide[]): PaceBasis {
  const declared = slides.filter((slide) => slide.budgetMs !== undefined).length;

  if (declared === slides.length) return "budgets";
  if (declared === 0) return "uniform";

  return "mixed";
}

/** Optional slides the speaker has not reached yet. */
function skippableAfter(
  slides: readonly PaceSlide[],
  position: number,
  budgetMs: number | undefined,
): SkippableSlide[] {
  const durations =
    budgetMs === undefined || budgetMs <= 0 ? undefined : expectedDurations(slides, budgetMs);

  return slides
    .filter((slide) => slide.optional === true && slide.index > position)
    .map((slide) => ({
      index: slide.index,
      ...(slide.title === undefined ? {} : { title: slide.title }),
      budgetMs: slide.budgetMs ?? durations?.[slide.index] ?? 0,
    }));
}

function sum(total: number, value: number): number {
  return total + value;
}

/**
 * One line, for the corner of a presenter view.
 *
 * Being behind is only actionable if something can be done about it, so the
 * line names the slides rather than the deficit alone. On the last slide there
 * is nothing left to drop and the only remaining move is to stop talking,
 * which is what it says instead.
 */
export function describePace(state: PaceState): string {
  if (state.pace === "unknown") return "";
  if (state.pace === "on-pace") return "on pace";

  const drift = formatDuration(Math.abs(state.driftMs ?? 0));

  if (state.pace === "ahead") return `${drift} ahead`;

  if (state.skippable.length === 0) {
    return `${drift} behind — nothing left to drop, wrap up`;
  }

  const names = state.skippable
    .map((slide) => slide.title ?? `slide ${slide.index + 1}`)
    .join(", ");

  return `${drift} behind — drop ${names} to recover ${formatDuration(state.recoverableMs)}`;
}
