// Publishing a deck, as it crosses out of Rust.
//
// Generated from the Rust types by `vp run generate:types`. Editing this file
// is pointless: `cargo test -p slidx_wasm` compares it against the types it
// came from, and the types win.

/**
 * A deck's metadata, as the author wrote it at proposal time.
 *
 * Nothing here is derived or defaulted. A field that is absent stays absent
 * all the way into the plan, where it becomes a named reason rather than a
 * guess.
 */
export type DeckMetadata = {
  title?: string;
  description?: string;
  author?: string;
  /**
   * Conference or meetup name.
   */
  event?: string;
  /**
   * ISO-8601 date, kept as text so a plan never depends on a clock.
   */
  date?: string;
  venue?: string;
  /**
   * Without the leading `#`, which is added back per platform.
   */
  hashtag?: string;
  /**
   * Canonical URL of the published deck.
   */
  url?: string;
  /**
   * The recording, once one exists.
   *
   * The only field here that is normally added weeks after the talk, which
   * is what the archive target is built around.
   */
  recording?: string;
  /**
   * Repository, listed on the resources page.
   */
  repo?: string;
  /**
   * Author-chosen tags. Never reordered, never rewritten.
   */
  tags?: Array<string>;
  /**
   * Explicit path segment for the upload targets.
   *
   * Present when the author has pinned one — usually because the derived
   * slug would change under them when the title is edited, and a slide URL
   * that moves after it has been shared is a broken link in someone's notes.
   */
  slug?: string;
};

/**
 * One slide, reduced to what publishing reads from it.
 */
export type DeckSlide = {
  /**
   * Zero-based position. Fixes the order of everything derived per slide.
   */
  index: number;
  title?: string;
  /**
   * Markdown body, as authored. Links are read out of it.
   */
  content?: string;
  /**
   * Speaker notes, in source order. The blog scaffold is made of these.
   */
  notes?: Array<string>;
};

/**
 * A file the build produced, offered to the targets that need one.
 */
export type Artifact = {
  kind: ArtifactKind;
  /**
   * Path as the build reported it. Never opened by this crate.
   */
  path: string;
  /**
   * Size in bytes, when the caller measured it. Checked against upload caps.
   *
   * Crosses as a `number` rather than the `bigint` a 64-bit integer would
   * otherwise become: what a caller has is `statSync(path).size`, and a
   * boundary that demanded `4194304n` would be a boundary nobody could hand
   * the answer they already had.
   */
  bytes?: number;
};

/**
 * The kinds of artifact a target asks for by name.
 *
 * An enum rather than a free string so a typo in a caller is a type error, not
 * a step that reports the PDF as missing on a build that produced one.
 */
export type ArtifactKind = "pdf" | "html" | "card" | "video";

/**
 * Everything the targets are composed from.
 */
export type DeckSource = {
  meta: DeckMetadata;
  slides: Array<DeckSlide>;
  artifacts: Array<Artifact>;
};

/**
 * Why a step cannot run, and what would fix it.
 *
 * `field` is the thing to add — a frontmatter key, or the build output that is
 * missing. Naming it is the whole point: "add `url:` to the frontmatter" is
 * actionable at 11pm after a talk, "social post unavailable" is not.
 */
export type BlockedReason = { field: string; message: string };

export type PlanOptions = {
  meta: DeckMetadata;
  /**
   * In any order; everything derived per slide is sorted by index.
   */
  slides: Array<DeckSlide>;
  /**
   * What the build produced. Absent is normal, and is reported per target.
   */
  artifacts: Array<Artifact>;
  /**
   * A subset to plan. Absent means all of [`PUBLISH_TARGETS`].
   */
  targets?: Array<PublishTarget>;
  social: SocialOptions;
};

export type PublishPlan = {
  /**
   * The deck's title, or a stand-in, for the plan's header line.
   */
  deck: string;
  steps: Array<PublishStep>;
};

/**
 * One destination.
 */
