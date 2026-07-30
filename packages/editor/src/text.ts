/**
 * Turning "the author retyped this line" into the byte range it was written in.
 *
 * The canvas shows the deck's own page and an operation names bytes of Markdown,
 * so an edit made in place has to cross between the two. It crosses by matching
 * rather than by rendering: the pipeline says where every block and every mark
 * of the slide is — those spans arrive with the deck — and this walks the text a
 * reader sees against them, in order, until each run of it has an address.
 *
 * # Why the marker survives
 *
 * A heading is `#   One` in the file and `One` on the screen. The run that is
 * matched is `One`, so the range that gets spliced is `One`, and the hash and the
 * three spaces the author typed after it are never in an edit at all. The same
 * arithmetic covers `- ` in front of a list item, `> ` in front of a quote, and
 * the words inside `[120ms]{#latency}` — the twelve bytes a reader never sees are
 * outside every run, so nothing typed on the canvas can reach them.
 *
 * # Why it says no
 *
 * Where the rendered text is not written anywhere in the source — emphasis, a
 * link, a reference, a footnote — no run matches and the element is not made
 * editable at all. That is the same trade [`selection`](./selection) makes and
 * for the same reason: the alternative is a position map threaded out of the
 * renderer, which is a real feature and a much larger one, and a *guess* here
 * would splice the wrong bytes of somebody's talk.
 *
 * Nothing here composes Markdown. It produces a range and the plain words that
 * go in it, and `slidx_edit` decides what the file then says.
 */

import { byteLength, sliceBytes } from "./bytes";
import type { BlockSpans } from "./client";
import type { ByteSpan } from "./operations";

/**
 * The attribute the pipeline writes onto a compiled mark, from
 * `slidx_core::mark::MARK_ATTRIBUTE`.
 *
 * Named here so which words a mark holds is read off the page rather than
 * guessed from the source. A name spelled differently on this side would map a
 * mark's words as though they were prose, and typing in one would take its key
 * away — the failure the whole module exists to prevent.
 */
const MARK_ATTRIBUTE = "data-slidx-mark";

const TEXT_NODE = 3;

/** What changed between two versions of the same run of text. */
export interface Change {
  /** Where the change starts, counted in the text as it was. */
  from: number;
  /** Where it ends, counted the same way. */
  to: number;
  /** What takes its place. */
  text: string;
}

/** One run of rendered text, and the bytes of the body it is written in. */
export interface TextRun {
  text: string;
  /** Body-local, because that is what an operation's range is measured in. */
  source: ByteSpan;
}

/**
 * The text of one element on the page, addressed in the source.
 *
 * `plain` is what the element reads as, which is what an author edits and what a
 * change is measured against. `runs` covers it in order, and covers all of it —
 * an element with a run this could not place is not offered for editing, so a
 * plan that exists is one every offset of which has an address.
 */
export interface TextPlan {
  plain: string;
  runs: TextRun[];
}

/**
 * Tags whose text is one run of inline content, so editing one is editing a
 * line rather than a document.
 *
 * A block wrapper is deliberately not among them: `contenteditable` on a whole
 * block lets the browser split paragraphs and merge lists, and what it produces
 * is HTML rather than an edit to a line — which is exactly the rendered markup
 * nothing here can convert back.
 */
