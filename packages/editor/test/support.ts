/**
 * A dev server that only pretends to hold files.
 *
 * The editor's contract with the server is small — read a deck, ask for one
 * change, get the answer and an edit that takes it back — so a stand-in for it
 * is worth more than a running Vite. Every test below is about what the editor
 * does with that answer.
 */

import { byteLength } from "../src/bytes";
import type {
  DeckState,
  EditAnswer,
  EditorClient,
  Finding,
  Measurement,
  SlideSummary,
} from "../src/client";
import type { Edit, EditOp } from "../src/operations";

export interface Recorded {
  ops: EditOp[];
  reverted: Edit[];
  uploaded: File[];
  /** What the editor asked the linter about a landing that had not happened. */
  asked: Measurement[][];
}

export interface FakeServer extends EditorClient, Recorded {
  /** What the next answer will say. */
  answer: Partial<EditAnswer>;
  /** What the linter will make of the next measurement. */
  findings: Finding[];
}

export function deckOf(...titles: string[]): DeckState {
  const source = titles.map((title) => `# ${title}`).join("\n\n---\n\n");
  let cursor = 0;

  // Byte offsets, because that is what the pipeline reports and what an
  // operation names. A fixture that counted characters would hide the one bug
  // these spans exist to catch.
  const spans = titles.map((title) => {
    const start = cursor;
    const end = start + byteLength(`# ${title}`);
    cursor = end + byteLength("\n\n---\n\n");

    return { content: { start, end }, body: { start, end } };
  });

  const slides: SlideSummary[] = titles.map((title, index) => ({
    id: title.toLowerCase().replace(/\s+/g, "-"),
    index,
    title,
    notes: [],
    stopCount: 1,
    estimatedSeconds: 0,
    optional: false,
    style: {},
    frontmatter: index === 0 ? { title: "A Deck" } : {},
  }));

  return {
    source,
    spans,
    deck: {
      title: "A Deck",
      // A slot, because a deck without one cannot say whether it fits, and the
      // storyboard reads it through the session rather than from the frontmatter.
      durationSeconds: 600,
      layouts: [
        {
          id: "full",
          summary: "One region, the whole slide.",
          areas: ["body"],
          columns: "1fr",
          rows: "1fr",
        },
        {
          id: "aside",
          summary: "A main region beside supporting content.",
          areas: ["main side"],
          columns: "2fr 1fr",
          rows: "1fr",
        },
      ],
      slides,
      diagnostics: [],
      hasBlocking: false,
    },
  };
}

export function fakeServer(initial: DeckState = deckOf("One", "Two", "Three")): FakeServer {
  const server: FakeServer = {
    ops: [],
    reverted: [],
    uploaded: [],
    asked: [],
    answer: {},
    findings: [],

    deck: async () => initial,

    measured: async (measured) => {
      server.asked.push(measured);
      return server.findings;
    },

    apply: async (op) => {
      server.ops.push(op);
      return { ...initial, undo: [{ splice: server.ops.length }], ...server.answer };
    },

    revert: async (edit) => {
      server.reverted.push(edit);
      return { ...initial, undo: [{ splice: -server.reverted.length }], ...server.answer };
    },

    upload: async (file) => {
      server.uploaded.push(file);
      return {
        kind: file.type.startsWith("video/") ? "video" : "image",
        src: file.name,
        alt: "media",
      };
    },
  };

  return server;
}
