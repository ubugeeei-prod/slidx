/**
 * Which slides are getting worse.
 *
 * One rehearsal tells a speaker where the time went. Three tell them whether
 * the changes they made are working — a better question, and the only one that
 * separates a slide that is over budget because it is genuinely hard from a
 * slide that is over budget and still drifting.
 *
 * Three decisions shape this:
 *
 * **The comparison is against the previous run, not the first.** A speaker who
 * took slide 7 from 4m to 2m to 2m30 has a slide creeping back up. Measured
 * against the first run that reads as a 90-second improvement, and the drift
 * stays invisible until the stage.
 *
 * **A regression needs the slide to be over budget as well as slipping.**
 * Growing from 20s to 50s inside a two-minute budget is a slide with room, and
 * naming it would spend the speaker's attention on the one place they have
 * slack. `slides` still reports the direction, so nothing is hidden — but the
 * sentence is about what to act on.
 *
 * **An improvement is worth saying out loud.** A slide that is over budget and
 * two minutes faster than last time is being fixed. A report that only ever
 * listed problems would send the speaker to cut it again, undoing work that is
 * already working.
 *
 * Comparison is a pure function of the recordings, so a history kept in
 * `localStorage`, in a file, or in a CI artefact all produce the same trend.
 */

import { formatList, formatSpan } from "./duration";
import type { RecordedSlide, RehearsalRecording } from "./rehearsal";

export type TrendDirection =
  | "slower"
  | "faster"
  /** Moved by less than a slide's tolerance, which is not a direction. */
  | "steady"
  /** Given this run, but not the last one, so there is nothing to compare. */
  | "new";

export interface SlideTrend {
  id: string;
  /** 1-based position in the latest run. */
  index: number;
  actualMs: number;
  /** Dwell in the previous run. Zero when the slide is new. */
  previousMs: number;
  /** `latest − previous`. Positive means slower. Zero when the slide is new. */
  deltaMs: number;
  direction: TrendDirection;
  /** True when the slide is still past the budget the deck declares. */
  overBudget: boolean;
}

export interface RehearsalTrend {
  /** How many rehearsals the history holds, including ones not compared. */
  runs: number;
  /** Every slide the latest run reached, in deck order. */
  slides: SlideTrend[];
  /** Slipping *and* over budget, worst first. What to act on. */
  regressions: SlideTrend[];
  /** One sentence about the direction of travel. */
  note: string;
}

/**
 * The most slides the sentence will name.
 *
 * Two, because a trend note sits underneath a per-slide table that is already
 * a better list than a sentence can be. The sentence exists to say the thing a
 * speaker should remember while walking away from the laptop.
 */
const MAX_NAMED = 2;

/**
 * Compares the latest rehearsal against the one before it.
 *
 * `history` is in chronological order, oldest first. Anything before the last
 * two runs is counted but not compared — see the module docs for why the
 * previous run is the right baseline.
 */
export function trackRehearsals(history: readonly RehearsalRecording[]): RehearsalTrend {
  const latest = history.at(-1);
  const previous = history.at(-2);

  if (!latest) {
    return { runs: 0, slides: [], regressions: [], note: "No rehearsal has been recorded yet." };
  }

  const before = new Map((previous?.slides ?? []).map((slide) => [slide.id, slide]));

  // Only slides the latest run reached. A slide that was not given this time
  // has no dwell to compare, and reporting it as "faster" because it was
  // skipped would be the most misleading reading available.
  const slides = latest.slides
    .filter((slide) => slide.visits > 0)
    .map((slide, position) =>
      compare(slide, before.get(slide.id), position + 1, latest.toleranceMs),
    );

  const regressions = slides
    .filter((slide) => slide.direction === "slower" && slide.overBudget)
    .sort((left, right) => right.deltaMs - left.deltaMs || left.index - right.index);

  return {
    runs: history.length,
    slides,
    regressions,
    note: noteFor(history.length, slides, regressions),
  };
}

function compare(
  slide: RecordedSlide,
  previous: RecordedSlide | undefined,
  index: number,
  toleranceMs: number,
): SlideTrend {
  const overBudget = slide.budgetMs !== undefined && slide.actualMs > slide.budgetMs;
  const base = { id: slide.id, index, actualMs: slide.actualMs, overBudget };

  // A slide the previous run never reached is new rather than infinitely
  // slower: it was not given, so there is no earlier number to be worse than.
  if (!previous || previous.visits === 0) {
    return { ...base, previousMs: 0, deltaMs: 0, direction: "new" };
  }

  const deltaMs = slide.actualMs - previous.actualMs;

  return {
    ...base,
    previousMs: previous.actualMs,
    deltaMs,
    direction: Math.abs(deltaMs) <= toleranceMs ? "steady" : deltaMs > 0 ? "slower" : "faster",
  };
}

function noteFor(
  runs: number,
  slides: readonly SlideTrend[],
  regressions: readonly SlideTrend[],
): string {
  if (runs < 2) {
    return "First rehearsal recorded — a second one is what makes it a trend.";
  }

  if (regressions.length > 0) return regressionNote(regressions);

  // Nothing is slipping. Say whether that is because the deck is stable or
  // because the speaker's cuts are landing, which are different pieces of news.
  const improved = slides.filter((slide) => slide.direction === "faster");

  if (improved.length === 0) return "Nothing got slower since the last rehearsal.";

  const best = [...improved].sort((left, right) => left.deltaMs - right.deltaMs);

  return `Nothing got slower, and ${namesOf(best)} came in ${formatSpan(Math.abs(best[0]?.deltaMs ?? 0))} faster than last time.`;
}

function regressionNote(regressions: readonly SlideTrend[]): string {
  const named = regressions.slice(0, MAX_NAMED);
  const worst = named[0];

  if (!worst) return "Nothing got slower since the last rehearsal.";

  if (named.length === 1) {
    return `Slide ${worst.index} is ${formatSpan(worst.deltaMs)} slower than last time and still over budget.`;
  }

  const rest = regressions.length - named.length;
  const others = rest > 0 ? `, and ${rest} more` : "";

  return `${capitalise(namesOf(named))} are slipping and still over budget${others} — slide ${worst.index} worst, ${formatSpan(worst.deltaMs)} slower than last time.`;
}

function namesOf(slides: readonly SlideTrend[]): string {
  const named = slides.slice(0, MAX_NAMED).map((slide) => String(slide.index));

  return named.length === 1 ? `slide ${named[0]}` : `slides ${formatList(named)}`;
}

function capitalise(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
