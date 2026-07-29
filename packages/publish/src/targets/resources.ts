/**
 * The page of links, so nobody has to squint at a URL on a projector.
 *
 * Every talk has the moment where a link appears on a slide and forty people
 * photograph it. The list already exists — it is scattered across the deck — so
 * the page is a collection job, not an authoring one, and collecting it by hand
 * afterwards is precisely the chore that does not get done.
 *
 * Order is the deck's order and the labels are the deck's words, because the
 * page is only useful if a reader can match an entry against the slide they
 * remember. Sorting it alphabetically would break that, so it is not sorted.
 */

import { ask, source, type Composed, type ResourcesPage, type SourceInput } from "../boundary";

export function composeResources(input: SourceInput): Composed<ResourcesPage> {
  return ask<Composed<ResourcesPage>>({ op: "composeResources", ...source(input) });
}

/** One line for a printed plan. */
export function describeResources(page: ResourcesPage): string {
  return ask<string>({ op: "describeResources", page });
}

export type { ResourcesPage } from "../boundary";
