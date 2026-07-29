/**
 * Docswell, as a payload.
 *
 * The same deck, the same PDF, and deliberately not the same shape. Docswell
 * calls the blurb an overview, addresses a deck by a path with a *minimum*
 * length, and takes a shorter list of shorter tags than Speaker Deck does.
 *
 * Sharing one payload type between the two would mean one set of limits, which
 * would have to be the intersection — and an author would silently lose 3000
 * characters of a Speaker Deck description to a cap that belongs to the other
 * site. Two modules, two sets of numbers, each stated where its fields are.
 */

import { ask, source, type Composed, type DocswellUpload, type SourceInput } from "../boundary";

export function composeDocswell(input: SourceInput): Composed<DocswellUpload> {
  return ask<Composed<DocswellUpload>>({ op: "composeDocswell", ...source(input) });
}

/** One line for a printed plan. */
export function describeDocswell(upload: DocswellUpload): string {
  return ask<string>({ op: "describeDocswell", upload });
}

export type { DocswellUpload } from "../boundary";
