/**
 * Every link a deck mentions, in the order the audience met it.
 *
 * The resources page is built from this, and the thing that makes such a page
 * worth generating is that it is exhaustive: a link an author read out loud
 * from a slide and never wrote down anywhere else is exactly the one an
 * attendee comes looking for afterwards.
 *
 * What a link looks like in Markdown is `slidx_publish::links::scan`'s
 * business, and that is the only place in the workspace which answers the
 * question for publishing. Code is excluded there, for the same reason
 * `scanner.rs` excludes it from slide separators: a URL inside a fenced block
 * is usually an example endpoint, and listing one as a resource sends people
 * somewhere that does not exist.
 */

import { ask, source, type DeckLink, type SourceInput } from "./boundary";

/**
 * Deck links, deduplicated, in slide order.
 *
 * The repository comes first because it belongs to the talk rather than to any
 * one slide. The deck's own canonical url is deliberately absent — a resources
 * page that links to the page it is part of is a loop, not a resource.
 */
export function collectLinks(input: SourceInput): DeckLink[] {
  return ask<DeckLink[]>({ op: "collectLinks", ...source(input) });
}

export type { DeckLink } from "./boundary";