export type PublishTarget =
  | "speakerdeck"
  | "docswell"
  | "social"
  | "blog"
  | "resources"
  | "cloudflare"
  | "archive";

/**
 * A destination's own payload, which is what makes a ready step worth having.
 */
export type ReadyPayload =
  | SpeakerDeckUpload
  | DocswellUpload
  | SocialPost
  | BlogScaffold
  | ResourcesPage
  | CloudflarePages
  | ArchiveRecord;

/**
 * What an upload consists of.
 *
 * Field names are Speaker Deck's, not slidx's. The whole value of a typed
 * payload is that it can be handed to whatever performs the upload without a
 * second mapping step in between, where a renamed field goes missing.
 */
export type SpeakerDeckUpload = {
  title: string;
  description: string;
  /**
   * Path segment under the author's profile.
   */
  slug: string;
  tags: Array<string>;
  /**
   * Path to the built PDF. Never read by this crate.
   */
  pdf: string;
  /**
   * Talk date, shown on the deck page. ISO-8601, as authored.
   */
  date?: string;
};

export type DocswellUpload = {
  title: string;
  /**
   * Docswell's name for the description.
   */
  overview: string;
  /**
   * Path segment under the author's namespace.
   */
  path: string;
  tags: Array<string>;
  /**
   * Path to the built PDF. Never read by this crate.
   */
  file: string;
  /**
   * Where the talk was given, shown under the title.
   */
  presentedAt?: string;
};

export type SocialOptions = {
  /**
   * Character budget. Defaults to [`DEFAULT_POST_LIMIT`].
   */
  limit?: number;
};

export type SocialPost = {
  text: string;
  /**
   * Characters, counted as the platform counts them. Never above `limit`.
   */
  length: number;
  limit: number;
  /**
   * True when the description was shortened or dropped to fit.
   */
  truncated: boolean;
  /**
   * Card image to attach, when the build produced one.
   */
  image?: string;
};

export type BlogScaffold = {
  /**
   * Suggested file name, dated so drafts sort by talk.
   */
  path: string;
  title: string;
  sections: Array<BlogSection>;
  /**
   * The whole file, frontmatter included.
   */
  markdown: string;
};

/**
 * One slide's worth of draft.
 */
export type BlogSection = {
  heading: string;
  /**
   * The slide's notes, joined. Never edited.
   */
  body: string;
  /**
   * Slide the section came from, so an editor can jump back.
   */
  slide: number;
};

export type ResourcesPage = {
  /**
   * Heading of the page.
   */
  title: string;
  /**
   * Suggested file name.
   */
  path: string;
  /**
   * Deduplicated, in slide order.
   */
  links: Array<DeckLink>;
  markdown: string;
};

/**
 * What slidx writes, and the command the author still has to run.
 */
export type CloudflarePages = {
  /**
   * Pages project name. Alphanumeric and dashes only.
   */
  name: string;
  /**
   * Always [`PATH`].
   */
  path: string;
  /**
   * The file, comments included.
   */
  toml: string;
  /**
   * What the author runs after slidx writes the file. Never executed here.
   */
  command: string;
};

/**
 * A talk, as it will be remembered.
 */
export type ArchiveRecord = {
  /**
   * File name stem. Stable, so re-running overwrites rather than piles up.
   */
  slug: string;
  path: string;
  title: string;
  event?: string;
  /**
   * ISO-8601, as authored. Kept as text so ordering never needs a clock.
   */
  date?: string;
  venue?: string;
  author?: string;
  description?: string;
  /**
   * Where the slides ended up.
   */
  deck?: string;
  /**
   * The recording, once there is one.
   */
  recording?: string;
  repo?: string;
  tags: Array<string>;
  /**
   * What is not here *yet*, and when to come back for it.
   */
  pending: Array<BlockedReason>;
  markdown: string;
};

/**
 * A link, attributed to where it first appeared.
 */