const EDITABLE = ["p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "th"];

/**
 * What disqualifies an element from being one line of text.
 *
 * A list item holding a nested list reads as both items at once, and a paragraph
 * holding an image has words that are an `alt` attribute rather than prose. A
 * fence is excluded outright: its content is whitespace-significant, and a
 * `contenteditable` has opinions about whitespace.
 *
 * Inline syntax is *not* on the list, and does not need to be. `**bold**` and
 * `[text](url)` put their words in the file exactly as they render, so the
 * forward search below finds them between the syntax and leaves the syntax
 * outside the run — which is how retyping a link's words keeps its URL.
 */
const NOT_A_LINE = "pre, ul, ol, table, blockquote, p, div, figure, img";

/** The elements of a block whose text is a line an author could retype. */
export function editableIn(block: Element): Element[] {
  return [...block.querySelectorAll(EDITABLE.join(","))].filter(
    (element) => element.querySelector(NOT_A_LINE) === null,
  );
}

/**
 * Where each of a block's editable lines is written.
 *
 * One walk for the whole block rather than one per line, because the lines are
 * matched against the source in order and the second `- ` item has to be found
 * after the first. An element missing from the answer is one that could not be
 * placed, and it is left alone.
 */
export function planBlock(
  body: string,
  block: BlockSpans,
  elements: readonly Element[],
): Map<Element, TextPlan> {
  const pieces = piecesOf(body, block);
  const plans = new Map<Element, TextPlan>();
  let cursor = { piece: 0, at: 0 };

  for (const element of elements) {
    const runs: TextRun[] = [];
    let placed = true;

    for (const run of runsIn(element)) {
      const found = place(pieces, cursor, run);
      if (found === undefined) {
        placed = false;
        break;
      }

      runs.push({ text: run.text, source: found.source });
      cursor = found.cursor;
    }

    if (placed && runs.length > 0) plans.set(element, { plain: element.textContent ?? "", runs });
  }

  return plans;
}

/** The byte range a change to an element's text asks for. */
export function rangeOf(plan: TextPlan, change: Change): ByteSpan | undefined {
  // An insertion is one position rather than two edges, and resolving it twice
  // would put its end before its start wherever the caret sits on a seam.
  const caret = change.from === change.to;
  const start = offsetIn(plan, change.from, caret ? "end" : "start");
  const end = offsetIn(plan, change.to, "end");
  if (start === undefined || end === undefined || end < start) return undefined;

  return { start, end };
}

/**
 * What an author changed, as one run rather than as a keystroke.
 *
 * The common prefix and suffix are dropped so that retyping a word in a
 * paragraph is a change to that word. `slidx_edit` narrows the splice again on
 * the other side; this narrowing is what keeps the *range* honest, so an editor
 * asking about a mark asks about the one the author was actually in.
 *
 * `undefined` when the two are the same, which is most blurs.
 */
export function changeBetween(was: string, now: string): Change | undefined {
  if (was === now) return undefined;

  let from = 0;
  while (from < was.length && from < now.length && was[from] === now[from]) from += 1;
  // Never between the halves of a surrogate pair: an emoji is one character an
  // author typed, and half of one is not text.
  if (from > 0 && isTrailing(was.charCodeAt(from))) from -= 1;

  let tail = 0;
  while (
    tail < was.length - from &&
    tail < now.length - from &&
    was[was.length - 1 - tail] === now[now.length - 1 - tail]
  ) {
    tail += 1;
  }
  if (tail > 0 && isTrailing(was.charCodeAt(was.length - tail))) tail -= 1;

  return { from, to: was.length - tail, text: now.slice(from, now.length - tail) };
}

/** One run of an element's text, and whether a mark wraps it. */
interface RenderedRun {
  text: string;
  /** True when the pipeline compiled a mark around it. */
  marked: boolean;
}

/**
 * The runs of text inside one element, in reading order.
 *
 * A mark compiles to a span carrying `data-slidx-mark`, so which words are
 * inside one is a question the page answers — the editor never looks for a
 * bracket in rendered HTML.
 */
export function runsIn(element: Element): RenderedRun[] {
  const runs: RenderedRun[] = [];

  const walk = (node: Node, marked: boolean) => {
    for (const child of node.childNodes) {
      if (child.nodeType === TEXT_NODE) {
        const text = child.textContent ?? "";
        if (text.length > 0) runs.push({ text, marked });
        continue;
      }

      const inside = marked || (child as Element).hasAttribute?.(MARK_ATTRIBUTE) === true;
      walk(child, inside);
    }
  };

  walk(element, false);
  return runs;
}

/** One stretch of a block's source, and whether a mark's words are in it. */
interface Piece {
  text: string;
  /** Body-local byte offset of the first character. */
  start: number;
  marked: boolean;
}

/**
 * A block's source, cut into the words a mark holds and the source between.
 *
 * Cut at the spans the pipeline reported rather than by looking for brackets:
 * one parser for the mark grammar, and it is not in the browser.
 */
function piecesOf(body: string, block: BlockSpans): Piece[] {
  const pieces: Piece[] = [];
  let cursor = block.span.start;

  for (const mark of block.marks ?? []) {
    pieces.push({ text: sliceBytes(body, cursor, mark.span.start), start: cursor, marked: false });
    pieces.push({
      text: sliceBytes(body, mark.words.start, mark.words.end),
      start: mark.words.start,
      marked: true,
    });
    cursor = mark.span.end;
  }

  pieces.push({ text: sliceBytes(body, cursor, block.span.end), start: cursor, marked: false });
  return pieces;
}

interface Cursor {
  piece: number;
  at: number;
}

/**
 * Where one rendered run is written, searching forward from the cursor.
 *
 * Forward only, so the second of two identical bullets is matched second. A
 * marked run has to *be* the next marked piece rather than appear inside it:
 * half a mark's words is not something a mark can hold.
 *
 * # Why the source in front of a run has to be syntax
 *
 * `##   ` in front of a heading, `- ` in front of a bullet, `**` in front of a
 * bold word: a run is preceded by the punctuation that produced it, and skipping
 * that punctuation is the whole trick. Skipping *words* is a different thing
 * entirely, and it is how a match goes wrong — the full stop ending
 * `[the docs](https://example.test/docs).` also appears inside the URL, and the
 * first occurrence is the wrong one. So a gap carrying a letter means this run is
 * not aligned with what the file says, and the line is left alone rather than
 * spliced at a guess.
 */
function place(
  pieces: readonly Piece[],
  cursor: Cursor,
  run: RenderedRun,
): { source: ByteSpan; cursor: Cursor } | undefined {
  for (let index = cursor.piece; index < pieces.length; index += 1) {
    const piece = pieces[index]!;
    const from = index === cursor.piece ? cursor.at : 0;

    if (piece.marked !== run.marked) {
      // A mark on the way to this run is a mark the page did not show — a take,
      // whose later states the pipeline lifts out of the content. Its words are
      // still in the file, so a run matched past it is matched past bytes an
      // edit would then be measured against.
      if (piece.marked || carriesWords(piece.text.slice(from))) return undefined;
      continue;
    }

    if (piece.marked) {
      return piece.text === run.text
        ? { source: bytesOf(piece, 0, run.text), cursor: { piece: index + 1, at: 0 } }
        : undefined;
    }

    // The first occurrence with nothing but syntax in front of it.
    let found = piece.text.indexOf(run.text, from);
    while (found !== -1 && carriesWords(piece.text.slice(from, found))) {
      found = piece.text.indexOf(run.text, found + 1);
    }
    if (found === -1) return undefined;

    return {
      source: bytesOf(piece, found, run.text),
      cursor: { piece: index, at: found + run.text.length },
    };
  }

  return undefined;
}

/**
 * True when a stretch of source holds a word rather than only punctuation.
 *
 * A letter and not a digit, because `1. ` in front of an ordered list item is
 * syntax and a numbered list is ordinary content.
 */
function carriesWords(text: string): boolean {
  return /\p{L}/u.test(text);
}

function bytesOf(piece: Piece, at: number, text: string): ByteSpan {
  const start = piece.start + byteLength(piece.text.slice(0, at));

  return { start, end: start + byteLength(text) };
}

/**
 * The byte offset one position in an element's text is at.
 *
 * A position on the seam between two runs belongs to whichever of them the
 * change is growing from: the end of a selection belongs to the run before it
 * and the start to the run after, so a word deleted at the front of a mark takes
 * bytes from the mark rather than from the prose in front of it.
 */
function offsetIn(plan: TextPlan, position: number, edge: "start" | "end"): number | undefined {
  let seen = 0;

  for (const run of plan.runs) {
    const inside =
      edge === "end" ? position <= seen + run.text.length : position < seen + run.text.length;

    if (inside && position >= seen) {
      return run.source.start + byteLength(run.text.slice(0, position - seen));
    }

    seen += run.text.length;
  }

  // Past the last run, which is where a caret at the end of a line is.
  const last = plan.runs[plan.runs.length - 1];
  return position === seen && last ? last.source.end : undefined;
}

/** True for the second half of a surrogate pair. */
function isTrailing(code: number): boolean {
  return code >= 0xdc00 && code <= 0xdfff;
}
