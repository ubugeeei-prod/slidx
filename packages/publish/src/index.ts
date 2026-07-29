/**
 * Publishing a deck, as a plan rather than an act.
 *
 * The chore after a talk is not one job, it is five: the PDF onto two slide
 * hosts, a post that links to it, a write-up nobody starts, and the page of
 * links the audience photographed off the screen. All five are already
 * described by the frontmatter written at proposal time — the title, the
 * event, the hashtag, the url — so none of them should need typing twice.
 *
 * **This package makes no network calls and takes no credentials.** It turns a
 * deck into typed payloads and returns them; something else — a CLI, a CI job,
 * the author with a browser open — performs them. That is a deliberate
 * boundary, not an unfinished one. A package that can post as you is a package
 * that has to be trusted with a token, and every token stored is a token that
 * can leak, be committed, or be used by a dependency you did not audit. There
 * is no HTTP client here to make it easy to cross that line later.
 *
 * What is left is a pure function from deck to plan, which has the pleasant
 * side effect of being trivially testable and diffable: the same deck plans
 * the same way every time, so `slidx publish --plan` is something you can read
 * before you mean it, and compare against what you did last time.
 */

export {
  blockedSteps,
  formatPlan,
  isReady,
  planPublish,
  PUBLISH_TARGETS,
  readySteps,
} from "./plan";
export type {
  BlockedStep,
  PlanOptions,
  PublishPlan,
  PublishStep,
  PublishTarget,
  ReadyStep,
} from "./plan";

export {
  composeArchive,
  composeBlog,
  composeDocswell,
  composeResources,
  composeSocial,
  composeSpeakerDeck,
  DEFAULT_POST_LIMIT,
} from "./targets";
export type {
  ArchiveRecord,
  BlogScaffold,
  BlogSection,
  DocswellUpload,
  ResourcesPage,
  SocialOptions,
  SocialPost,
  SpeakerDeckUpload,
} from "./targets";

export { buildTalkIndex } from "./talks";
export type { TalkIndex, TalkIndexOptions } from "./talks";

export { collectLinks } from "./links";
export type { DeckLink } from "./links";

export type {
  Artifact,
  ArtifactKind,
  BlockedReason,
  Composed,
  DeckMetadata,
  DeckSlide,
  DeckSource,
} from "./types";
