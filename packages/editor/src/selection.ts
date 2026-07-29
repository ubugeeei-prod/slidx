/**
 * Turning "these three words on screen" into a byte range in the file.
 *
 * This is the one place the editor has to bridge two representations, because
 * the canvas shows rendered HTML and an operation names bytes of Markdown. It
 * bridges them by *searching* rather than by mapping: the selected text is
 * looked up in the slide's source body, and the occurrence the author picked on
 * screen picks the occurrence in the file.
 *
 * That is honest about its limits and says so out loud. Where a phrase is
 * written differently in the source than it renders — a link, a mark that is
 * already there, a hard-wrapped line — the search finds nothing and the
 * inspector says the selection cannot be addressed rather than guessing. The
 * alternative, a position map threaded from the renderer, is a real feature and
 * a much larger one; this covers the case that matters now, which is prose.
 */

import { byteLength } from "./bytes";
import type { ByteSpan } from "./operations";

/** Why a selection could not be turned into a range. */
export type SelectionProblem = "empty" | "not-found";

export type LocatedSelection = { range: ByteSpan; text: string } | { problem: SelectionProblem };

/**
 * Where a selection is in a slide's source body.
 *
 * `occurrence` is which appearance of the text was selected on screen, counted
 * from zero. A phrase that appears once needs no help; one that appears three
 * times needs to know which.
 */
export function locateSelection(body: string, text: string, occurrence = 0): LocatedSelection {
  const selected = text.trim();
  if (selected.length === 0) return { problem: "empty" };

  const found = occurrenceIndex(body, selected, occurrence);
  if (found === -1) return { problem: "not-found" };

  const start = byteLength(body.slice(0, found));

  return { range: { start, end: start + byteLength(selected) }, text: selected };
}

/**
 * Which appearance of `text` was selected, counted in the rendered slide.
 *
 * The rendered text and the source differ, but they agree on how many times a
 * phrase of prose appears, which is all this has to be right about.
 */
export function occurrenceInRendered(rendered: string, text: string, at: number): number {
  const selected = text.trim();
  if (selected.length === 0) return 0;

  let count = 0;
  let cursor = rendered.indexOf(selected);

  while (cursor !== -1 && cursor < at) {
    count += 1;
    cursor = rendered.indexOf(selected, cursor + 1);
  }

  return count;
}

/** The character index of the nth appearance, or -1 when there is no nth. */
function occurrenceIndex(haystack: string, needle: string, nth: number): number {
  let found = haystack.indexOf(needle);

  for (let seen = 0; seen < nth && found !== -1; seen += 1) {
    found = haystack.indexOf(needle, found + 1);
  }

  // A phrase selected on screen for the third time but written twice in the
  // source is a phrase the source spells differently. Fall back to the first
  // appearance rather than to nothing, which is what an author means when a
  // mark is already wrapping one of them.
  return found === -1 ? haystack.indexOf(needle) : found;
}
