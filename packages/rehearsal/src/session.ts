/**
 * A rehearsal that survives presenter-page navigation.
 *
 * Presenter pages are ordinary HTML documents, one per slide. Moving forward
 * destroys the JavaScript object that was timing the current slide, so the
 * recorder alone cannot distinguish a navigation from closing the talk. This
 * adapter checkpoints the live snapshot, restores it on the next page, and
 * resumes only when the stored session was running.
 *
 * Storage is an argument rather than a direct `localStorage` dependency. That
 * keeps the package usable outside a browser and, more importantly, lets a
 * presenter keep working when a `file://` browser denies storage access.
 */

import {
  createRehearsal,
  restoreRehearsal,
  type Rehearsal,
  type RehearsalOptions,
  type RehearsalRecording,
  type RehearsalState,
} from "./rehearsal";
import { buildReport, type RehearsalReport } from "./report";

/** The part of Web Storage a rehearsal needs. */
export interface RehearsalStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export type RehearsalPersistence = "available" | "unavailable";

export interface RehearsalSessionOptions extends Pick<
  RehearsalOptions,
  "slides" | "now" | "toleranceMs" | "totalToleranceMs"
> {
  /** Stable deck-specific key. The caller owns namespacing it. */
  key: string;
  /** The slide rendered by this presenter page. */
  slideId: string;
  /** Usually `window.localStorage`; absent when persistence is not wanted. */
  storage?: RehearsalStorage;
}

export interface RehearsalSession {
  /** Start here, or resume here after an explicit pause. */
  start(): void;
  /** Record a same-document navigation. Cross-document navigation restores automatically. */
  visit(slideId: string): void;
  pause(): void;
  /** Finish and return the actionable, per-slide report. */
  finish(): RehearsalReport;
  /** Keep a partial report when the speaker stops early. */
  abandon(): RehearsalReport;
  state(): RehearsalState;
  recording(): RehearsalRecording;
  report(): RehearsalReport;
  /**
   * Save the live slide without pausing it.
   *
   * Call from `pagehide`: the next presenter page sees a running recording and
   * resumes there, while an explicit pause remains paused.
   */
  checkpoint(): RehearsalPersistence;
  /**
   * Every run that has ended, oldest first.
   *
   * Not including the live one: a rehearsal in progress has no dwell time to
   * compare yet, and counting it would make the first slide of a talk look
   * like the whole of it.
   */
  history(): RehearsalRecording[];
  /**
   * Remove this run and return to an idle recorder for the current deck.
   *
   * The history is untouched. Starting again is not a reason to forget what
   * the last three rehearsals said.
   */
  reset(): void;
  persistence(): RehearsalPersistence;
}

/**
 * Opens the recording for one presenter page.
 *
 * A stored running session resumes on this page immediately. A stored paused,
 * finished, or abandoned session stays exactly that way: a reload must never
 * invent speaking time or reopen a completed report.
 */
/**
 * How many finished runs are kept beside the live one.
 *
 * Bounded because `localStorage` is a few megabytes shared with everything else
 * on the origin, and a deck rehearsed weekly for a year would otherwise grow
 * without anyone deciding to let it. Twenty is far more than the comparison
 * needs — `trackRehearsals` reads the last two — and enough that
 * `RehearsalTrend.runs` stays a true count of what is kept for any deck
 * somebody is actually working on.
 */
const HISTORY_LIMIT = 20;

/** Where the finished runs live, beside the live one rather than inside it. */
function historyKey(key: string): string {
  return `${key}:history`;
}

