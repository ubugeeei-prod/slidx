/**
 * The destinations, and the one rule they share.
 *
 * A target turns deck metadata into one platform's shape. It composes, checks
 * that shape against that platform's documented caps, and returns either a
 * payload or the named reasons there is none. **No target performs anything.**
 * None of them opens a socket, reads a file, or takes a token, and the plan is
 * the boundary where that stops being an implementation detail and becomes a
 * property: a package that can post as you is a package that has to be trusted
 * with a credential, and this one never asks for one.
 *
 * The shared policy on limits, stated once and applied by every module here:
 * what the author wrote is passed through or reported, what slidx derived is
 * fitted. The social post is the documented exception — a character budget is
 * the entire premise of that target, so its description is cut to fit and the
 * payload says so.
 */

export { composeBlog, describeBlog } from "./targets/blog";
export type { BlogScaffold, BlogSection } from "./targets/blog";
export { composeDocswell, describeDocswell } from "./targets/docswell";
export type { DocswellUpload } from "./targets/docswell";
export { composeResources, describeResources } from "./targets/resources";
export type { ResourcesPage } from "./targets/resources";
export { composeSocial, DEFAULT_POST_LIMIT, describeSocial } from "./targets/social";
export type { SocialOptions, SocialPost } from "./targets/social";
export { composeSpeakerDeck, describeSpeakerDeck } from "./targets/speakerdeck";
export type { SpeakerDeckUpload } from "./targets/speakerdeck";
