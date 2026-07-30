/**
 * The dev server, as the editor talks to it.
 *
 * Two calls: read the deck, and ask for one change. The server owns the files
 * and the pipeline owns the bytes, so this is a courier and holds nothing.
 *
 * `fetch` is injected so the whole editor can be driven in a test without a
 * server, which is the only way the surfaces below are testable at all.
 */

import type { Edit, EditOp, EditRefusal } from "./operations";

/** One thing a slide's steps can name, which is one row of the timeline. */
export interface StepRow {
  target: string;
  label: string;
  /** The mark's `#key`. Absent for a row `autoSteps:` or a marker staged. */
  key?: string;
  /**
   * Whether the row is painted at each stop, one entry per stop.
   *
   * Off the compiled frames rather than folded out of the actions here, so the
   * bar a cell draws cannot disagree with what the deck shows.
   */
  visible: boolean[];
}

/** One authored action, placed on the grid. */
export interface StepPlacement {
  /** Position in the slide's action list, which is what an operation names. */
  index: number;
  kind: "reveal" | "hide" | "emphasize" | "set" | "group";
  /** The stop it lands on. */
  stop: number;
  targets: string[];
  /** True when it plays on a timer and shares the stop before it. */
  timed: boolean;
  /** Canonical `steps:` source, without the leading `- `. */
  source: string;
}

/**
 * A slide's steps as rows and stops.
 *
 * Mirrors `StepGrid` in `crates/slidx_core/src/grid.rs`, which is the
 * definition. Which stop an action lands on is decided there, because it is a
 * rule of the step compiler and a second answer to it here would put a click on
 * the wrong column.
 */
export interface StepGrid {
  rows: StepRow[];
  actions: StepPlacement[];
  /** Stops on this slide, including the resting frame. Always at least one. */
  stops: number;
  /** True when the author wrote `steps:`, so a cell has a line to change. */
  declared: boolean;
  /** The `autoSteps:` mode in force. */
  auto?: "list" | "block" | "row";
}

/** A slide, as the outline and the canvas need it. */
export interface SlideSummary {
  id: string;
  index: number;
  title?: string;
  notes: string[];
  stopCount: number;
  /** This slide's steps as rows and stops, for the timeline. */
  steps?: StepGrid;
  /** The keys the author wrote on this slide. The deck's own are slide zero's. */
  frontmatter?: Record<string, unknown>;
}

/** A parse diagnostic or a lint finding. */
export interface Finding {
  severity: string;
  code: string;
  message: string;
  help?: string;
  slideIndex?: number;
}

/** Where one slide's bytes are in the deck source. */
export interface SlideSpans {
  content: { start: number; end: number };
  body: { start: number; end: number };
}

/** Everything the editor needs to draw itself once. */
export interface DeckState {
  source: string;
  spans: SlideSpans[];
  deck: {
    title?: string;
    slides: SlideSummary[];
    diagnostics: Finding[];
    hasBlocking: boolean;
  };
}

/** What came back from asking for a change. */
export interface EditAnswer extends DeckState {
  /** The edit that takes this one back. Absent when the deck was not changed. */
  undo?: Edit;
  /** Which files were written. */
  written?: string[];
  /** Set when the operation named something the deck does not have. */
  error?: EditRefusal;
}

/**
 * What a browser found when it laid one box out.
 *
 * Mirrors `slidx_lint::Measurement`. Shares of the box rather than pixels,
 * because the rule that reads them is comparing against a box whose size it
 * does not know.
 */
export interface Measurement {
  slideIndex: number;
  stop: number;
  overHeight: number;
  overWidth: number;
  /** The region measured, when it is one rather than the whole slide. */
  region?: string;
}

export interface EditorClient {
  deck(): Promise<DeckState>;
  apply(op: EditOp): Promise<EditAnswer>;
  revert(edit: Edit): Promise<EditAnswer>;
  /**
   * What the linter makes of a measurement the editor took.
   *
   * The one call that changes nothing. Overflow is the rule no build-time model
   * can answer — it depends on where lines break — so the editor measures the
   * canvas and the pipeline decides what the numbers mean. That is what lets a
   * block being dragged into a column too narrow for it say so *before* it
   * lands, in the sentence a build would have used.
   */
  measured(measured: Measurement[]): Promise<Finding[]>;
}

/**
 * The prefix every route the editor talks to lives under.
 *
 * Named here because a panel that reads something other than the deck — the
 * history panel does — still has to find the dev server, and a second copy of
 * this string is a second place to change when the prefix moves.
 */
export const EDITOR_BASE = "/__slidx/";

export interface ClientOptions {
  /** Where the editing routes live. */
  base?: string;
  fetch?: typeof globalThis.fetch;
}

export function createClient(options: ClientOptions = {}): EditorClient {
  const base = options.base ?? EDITOR_BASE;
  const send = options.fetch ?? globalThis.fetch.bind(globalThis);

  async function post(body: unknown): Promise<EditAnswer> {
    const response = await send(`${base}edit`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });

    const payload = (await response.json()) as EditAnswer & { message?: string };

    // A deck the server cannot write is the one thing worth stopping for; a
    // refusal to name a missing slide comes back as an ordinary answer.
    if (!response.ok) throw new Error(payload.message ?? `The deck could not be written.`);

    return payload;
  }

  return {
    async deck() {
      const response = await send(`${base}deck`);
      return (await response.json()) as DeckState;
    },
    apply: (op) => post({ op }),
    revert: (edit) => post({ edit }),

    async measured(measured) {
      const response = await send(`${base}measured`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ measured }),
      });

      // A question about a landing is worth nothing if it fails, so it says
      // nothing rather than interrupting a drag with an error.
      return response.ok ? ((await response.json()) as Finding[]) : [];
    },
  };
}
