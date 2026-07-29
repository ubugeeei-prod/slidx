/**
 * What the speaker actually did, recorded while they do it.
 *
 * A deck declares `budget:` per slide. That is a plan, and a plan is a guess
 * until it has been run once. This module records the other half — how long the
 * speaker really spent on each slide — so the two can be diffed. Without it, a
 * speaker who is six minutes over knows only that they are six minutes over,
 * which is the one fact that does not tell them what to cut.
 *
 * Dwell is accumulated **per slide, not per visit**, because a talk is not a
 * straight line. A speaker goes back to re-explain a diagram, and that second
 * visit is more time on that slide rather than a correction to the first.
 *
 * The recording is plain JSON and self-describing: it carries the budgets and
 * the tolerances it was made with, so a report can be recomputed from a
 * reloaded recording with no deck present, and compared against last week's.
 *
 * # Surviving the rehearsal being abandoned
 *
 * Most rehearsals are abandoned. The speaker is interrupted, or stops caring
 * halfway, and nobody presses a stop button. So this records defensively:
 *
 * - Every change emits a fresh snapshot through `onChange`. A recorder that
 *   waited to be asked for its recording would, in the common case, never be
 *   asked.
 * - Ending explicitly is still supported and still better — `finish` and
 *   `abandon` mean different things to a report — but neither is required for
 *   the measurement to be worth reading.
 * - What is lost when a tab closes is the dwell on the slide that was on
 *   screen, because nothing observed it ending. That is the right thing to
 *   lose: the speaker walked away during that slide, and counting it would
 *   charge the talk for the coffee break.
 *
 * The clock is injected. A rehearsal is forty minutes long, and a suite that
 * measured one with real time would be a suite nobody runs.
 */

/** A slide as the deck declares it. */
export interface RehearsalSlide {
  /** The slug `slidx_core` assigns, stable across builds. */
  id: string;
  /** What the deck budgeted for it. Absent when the slide declares nothing. */
  budgetMs?: number | undefined;
}

/** Where a rehearsal is in its life. */
export type RehearsalStatus =
  /** Created, nothing recorded yet. */
  | "idle"
  | "recording"
  | "paused"
  /** The speaker reached the end and said so. Diffed against the whole deck. */
  | "finished"
  /** Given up part-way. Diffed only against the slides that were reached. */
  | "abandoned";

export interface RehearsalState {
  status: RehearsalStatus;
  /** The slide the clock is on, or null before the first visit and after finishing. */
  slideId: string | null;
  /** Everything recorded so far, across every slide. Excludes paused time. */
  elapsedMs: number;
  /** Dwell on the current slide across every visit to it, so far. */
  slideMs: number;
}

/** One slide's totals, as stored. */
export interface RecordedSlide {
  id: string;
  budgetMs?: number;
  /** Dwell summed across every visit. */
  actualMs: number;
  /** How many times the speaker landed here. More than one means a re-explain. */
  visits: number;
}

/**
 * A rehearsal as plain JSON.
 *
 * Carries its own tolerances so a report recomputed from storage is the same
 * report, and a `version` so a recording written by an older slidx is rejected
 * loudly instead of quietly read as a rehearsal nobody gave.
 */
export interface RehearsalRecording {
  version: 1;
  status: RehearsalStatus;
  toleranceMs: number;
  totalToleranceMs: number;
  slides: RecordedSlide[];
  /** The slide the clock was on when this was taken, so a reload can resume. */
  currentSlide?: string;
}

export interface Rehearsal {
  /** The speaker is now on this slide. The first call starts the recording. */
  visit(slideId: string): void;
  /** Stops the clock without ending the rehearsal. Used when a question runs long. */
  pause(): void;
  /** Restarts the clock on the slide it was paused on. */
  resume(): void;
  /** The talk is over. The report is diffed against the whole deck. */
  finish(): void;
  /** Stop, keeping what was recorded. The report says how far it got. */
  abandon(): void;
  state(): RehearsalState;
  /** A plain-JSON snapshot. Survives a reload; `restoreRehearsal` reads it back. */
  toJSON(): RehearsalRecording;
}

export interface RehearsalOptions {
  /** The deck, in order. Slides visited but not listed are appended as they appear. */
  slides?: readonly RehearsalSlide[];
  /** Injected so a rehearsal is a specification rather than a suite full of sleeps. */
  now?: () => number;
  /** Below this, a slide's difference from its budget is delivery, not a plan to fix. */
  toleranceMs?: number;
  /** Below this, the talk fits its slot. */
  totalToleranceMs?: number;
  /**
   * Called with a fresh snapshot whenever the recording changes.
   *
   * Wire it to storage. This is what makes an abandoned rehearsal survive the
   * tab that recorded it, which is most of them.
   */
  onChange?: (recording: RehearsalRecording) => void;
}

/**
 * How far a slide may drift before the report says anything about it.
 *
 * Fifteen seconds is about one sentence plus the pause after it. A speaker
 * cannot pace a slide more finely than that, so a report that flagged a slide
 * for running four seconds long would be reporting the speaker's breathing.
 */
export const TOLERANCE_MS = 15_000;

/**
 * How far the whole talk may drift before the report says anything.
 *
 * A minute, because the last minute of a slot is the buffer every speaker
 * already leaves for the walk to the lectern and the question that lands early.
 * Reusing the fifteen-second slide tolerance here would call a twenty-minute
 * talk that ran twenty minutes and sixteen seconds "over", which is true and
 * useless.
 */
export const TOTAL_TOLERANCE_MS = 60_000;