export type DeckLink = {
  /**
   * As authored, minus trailing punctuation that belonged to the sentence.
   */
  url: string;
  /**
   * Link text where there was some, otherwise the URL without its scheme.
   */
  label: string;
  /**
   * Slide index, or null for a link that came from the frontmatter.
   */
  slide: number | null;
};

export type TalkIndex = {
  title: string;
  path: string;
  /**
   * Most recent first; undated last, in the order they were given.
   */
  talks: Array<ArchiveRecord>;
  /**
   * How many records still have no recording.
   *
   * The one number worth surfacing: it is a to-do list of conferences to
   * chase, and it is the only thing about an archive that changes after the
   * fact.
   */
  awaitingRecording: number;
  markdown: string;
};

export type TalkIndexOptions = {
  /**
   * Heading of the page.
   */
  title?: string;
  path?: string;
};

/**
 * What a caller is asking for.
 *
 * Reaches TypeScript as `PublishCall`, so the wrapper is type-checked against
 * the operations that exist rather than against a string it hopes is right.
 */
export type PublishCall =
  | ({ op: "plan" } & PlanOptions)
  | { op: "formatPlan"; plan: PublishPlan }
  | { op: "isReady"; plan: PublishPlan }
  | ({ op: "composeSpeakerDeck" } & DeckSource)
  | ({ op: "composeDocswell" } & DeckSource)
  | { op: "composeSocial"; source: DeckSource; options: SocialOptions }
  | ({ op: "composeBlog" } & DeckSource)
  | ({ op: "composeResources" } & DeckSource)
  | ({ op: "composeCloudflare" } & DeckSource)
  | ({ op: "composeArchive" } & DeckSource)
  | { op: "describeSpeakerDeck"; upload: SpeakerDeckUpload }
  | { op: "describeDocswell"; upload: DocswellUpload }
  | { op: "describeSocial"; post: SocialPost }
  | { op: "describeBlog"; scaffold: BlogScaffold }
  | { op: "describeResources"; page: ResourcesPage }
  | { op: "describeCloudflare"; pages: CloudflarePages }
  | { op: "describeArchive"; record: ArchiveRecord }
  | ({ op: "collectLinks" } & DeckSource)
  | { op: "buildTalkIndex"; records: Array<ArchiveRecord>; options: TalkIndexOptions }
  | { op: "countCharacters"; text: string }
  | { op: "truncate"; text: string; limit: number }
  | { op: "asciiSlug"; text: string }
  | { op: "fileSlug"; text: string }
  | { op: "fitSlug"; slug: string; limit: number }
  | { op: "normalizeTag"; tag: string }
  | { op: "uniqueTags"; tags: Array<string> }
  | { op: "tidyBlock"; text: string };

/**
 * A composed payload, or the reasons there is none.
 *
 * Targets answer with this rather than failing. A plan is most useful when the
 * deck is least ready, so one missing field must not stop the other
 * destinations from being planned and printed.
 */
export type Composed<T> = { ok: true; value: T } | { ok: false; reasons: BlockedReason[] };

/**
 * One step of a plan: everything a destination needs, or the fields that would
 * unblock it.
 *
 * `summary` is one line for a printed plan. A blocked step's summary names the
 * fields, because that is what the author acts on.
 */
export type PublishStep =
  | { status: "ready"; target: "speakerdeck"; summary: string; payload: SpeakerDeckUpload }
  | { status: "ready"; target: "docswell"; summary: string; payload: DocswellUpload }
  | { status: "ready"; target: "social"; summary: string; payload: SocialPost }
  | { status: "ready"; target: "blog"; summary: string; payload: BlogScaffold }
  | { status: "ready"; target: "resources"; summary: string; payload: ResourcesPage }
  | { status: "ready"; target: "cloudflare"; summary: string; payload: CloudflarePages }
  | { status: "ready"; target: "archive"; summary: string; payload: ArchiveRecord }
  | { status: "blocked"; target: PublishTarget; summary: string; reasons: BlockedReason[] };
