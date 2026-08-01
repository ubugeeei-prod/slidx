/**
 * The editor's one piece of state, and the only thing that changes it.
 *
 * Every surface reads this and none of them writes a deck: a click on the
 * outline, a committed heading, a class typed into the inspector all become an
 * [`EditOp`](./operations) handed to `run`. That keeps the number of things
 * that can disagree with the file at one.
 *
 * Deliberately not a framework. slidx is framework-agnostic and every
 * integration is opt-in and removable; an editor built on Vue or React would
 * make that claim false in the one place an author looks at all day. What a
 * store is actually for here is subscription, which is thirty lines.
 */

import { sliceBytes } from "./bytes";
import type { Viewer } from "./collab";
import type {
  BlockSpans,
  DeckState,
  EditorClient,
  Finding,
  LayoutChoice,
  SlideSpans,
  SlideSummary,
  ThemeChoice,
  TransitionChoice,
} from "./client";
import { createHistory, type History } from "./history";
import type { EditOp, EditRefusal } from "./operations";
import { locateSelection } from "./selection";

/** What is selected, which is what the inspector is about. */
/**
 * Where the author is.
 *
 * The optional fields are written `| undefined` on purpose. These are patched
 * rather than replaced, and moving to another slide *clears* the range — so a
 * caller has to be able to say "this field is now nothing", which is a
 * different statement from leaving the field out of the patch.
 */
export interface Selection {
  slide: number;
  /** One rendered block, addressed in the source order shared with the pipeline. */
  block?: number | undefined;
  /** A range of the current slide's source body, when text is selected. */
  range?: { start: number; end: number } | undefined;
  /** The text that range names, for showing back to the author. */
  text?: string | undefined;
}

export interface EditorState {
  source: string;
  spans: SlideSpans[];
  /** The deck title, for the workspace context above every surface. */
  title?: string | undefined;
  slides: SlideSummary[];
  /** Layouts the active pipeline can render, in picker order. */
  layouts: LayoutChoice[];
  /** Theme that produced the rendered canvas. */
  activeTheme: string;
  /** Whether build configuration, rather than the deck, chose that theme. */
  themeLocked: boolean;
  /** Themes the active pipeline can render, in picker order. */
  themes: ThemeChoice[];
  /** Slide transitions the active renderer implements, in picker order. */
  transitions: TransitionChoice[];
  /**
   * Length of the speaking slot, when the deck declares one.
   *
   * Deck-level rather than per-slide because it is what the per-slide budgets
   * are laid against, and nothing else in the deck answers "does this fit".
   */
  durationSeconds?: number | undefined;
  diagnostics: Finding[];
  selection: Selection;
  /**
   * Everyone connected to this dev server, the author included.
   *
   * State rather than a message passed between two surfaces, because it is
   * drawn in two places — as a list of names, and as marks over the blocks
   * those people have selected — and a second route into a second surface is a
   * second thing that can be out of date. Empty until a stream says otherwise,
   * which is also what an author working alone sees.
   */
  viewers: Viewer[];
  /** Whether this browser's link may change the deck. Absent means local/editor-owned. */
  canEdit?: boolean | undefined;
  /**
   * The seat this editor is following, when it is following one.
   *
   * A seat id rather than a slide number, because what an author asks for is
   * "show me what they are looking at" and a number stops being that the
   * moment the other person moves. Cleared by anything the author selects
   * themselves, and by the followed person closing their tab.
   */
  following?: string | undefined;
  canUndo: boolean;
  canRedo: boolean;
  /** True while a semantic edit is being written through the dev server. */
  writing: boolean;
  /** What the last operation was refused for, cleared by the next one. */
  refusal?: EditRefusal | undefined;
  /** A message about something that stopped, rather than something refused. */
  problem?: string | undefined;
  /**
   * What the linter says about something that has not happened yet.
   *
   * A block being dragged has a landing before it has a line in the file, and
   * whether it will fit the region it is heading for is knowable then. These
   * are cleared the moment the gesture ends, because a warning about a
   * placement nobody made is a warning about nothing.
   */
  foreseen?: Finding[] | undefined;
}