export function createRehearsal(options: RehearsalOptions = {}): Rehearsal {
  return record({
    now: options.now ?? (() => Date.now()),
    toleranceMs: options.toleranceMs ?? TOLERANCE_MS,
    totalToleranceMs: options.totalToleranceMs ?? TOTAL_TOLERANCE_MS,
    onChange: options.onChange,
    status: "idle",
    currentSlide: null,
    slides: (options.slides ?? []).map((slide) => ({
      id: slide.id,
      ...(slide.budgetMs === undefined ? {} : { budgetMs: slide.budgetMs }),
      actualMs: 0,
      visits: 0,
    })),
  });
}

/**
 * Picks a stored rehearsal back up.
 *
 * A restored rehearsal that was mid-talk comes back *paused* rather than
 * running, because the gap between the snapshot and the restore is a page load
 * and not talking. Backdating the clock to the snapshot would charge every
 * recovered rehearsal for the reload, making it look worse than the talk that
 * produced it.
 */
export function restoreRehearsal(
  recording: RehearsalRecording,
  options: Pick<RehearsalOptions, "now" | "onChange"> = {},
): Rehearsal {
  if (recording.version !== 1) {
    throw new Error(`unsupported rehearsal recording version: ${String(recording.version)}`);
  }

  return record({
    now: options.now ?? (() => Date.now()),
    onChange: options.onChange,
    toleranceMs: recording.toleranceMs,
    totalToleranceMs: recording.totalToleranceMs,
    status: recording.status === "recording" ? "paused" : recording.status,
    currentSlide: recording.currentSlide ?? null,
    slides: recording.slides.map((slide) => ({ ...slide })),
  });
}

interface RecorderConfig {
  now: () => number;
  toleranceMs: number;
  totalToleranceMs: number;
  onChange: ((recording: RehearsalRecording) => void) | undefined;
  status: RehearsalStatus;
  currentSlide: string | null;
  slides: RecordedSlide[];
}

/**
 * The one recorder, shared by a fresh rehearsal and a restored one.
 *
 * Restoring differs from starting only in what the totals begin at, so there is
 * no second implementation to keep in step — and no public way to tell a
 * recorder what it recorded, which would not be a recording.
 */
function record(config: RecorderConfig): Rehearsal {
  const { now } = config;

  // Insertion order is deck order, which is why this is a Map rather than a
  // record: the report lists slides in the order the audience sees them, and a
  // record's key order is only reliable for keys that are not integer-like.
  const slides = new Map(config.slides.map((slide) => [slide.id, slide]));

  let status = config.status;
  let currentSlide = config.currentSlide;
  /** When the running visit's clock started, or null whenever nothing is running. */
  let runningSince: number | null = null;

  const ended = () => status === "finished" || status === "abandoned";
  const running = () => (runningSince === null ? 0 : now() - runningSince);

  /**
   * A slide the deck did not declare.
   *
   * A rehearsal must never refuse a navigation: a speaker jumping to a backup
   * slide, or rehearsing a deck whose slugs have since changed, is spending
   * real time that would otherwise vanish. It is appended unbudgeted, which is
   * exactly what the report should then say about it.
   */
  function track(slideId: string): RecordedSlide {
    const existing = slides.get(slideId);
    if (existing) return existing;

    const added: RecordedSlide = { id: slideId, actualMs: 0, visits: 0 };
    slides.set(slideId, added);
    return added;
  }

  /** Banks the running visit onto its slide and stops the clock. */
  function bank(): void {
    if (runningSince === null || currentSlide === null) return;

    track(currentSlide).actualMs += now() - runningSince;
    runningSince = null;
  }

  function snapshot(): RehearsalRecording {
    const live = running();

    return {
      version: 1,
      status,
      toleranceMs: config.toleranceMs,
      totalToleranceMs: config.totalToleranceMs,
      // Cloned with the running visit folded in, so a snapshot taken mid-slide
      // is not missing the minute currently being spent on it.
      slides: [...slides.values()].map((slide) => ({
        ...slide,
        actualMs: slide.actualMs + (slide.id === currentSlide ? live : 0),
      })),
      ...(currentSlide === null ? {} : { currentSlide }),
    };
  }

  /** Publishes the recording after anything that changed it. */
  function changed(): void {
    config.onChange?.(snapshot());
  }

  return {
    visit(slideId) {
      // A rehearsal that has ended is a record, not a session. Reopening it on
      // a stray arrow key would corrupt the numbers the speaker is reading.
      if (ended()) return;

      bank();

      track(slideId).visits += 1;
      currentSlide = slideId;
      // Navigating is presenting, so a visit while paused resumes: a speaker
      // who answered a question and moved on should not have to press two
      // keys, one of which they will forget.
      status = "recording";
      runningSince = now();

      changed();
    },

    pause() {
      if (status !== "recording") return;

      bank();
      status = "paused";
      changed();
    },

    resume() {
      // Resuming before the first visit has nothing to resume onto.
      // Attributing that time to whichever slide the speaker later opens would
      // be an invention, and an invention is worse than a gap.
      if (status !== "paused" || currentSlide === null) return;

      status = "recording";
      runningSince = now();
      changed();
    },

    finish() {
      if (ended()) return;

      bank();
      status = "finished";
      currentSlide = null;
      changed();
    },

    abandon() {
      if (ended()) return;

      bank();
      status = "abandoned";
      // The slide is kept: "gave up on slide 7" is the most useful single fact
      // about a rehearsal that stopped.
      changed();
    },

    state() {
      const banked = [...slides.values()].reduce((total, slide) => total + slide.actualMs, 0);
      const onSlide = currentSlide === null ? 0 : (slides.get(currentSlide)?.actualMs ?? 0);

      return {
        status,
        slideId: currentSlide,
        elapsedMs: banked + running(),
        slideMs: onSlide + running(),
      };
    },

    toJSON: snapshot,
  };
}
