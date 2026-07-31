/**
 * How long each slide is, and how that lies against the slot.
 *
 * Nothing here draws anything and nothing here judges anything. It turns the
 * numbers the pipeline already resolved into the widths the storyboard needs,
 * which is one arithmetic problem worth keeping away from the DOM: a width that
 * misrepresents a talk's length is the one bug this surface could have that a
 * speaker would not notice until they were on stage.
 *
 * # Why the total mixes two kinds of number
 *
 * The linter refuses to add up per-slide budgets unless *every* slide declares
 * one, because a partial sum compared against a slot would read as the whole
 * talk. That is right for a diagnostic, which is one line of text. It is wrong
 * here: this surface has room to mark each slide with which number it used, so
 * the reader can see the mixture rather than having to be protected from it.
 *
 * Whether a total is a problem is still the linter's to say, and it says it two
 * panels down. This only states what the numbers are.
 */

import type { SlideSummary } from "../client";

/** Which number a slide's width came from. */
export type TimeSource = "budget" | "estimate";

/** One slide, as a length. */
export interface SlideTime {
  index: number;
  title: string;
  /** What the speaker says here, as one block. Empty when nothing is written. */
  message: string;
  seconds: number;
  source: TimeSource;
  optional: boolean;
  /**
   * The budget as the author typed it.
   *
   * `90s`, `1m30s` and `1:30` are the same length, and the one in the file is
   * theirs. Showing the resolved seconds back would make a field nobody touched
   * turn up in the diff.
   */
  written: string;
  /** Fraction of the bar this slide takes, from 0 to 1. */
  share: number;
}

/** The deck as lengths, and what they come to. */
export interface Plan {
  slides: SlideTime[];
  /** Every slide's seconds added up, budgets and estimates together. */
  plannedSeconds: number;
  /** The slot, when the deck declares one. */
  slotSeconds?: number | undefined;
  /** How far past the slot the deck runs. Zero when it fits or has no slot. */
  overSeconds: number;
  /** What is left of the slot. Zero when the deck does not fit or has no slot. */
  spareSeconds: number;
  /** Seconds carried by the slides marked optional. */
  slackSeconds: number;
  /** Slides with no budget and nothing written down, so no length at all. */
  untimed: number;
  /**
   * Fraction of the bar that is inside the slot, from 0 to 1.
   *
   * Absent when the deck declares no slot, and then there is no end for
   * anything to run past — which is a different picture, not a full bar.
   */
  slotShare?: number | undefined;
  /** No slide has any length yet, so there is no bar to draw. */
  empty: boolean;
}

/**
 * The deck as widths.
 *
 * The bar spans whichever is longer, the slot or the talk, so a deck that fits
 * fills part of its slot and a deck that does not carries on past where the slot
 * ends. Scaling to the talk instead would make every deck look exactly full,
 * which is the one thing this surface exists to stop being invisible.
 */
export function planTime(slides: readonly SlideSummary[], slotSeconds: number | undefined): Plan {
  const lengths = slides.map(lengthOf);
  const plannedSeconds = lengths.reduce((total, slide) => total + slide.seconds, 0);
  const span = Math.max(plannedSeconds, slotSeconds ?? 0);

  return {
    slides: lengths.map((slide) => ({
      ...slide,
      share: span === 0 ? 0 : slide.seconds / span,
    })),
    plannedSeconds,
    slotSeconds,
    overSeconds: Math.max(0, plannedSeconds - (slotSeconds ?? plannedSeconds)),
    spareSeconds: Math.max(0, (slotSeconds ?? plannedSeconds) - plannedSeconds),
    slackSeconds: lengths
      .filter((slide) => slide.optional)
      .reduce((total, slide) => total + slide.seconds, 0),
    untimed: lengths.filter((slide) => slide.seconds === 0).length,
    slotShare: slotSeconds === undefined || span === 0 ? undefined : slotSeconds / span,
    empty: span === 0,
  };
}

/**
 * One slide's length, and where it came from.
 *
 * A declared budget wins over an estimate even when it is shorter: the author
 * saying "this slide gets ninety seconds" is a decision, and the notes reading
 * longer than that is a finding the linter already reports.
 */
function lengthOf(slide: SlideSummary): Omit<SlideTime, "share"> {
  const budget = slide.budgetSeconds;
  const written = slide.frontmatter?.["budget"];

  return {
    index: slide.index,
    title: slide.title ?? `Slide ${slide.index + 1}`,
    message: slide.notes.join("\n\n"),
    seconds: budget ?? slide.estimatedSeconds,
    source: budget === undefined ? "estimate" : "budget",
    optional: slide.optional,
    written: written === undefined || written === null ? "" : String(written),
  };
}

/**
 * A duration in the notation an author writes one in.
 *
 * The same spelling the linter's timing findings use — `90s`, `1m30s`, `20m` —
 * so a number read here and a number read in a diagnostic are recognisably the
 * same number. It is deliberately not the presenter clock's `1:30`, which reads
 * as a time of day inside a sentence, and deliberately not imported from
 * `@slidxjs/rehearsal`: the editor is served to the browser as one self-contained
 * module by the dev server, and a bare import in it would not resolve.
 */
export function formatSeconds(seconds: number): string {
  const whole = Math.round(seconds);
  if (whole < 60) return `${whole}s`;

  const rest = whole % 60;
  return rest === 0 ? `${whole / 60}m` : `${Math.floor(whole / 60)}m${rest}s`;
}