export interface Session {
  state(): EditorState;
  subscribe(listener: (state: EditorState) => void): () => void;
  /** Reads the deck as it is on disk. */
  open(): Promise<void>;
  /** Applies one operation and, when it succeeds, may land on its result. */
  run(op: EditOp, selection?: Selection): Promise<void>;
  undo(): Promise<void>;
  redo(): Promise<void>;
  select(selection: Partial<Selection>): void;
  /** What the linter says about a change that has not been made yet. */
  foresee(findings: Finding[]): void;
  /** Who is connected, as the dev server last said. */
  saw(viewers: Viewer[]): void;
  /** Moves with another seat until the author takes the wheel back. */
  follow(seat: string | undefined): void;
  /** The Markdown of one slide's body, as the author wrote it. */
  bodyOf(slide: number): string;
  /** The slide's complete Markdown, including its own frontmatter and notes. */
  contentOf(slide: number): string;
  /**
   * Where that body's blocks and marks are, for a gesture that names bytes.
   *
   * From the pipeline rather than worked out here: a second answer about where a
   * mark's words end would let typing in a paragraph take away the `#key` a step
   * animates.
   */
  blocksOf(slide: number): readonly BlockSpans[];
}

const EMPTY: EditorState = {
  source: "",
  spans: [],
  slides: [],
  layouts: [],
  activeTheme: "",
  themeLocked: false,
  themes: [],
  transitions: [],
  diagnostics: [],
  selection: { slide: 0 },
  viewers: [],
  canEdit: true,
  canUndo: false,
  canRedo: false,
  writing: false,
};

