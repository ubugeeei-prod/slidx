/**
 * The page of links, so nobody has to squint at a URL on a projector.
 *
 * Every talk has the moment where a link appears on a slide and forty people
 * photograph it. The list already exists — it is scattered across the deck —
 * so the page is a collection job, not an authoring one, and collecting it by
 * hand afterwards is precisely the chore that does not get done.
 *
 * Order is the deck's order and the labels are the deck's words, because the
 * page is only useful if a reader can match an entry against the slide they
 * remember. Sorting it alphabetically would break that, so it is not sorted.
 */

import { collectLinks, type DeckLink } from "../links";
import { blocked, composed, reason, type Composed, type DeckSource } from "../types";

export interface ResourcesPage {
  /** Heading of the page. */
  title: string;
  /** Suggested file name. */
  path: string;
  /** Deduplicated, in slide order. */
  links: DeckLink[];
  markdown: string;
}

export function composeResources(source: DeckSource): Composed<ResourcesPage> {
  const links = collectLinks(source);

  if (links.length === 0) {
    return blocked(
      reason(
        "links",
        "no link appears anywhere in the deck — add `repo:` to the frontmatter, or link " +
          "something from a slide",
      ),
    );
  }

  // The only target that needs nothing from the frontmatter. A deck that
  // filled in none of it still has links in it, and this page is still worth
  // producing, so the heading falls back rather than blocking.
  const deckTitle = source.meta.title?.trim() ?? "";
  const title = deckTitle === "" ? "Resources" : `Resources — ${deckTitle}`;

  const items = links.map((link) => `- [${escapeLabel(link.label)}](${link.url})`);

  return composed({
    title,
    path: "resources.md",
    links,
    markdown: `# ${title}\n\n${items.join("\n")}\n`,
  });
}

/**
 * Brackets in link text break the link they are in.
 *
 * A label taken from a slide can contain anything the author typed, and a
 * page of resources whose third entry swallowed the fourth is worse than an
 * escaped bracket.
 */
function escapeLabel(label: string): string {
  return label.replace(/([[\]])/g, "\\$1");
}

/** One line for a printed plan. */
export function describeResources(page: ResourcesPage): string {
  return `write ${page.path} with ${page.links.length} link(s)`;
}
