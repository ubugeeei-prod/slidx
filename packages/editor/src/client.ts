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

/** A slide, as the outline and the canvas need it. */
export interface SlideSummary {
  id: string;
  index: number;
  title?: string;
  notes: string[];
  stopCount: number;
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

export interface EditorClient {
  deck(): Promise<DeckState>;
  apply(op: EditOp): Promise<EditAnswer>;
  revert(edit: Edit): Promise<EditAnswer>;
}

export interface ClientOptions {
  /** Where the editing routes live. */
  base?: string;
  fetch?: typeof globalThis.fetch;
}

export function createClient(options: ClientOptions = {}): EditorClient {
  const base = options.base ?? "/__slidx/";
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
  };
}