export function createSession(client: EditorClient, history: History = createHistory()): Session {
  let state = EMPTY;
  const listeners = new Set<(state: EditorState) => void>();

  function set(change: Partial<EditorState>): void {
    state = { ...state, ...change };
    for (const listener of listeners) listener(state);
  }

  /** The deck part of the answer, with the selection kept in range. */
  function adopt(deck: DeckState, extra: Partial<EditorState> = {}): void {
    const slide = Math.min(state.selection.slide, Math.max(deck.deck.slides.length - 1, 0));
    const block =
      slide === state.selection.slide &&
      state.selection.block !== undefined &&
      state.selection.block < (deck.spans[slide]?.blocks?.length ?? 0)
        ? state.selection.block
        : undefined;
    const text = relocatedText(deck, slide);

    set({
      source: deck.source,
      spans: deck.spans,
      title: deck.deck.title,
      slides: deck.deck.slides,
      layouts: deck.deck.layouts,
      activeTheme: deck.deck.activeTheme,
      themeLocked: deck.deck.themeLocked,
      themes: deck.deck.themes,
      transitions: deck.deck.transitions,
      durationSeconds: deck.deck.durationSeconds,
      diagnostics: deck.deck.diagnostics,
      // A live room announcement belongs to every viewer, so it deliberately
      // carries no access. Keep the capability established by the initial,
      // viewer-specific deck response in that case.
      canEdit: deck.access?.canEdit ?? state.canEdit,
      // A source range cannot survive a rewrite numerically, but the words the
      // author selected can survive logically. Relocate the same occurrence in
      // the new body so a tone click does not throw them out of Text controls
      // before they can add weight or typeface. A whole block survives by its
      // shared source order for the same reason direct-manipulation handles do.
      selection: {
        slide,
        ...(block === undefined ? {} : { block }),
        ...text,
      },
      canUndo: history.canUndo,
      canRedo: history.canRedo,
      refusal: undefined,
      problem: undefined,
      // The change has happened, so what was foreseen about it is now either
      // in `diagnostics` or was never true.
      foreseen: undefined,
      ...extra,
    });
  }

  /** The same selected words, rebased onto a newly returned source body. */
  function relocatedText(deck: DeckState, slide: number): Pick<Selection, "range" | "text"> {
    const { text, range } = state.selection;
    if (slide !== state.selection.slide || !text || !range) return {};

    const oldSpan = state.spans[slide]?.body;
    const nextSpan = deck.spans[slide]?.body;
    if (!oldSpan || !nextSpan) return {};

    const oldBody = sliceBytes(state.source, oldSpan.start, oldSpan.end);
    const nextBody = sliceBytes(deck.source, nextSpan.start, nextSpan.end);
    const occurrence = occurrencesBefore(oldBody, text.trim(), range.start);
    const found = locateSelection(nextBody, text, occurrence);

    return "problem" in found ? {} : { text: found.text, range: found.range };
  }

  /**
   * The selection that puts this editor where a followed seat is.
   *
   * Nothing at all when they are on the slide already, which is the common
   * case and the one worth being careful about: replacing the selection there
   * would take away the block the author has selected every time somebody else
   * clicked something on the same slide.
   */
  function alongside(followed: Viewer | undefined): Partial<EditorState> {
    return followed === undefined || followed.slide === state.selection.slide
      ? {}
      : { selection: { slide: followed.slide } };
  }

  async function attempt(
    work: () => Promise<void>,
    failed: Partial<EditorState> = {},
  ): Promise<void> {
    try {
      await work();
    } catch (error) {
      set({ problem: error instanceof Error ? error.message : String(error), ...failed });
    }
  }

  return {
    state: () => state,

    subscribe(listener) {
      listeners.add(listener);
      listener(state);
      return () => listeners.delete(listener);
    },

    open() {
      return attempt(async () => {
        history.clear();
        adopt(await client.deck());
      });
    },

    run(op, selection) {
      if (state.canEdit === false) return Promise.resolve();
      set({ writing: true, refusal: undefined, problem: undefined });
      return attempt(
        async () => {
          const answer = await client.apply(op);
          if (answer.error) {
            set({ refusal: answer.error, writing: false });
            return;
          }

          history.applied(answer.undo ?? []);
          adopt(
            answer,
            selection === undefined ? { writing: false } : { writing: false, selection },
          );
        },
        { writing: false },
      );
    },

    undo() {
      if (state.canEdit === false) return Promise.resolve();
      const edit = history.nextUndo();
      if (!edit) return Promise.resolve();

      set({ writing: true, refusal: undefined, problem: undefined });
      return attempt(
        async () => {
          const answer = await client.revert(edit);
          history.undone(answer.undo ?? []);
          adopt(answer, { writing: false });
        },
        { writing: false },
      );
    },

    redo() {
      if (state.canEdit === false) return Promise.resolve();
      const edit = history.nextRedo();
      if (!edit) return Promise.resolve();

      set({ writing: true, refusal: undefined, problem: undefined });
      return attempt(
        async () => {
          const answer = await client.revert(edit);
          history.redone(answer.undo ?? []);
          adopt(answer, { writing: false });
        },
        { writing: false },
      );
    },

    select(selection) {
      // Selecting anything is the author taking the wheel back. Following that
      // survived a deliberate click would drag the author off the slide they
      // just chose, at whatever moment somebody else happened to move.
      set({ selection: { ...state.selection, ...selection }, following: undefined });
    },

    foresee(findings) {
      // Nothing to nothing is not a change, and a gesture that clears these on
      // every pointer event would re-render the whole editor on every one.
      if (findings.length === 0 && (state.foreseen ?? []).length === 0) return;

      set({ foreseen: findings });
    },

    saw(viewers) {
      const followed = viewers.find((viewer) => viewer.id === state.following);

      // A seat that is no longer in the roster is a tab that was closed. There
      // is nothing left to follow, and an editor that kept following it would
      // simply stop moving with no way to tell why.
      if (state.following !== undefined && followed === undefined) {
        set({ viewers, following: undefined });
        return;
      }

      set({ viewers, ...alongside(followed) });
    },

    follow(seat) {
      const followed = state.viewers.find((viewer) => viewer.id === seat);

      set({
        following: followed === undefined ? undefined : seat,
        // Arriving where they are is the whole request, so it happens now
        // rather than at their next move — which might be minutes away.
        ...alongside(followed),
      });
    },

    bodyOf(slide) {
      const span = state.spans[slide]?.body;
      return span ? sliceBytes(state.source, span.start, span.end) : "";
    },

    contentOf(slide) {
      const span = state.spans[slide]?.content;
      return span ? sliceBytes(state.source, span.start, span.end) : "";
    },

    blocksOf: (slide) => state.spans[slide]?.blocks ?? [],
  };
}

/** Which raw-source appearance begins at the selected byte range. */
function occurrencesBefore(body: string, text: string, byte: number): number {
  if (text.length === 0) return 0;

  const prefix = sliceBytes(body, 0, byte);
  let count = 0;
  let cursor = prefix.indexOf(text);
  while (cursor !== -1) {
    count += 1;
    cursor = prefix.indexOf(text, cursor + 1);
  }
  return count;
}
