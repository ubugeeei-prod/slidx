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

/** A half-open byte range. Bytes, not characters — a selection has to survive CJK. */
export interface ByteSpan {
  start: number;
  end: number;
}

/** A mark without its text, which is whatever the author selected. */
export interface MarkAttributes {
  key?: string;
  classes?: string[];
  properties?: Record<string, string>;
}

/** One authored intent on a slide's timeline. */
export type StepAction =
  | { reveal: { target: string; options?: Record<string, unknown> } }
  | { hide: { target: string; options?: Record<string, unknown> } }
  | { emphasize: { target: string; options?: Record<string, unknown> } };

/** One change to a deck source. */
export type EditOp =
  | { op: "setBody"; slide: SlideRef; body: string }
  | { op: "setHeading"; slide: SlideRef; text: string }
  | { op: "insertSlide"; at: number; body: string }
  | { op: "removeSlide"; slide: SlideRef }
  | { op: "moveSlide"; slide: SlideRef; to: number }
  | { op: "setField"; slide: SlideRef; key: string; value: unknown }
  | { op: "addMark"; slide: SlideRef; range: ByteSpan; attributes: MarkAttributes }
  | { op: "setMark"; slide: SlideRef; mark: MarkRef; attributes: MarkAttributes }
  | { op: "removeMark"; slide: SlideRef; mark: MarkRef }
  | { op: "addStep"; slide: SlideRef; action: StepAction }
  | { op: "removeStep"; slide: SlideRef; index: number }
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
