/**
 * The post that says the slides are up.
 *
 * A social post is the one target with a hard character budget, so it is the
 * one target that composes rather than maps. The composition rule is fixed and
 * worth stating plainly, because it is what the tests are about:
 *
 * **The link and the hashtag are never what gets cut.** A post that loses a
 * clause is a slightly worse post. A post that loses its URL is a post that
 * did not do the one thing it existed for, and a post that loses its hashtag
 * is invisible to everyone following the conference. So the budget is spent on
 * those first, then on the title, and the description gets whatever is left.
 *
 * There is no invented sentence around it — no "Slides are up!", no "Thanks
 * everyone!". The deck's own words are the post. Boilerplate here would be
 * English text inserted into the timeline of an author who wrote their deck in
 * Japanese, in slidx's voice rather than theirs.
 */

import { countCharacters, normalizeTag, truncate } from "../text";
import {
  artifactOf,
  blocked,
  composed,
  reason,
  type BlockedReason,
  type Composed,
  type DeckSource,
} from "../types";

/**
 * The default budget.
 *
 * 280 is the shortest limit among the networks people announce talks on, so a
 * post composed for it fits everywhere without a per-network variant. Callers
 * with a longer budget pass one.
 */
export const DEFAULT_POST_LIMIT = 280;

/**
 * Below this, a description is not worth the space.
 *
 * A four-word fragment ending in an ellipsis reads as a bug, and costs the
 * characters that made the title readable. Under the floor the description is
 * dropped whole.
 */
const DESCRIPTION_FLOOR = 24;

/** Blank line between the parts, in characters. */
const SEPARATOR = "\n\n";

export interface SocialPost {
  text: string;
  /** Characters, counted as the platform counts them. Never above `limit`. */
  length: number;
  limit: number;
  /** True when the description was shortened or dropped to fit. */
  truncated: boolean;
  /** Card image to attach, when the build produced one. */
  image?: string;
}

export interface SocialOptions {
  /** Character budget. Defaults to {@link DEFAULT_POST_LIMIT}. */
  limit?: number;
}

export function composeSocial(
  source: DeckSource,
  options: SocialOptions = {},
): Composed<SocialPost> {
  const limit = options.limit ?? DEFAULT_POST_LIMIT;
  const { meta } = source;
  const reasons: BlockedReason[] = [];

  const title = meta.title?.trim() ?? "";
  const url = meta.url?.trim() ?? "";

  if (title === "") {
    reasons.push(reason("title", "a post needs a title — add `title:` to the deck frontmatter"));
  }

  // The whole point of the post is the link. Emitting one without it would be
  // an announcement of slides nobody can reach.
  if (url === "") {
    reasons.push(
      reason(
        "url",
        "a post needs somewhere to send people — add `url:` with the published deck's address",
      ),
    );
  }

  if (reasons.length > 0) return blocked(...reasons);

  const hashtag = meta.hashtag === undefined ? "" : normalizeTag(meta.hashtag);
  const event = meta.event?.trim() ?? "";

  const lead = event === "" ? title : `${title} — ${event}`;
  const tail = hashtag === "" ? url : `${url} #${hashtag}`;

  const fixed = countCharacters(lead) + SEPARATOR.length + countCharacters(tail);

  if (fixed > limit) {
    return blocked(mandatoryPartsDoNotFit(lead, tail, limit));
  }

  const description = meta.description?.trim() ?? "";
  const available = limit - fixed - SEPARATOR.length;
  const body = available >= DESCRIPTION_FLOOR ? truncate(description, available) : "";
  const truncated = description !== "" && body !== description;

  const text = [lead, body, tail].filter((part) => part !== "").join(SEPARATOR);
  const image = artifactOf(source, "card");

  return composed({
    text,
    length: countCharacters(text),
    limit,
    truncated,
    ...(image === undefined ? {} : { image: image.path }),
  });
}

/**
 * Which field to name when even the mandatory parts overflow.
 *
 * The URL is not shortenable by the author in any useful sense, so when it
 * alone blows the budget the honest report is about the budget. Otherwise the
 * title is the part that can be edited, and saying so is more use than saying
 * the post is too long.
 */
function mandatoryPartsDoNotFit(lead: string, tail: string, limit: number): BlockedReason {
  const tailLength = countCharacters(tail);

  if (tailLength > limit) {
    return reason(
      "url",
      `the URL and hashtag need ${tailLength} characters of a ${limit}-character post — ` +
        "shorten the URL or raise the budget",
    );
  }

  return reason(
    "title",
    `the title, URL, and hashtag need ${countCharacters(lead) + SEPARATOR.length + tailLength} ` +
      `characters of a ${limit}-character post — shorten \`title\``,
  );
}

/** One line for a printed plan. */
export function describeSocial(post: SocialPost): string {
  const shortened = post.truncated ? ", description shortened" : "";
  return `compose a ${post.length}/${post.limit} character post${shortened}`;
}