export function openRehearsalSession(options: RehearsalSessionOptions): RehearsalSession {
  let persistence: RehearsalPersistence = options.storage ? "available" : "unavailable";

  function write(recording: RehearsalRecording): void {
    if (!options.storage) return;

    try {
      options.storage.setItem(options.key, JSON.stringify(recording));
      persistence = "available";
    } catch {
      // Browsers may expose localStorage and still throw on access, notably on
      // file URLs and under privacy policies. Recording in memory remains
      // useful, so persistence failure is a status rather than an exception.
      persistence = "unavailable";
    }
  }

  /**
   * Every finished run, oldest first, which is the order `trackRehearsals` reads.
   *
   * A shape it cannot parse is treated as no history rather than as an error:
   * one rehearsal's trend is not worth failing a presenter page over, and the
   * next finished run replaces it.
   */
  function history(): RehearsalRecording[] {
    if (!options.storage) return [];

    try {
      const raw = options.storage.getItem(historyKey(options.key));
      const parsed: unknown = raw === null ? [] : JSON.parse(raw);

      return Array.isArray(parsed) ? (parsed as RehearsalRecording[]) : [];
    } catch {
      return [];
    }
  }

  /**
   * Files a run that has ended.
   *
   * Called from `finish` and `abandon` and nowhere else, which is what makes it
   * once per run: a reload restores a finished recording without ending it
   * again, and `reset` starts a new one rather than replaying this.
   *
   * An abandoned run is kept on purpose. A talk the speaker stopped halfway is
   * still where the time went for the slides they reached, and dropping it
   * would make the next comparison silently span two rehearsals rather than
   * one.
   */
  function archive(recording: RehearsalRecording): void {
    if (!options.storage) return;

    const kept = [...history(), recording].slice(-HISTORY_LIMIT);

    try {
      options.storage.setItem(historyKey(options.key), JSON.stringify(kept));
    } catch {
      // The live recording matters more than the history, and it is written
      // through its own key. Losing a trend is not worth losing a report.
      persistence = "unavailable";
    }
  }

  function remove(): void {
    if (!options.storage) return;

    try {
      options.storage.removeItem(options.key);
      persistence = "available";
    } catch {
      persistence = "unavailable";
    }
  }

  function read(): RehearsalRecording | undefined {
    if (!options.storage) return undefined;

    let raw: string | null;
    try {
      raw = options.storage.getItem(options.key);
      persistence = "available";
    } catch {
      persistence = "unavailable";
      return undefined;
    }

    if (raw === null) return undefined;

    try {
      return JSON.parse(raw) as RehearsalRecording;
    } catch {
      // A half-written or user-edited value is not a rehearsal. Removing it
      // makes the next reload deterministic instead of failing forever.
      remove();
      return undefined;
    }
  }

  function fresh(): Rehearsal {
    return createRehearsal({
      ...(options.slides === undefined ? {} : { slides: options.slides }),
      ...(options.now === undefined ? {} : { now: options.now }),
      ...(options.toleranceMs === undefined ? {} : { toleranceMs: options.toleranceMs }),
      ...(options.totalToleranceMs === undefined
        ? {}
        : { totalToleranceMs: options.totalToleranceMs }),
      onChange: write,
    });
  }

  const stored = read();
  let rehearsal: Rehearsal;

  if (stored === undefined) {
    rehearsal = fresh();
  } else {
    try {
      rehearsal = restoreRehearsal(stored, {
        ...(options.now === undefined ? {} : { now: options.now }),
        onChange: write,
      });
    } catch {
      // A valid JSON value can still have an unknown version or wrong shape.
      // Treat it like corrupt storage rather than breaking the presenter view.
      remove();
      rehearsal = fresh();
    }
  }

  // `restoreRehearsal` deliberately returns paused so the page-load gap is not
  // billed. Visiting now starts a fresh clock at this page and banks nothing
  // from that gap.
  if (stored?.status === "recording" && rehearsal.state().status === "paused") {
    rehearsal.visit(options.slideId);
  }

  return {
    start() {
      const state = rehearsal.state();
      if (state.status === "idle") {
        rehearsal.visit(options.slideId);
      } else if (state.status === "paused") {
        // A pause and resume on one slide is still one visit. If navigation
        // happened while paused, though, the new page is a genuine landing.
        if (state.slideId === options.slideId) rehearsal.resume();
        else rehearsal.visit(options.slideId);
      }
    },

    visit(slideId) {
      rehearsal.visit(slideId);
    },

    pause() {
      rehearsal.pause();
    },

    finish() {
      rehearsal.finish();
      const recording = rehearsal.toJSON();
      archive(recording);
      return buildReport(recording);
    },

    abandon() {
      rehearsal.abandon();
      const recording = rehearsal.toJSON();
      archive(recording);
      return buildReport(recording);
    },

    history,

    state() {
      return rehearsal.state();
    },

    recording() {
      return rehearsal.toJSON();
    },

    report() {
      return buildReport(rehearsal.toJSON());
    },

    checkpoint() {
      write(rehearsal.toJSON());
      return persistence;
    },

    reset() {
      remove();
      rehearsal = fresh();
    },

    persistence() {
      return persistence;
    },
  };
}
