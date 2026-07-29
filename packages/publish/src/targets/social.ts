/**
 * The post that says the slides are up.
 *
 * A social post is the one target with a hard character budget, so it is the
 * one target that composes rather than maps. The composition rule is fixed and
 * worth stating plainly, because it is what the tests are about:
 *
 * **The link and the hashtag are never what gets cut.** A post that loses a
 * clause is a slightly worse post. A post that loses its URL is a post that did
 * not do the one thing it existed for, and a post that loses its hashtag is
 * invisible to everyone following the conference. So the budget is spent on
 * those first, then on the title, and the description gets whatever is left.
 *
 * There is no invented sentence around it — no "Slides are up!", no "Thanks
 * everyone!". The deck's own words are the post. Boilerplate here would be
 * English text inserted into the timeline of an author who wrote their deck in
 * Japanese, in slidx's voice rather than theirs.
 */

import {
  ask,
  source,
  type Composed,
  type SocialOptions,
  type SocialPost,
  type SourceInput,
} from "../boundary";

/**
 * The default budget.
 *
 * 280 is the shortest limit among the networks people announce talks on, so a
 * post composed for it fits everywhere without a per-network variant. Callers
 * with a longer budget pass one.
 */
export const DEFAULT_POST_LIMIT = 280;

export function composeSocial(
  input: SourceInput,
  options: SocialOptions = {},
): Composed<SocialPost> {
  return ask<Composed<SocialPost>>({ op: "composeSocial", source: source(input), options });
}

/** One line for a printed plan. */
export function describeSocial(post: SocialPost): string {
  return ask<string>({ op: "describeSocial", post });
}

export type { SocialOptions, SocialPost } from "../boundary";
