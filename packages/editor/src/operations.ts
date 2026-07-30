/**
 * The operation set, as TypeScript sees it.
 *
 * This is the only thing the editor is allowed to build. Every change to a deck
 * is one of these values, posted to the dev server and turned into a byte-range
 * splice by `slidx_edit`. Nothing in this package ever composes Markdown: if a
 * gesture cannot be expressed here, the answer is a new operation in Rust, not
 * a string built in a browser. The moment there are two writers, the promise
 * that the canvas and the file are one document is gone.
 *
 * The shapes mirror `crates/slidx_edit/src/op.rs`, which is the definition.
 */

/** A slide, by position or by the slug in its URL. */
export type SlideRef = number | string;

/** A mark, by position in source order or by its `#key`. */
export type MarkRef = number | string;

/**
 * A block, by position in source order or by its `#key`.
 *
 * The renderer writes the position onto every block as `data-slidx-block`, so a
 * drag reads the number off the page rather than counting the Markdown for
 * itself. It is stable for exactly one operation.
 */
export type BlockRef = number | string;

/** A half-open byte range. Bytes, not characters — a selection has to survive CJK. */
export interface ByteSpan {
  start: number;
  end: number;
}

/**
 * What a block's attribute line carries.
 *
 * The same three things a mark's group does, because it is the same group on a
 * line of its own. Mirrors `slidx_core::Attributes`.
 */
export interface BlockAttributes {
  key?: string | undefined;
  classes?: string[] | undefined;
  properties?: Record<string, string> | undefined;
}

/** A mark without its text, which is whatever the author selected. */
export interface MarkAttributes {
  /**
   * Written `| undefined` because the inspector edits these as a patch: an
   * author removing a mark's key is saying it has none, which a missing field
   * cannot express.
   */
  key?: string | undefined;
  classes?: string[] | undefined;
  properties?: Record<string, string> | undefined;
}

/**
 * How long an action takes and what it looks like.
 *
 * Every field is optional and the theme decides what an absent one means, which
 * is the point: motion belongs to the theme, so a timeline writes *what* happens
 * and only overrides *how* when an author asked for something specific.
 */
export interface StepOptions {
  /** Milliseconds after the stop before this plays, instead of on a press. */
  after?: number | undefined;
  preset?: string | undefined;
  duration?: number | undefined;
  easing?: string | undefined;
  origin?: string | undefined;
}

/**
 * One authored intent on a slide's timeline.
 *
 * The four kinds the deck model has, spelled the way `StepAction` serialises in
 * Rust. `set` carries a patch because a change to something already on screen has
 * to say what it became; the other three name a target and stop there.
 */
export type StepAction =
  | { reveal: { target: string; options: StepOptions } }
  | { hide: { target: string; options: StepOptions } }
  | { emphasize: { target: string; options: StepOptions } }
  | {
      set: {
        target: string;
        patch: { content?: string | undefined; properties?: Record<string, string> | undefined };
        options: StepOptions;
      };
    };

/** One change to a deck source. */
export type EditOp =
  | { op: "setBody"; slide: SlideRef; body: string }
  | { op: "setHeading"; slide: SlideRef; text: string }
  /**
   * Typing on the canvas: a run of the slide's body, and the words for it.
   *
   * `range` is measured in the source body, the same as a mark's, because that
   * is what a caret in a rendered slide maps onto — `text.ts` does the mapping
   * and `slidx_edit` decides what the bytes become. A mark the range reaches is
   * not collateral: its `#key` survives, which is what makes the same gesture
   * safe on a phrase a step animates.
   */
  | { op: "setText"; slide: SlideRef; range: ByteSpan; text: string }
  | { op: "insertSlide"; at: number; body: string }
  | { op: "removeSlide"; slide: SlideRef }
  | { op: "moveSlide"; slide: SlideRef; to: number }
  | { op: "setField"; slide: SlideRef; key: string; value: unknown }
  | { op: "addMark"; slide: SlideRef; range: ByteSpan; attributes: MarkAttributes }
  | { op: "setMark"; slide: SlideRef; mark: MarkRef; attributes: MarkAttributes }
  | { op: "removeMark"; slide: SlideRef; mark: MarkRef }
  | { op: "setBlockAttributes"; slide: SlideRef; block: BlockRef; attributes: BlockAttributes }
  /**
   * How much of its region a block takes, as a share the theme names.
   *
   * Never a length. The reasoning is in `slidx_theme::layout::width`: a pixel is
   * unreviewable in a diff, means something else at another aspect ratio, and is
   * opaque to the rule that has to say whether the content still fits. `full` is
   * written by removing the property, so a resize out and back is byte-identical.
   */
  | { op: "setBlockWidth"; slide: SlideRef; block: BlockRef; width: string }
  /**
   * Where a block is on the slide, which is a region and a position in it.
   *
   * Both in one operation because a drag is one gesture: an editor that wrote
   * them separately would make its primary gesture cost two presses of undo.
   * `region` absent leaves the block's placement alone.
   */
  | { op: "moveBlock"; slide: SlideRef; block: BlockRef; to: number; region?: string }
  | { op: "addStep"; slide: SlideRef; at?: number; action: StepAction }
  | { op: "removeStep"; slide: SlideRef; index: number }
  | { op: "moveStep"; slide: SlideRef; from: number; to: number }
  | { op: "setStep"; slide: SlideRef; index: number; action: StepAction }
  | { op: "adoptSteps"; slide: SlideRef }
  | { op: "setNotes"; slide: SlideRef; notes: string };

/**
 * The splices that take an edit back.
 *
 * Opaque: an undo stack holds these and hands them back untouched. Reading one
 * in the browser would be the beginning of a second writer.
 */
export type Edit = readonly unknown[];

/** What an operation named that the deck does not have. */
export interface EditRefusal {
  error: string;
  [detail: string]: unknown;
}
