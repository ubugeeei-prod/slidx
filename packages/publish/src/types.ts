/**
 * What a target is given, and what it may answer.
 *
 * The input types mirror `DeckMeta`, `TalkMeta`, and `Slide` in `slidx_core`,
 * flattened. Flattening is deliberate: a target cares whether it has a `url`,
 * not whether the url lives under `talk`, and a plain object is what a CLI, a
 * plugin, or a hand-written script can all produce without loading the parser.
 *
 * Every field is optional except the ones that make a deck a deck, because
 * absence is the normal case this package exists to report on. A deck for an
 * internal brown bag has no event, no hashtag, and no url, and must still
 * plan — with the steps that need those fields reported as blocked rather than
 * quietly emitting a post that links nowhere.
 */

/**
 * A deck's metadata, as the author wrote it at proposal time.
 *
 * Nothing here is derived or defaulted. A field that is absent stays absent
 * all the way into the plan, where it becomes a named reason rather than a
 * guess.
 */
export interface DeckMetadata {
  title?: string;
  description?: string;
  author?: string;
  /** Conference or meetup name. */
  event?: string;
  /** ISO-8601 date, kept as text so a plan never depends on a clock. */
  date?: string;
  venue?: string;
  /** Without the leading `#`, which is added back per platform. */
  hashtag?: string;
  /** Canonical URL of the published deck. */
  url?: string;
  /** Repository, listed on the resources page. */
  repo?: string;
  /** Author-chosen tags. Never reordered, never rewritten. */
  tags?: string[];
  /**
   * Explicit path segment for the upload targets.
   *
   * Present when the author has pinned one — usually because the derived slug
   * would change under them when the title is edited, and a slide URL that
   * moves after it has been shared is a broken link in someone's notes.
   */
  slug?: string;
}

/** One slide, reduced to what publishing reads from it. */
export interface DeckSlide {
  /** Zero-based position. Fixes the order of everything derived per slide. */
  index: number;
  title?: string;
  /** Markdown body, as authored. Links are read out of it. */
  content?: string;
  /** Speaker notes, in source order. The blog scaffold is made of these. */
  notes?: string[];
}

/** A file the build produced, offered to the targets that need one. */
export interface Artifact {
  kind: ArtifactKind;
  /** Path as the build reported it. Never opened by this package. */
  path: string;
  /** Size in bytes, when the caller measured it. Checked against upload caps. */
  bytes?: number;
}

/**
 * The kinds of artifact a target asks for by name.
 *
 * A union rather than a free string so a typo in a caller is a type error,
 * not a step that reports the PDF as missing on a build that produced one.
 */
export type ArtifactKind = "pdf" | "html" | "card" | "video";

/**
 * Why a step cannot run, and what would fix it.
 *
 * `field` is the thing to add — a frontmatter key, or the build output that is
 * missing. Naming it is the whole point: "add `url:` to the frontmatter" is
 * actionable at 11pm after a talk, "social post unavailable" is not.
 */
export interface BlockedReason {
  field: string;
  message: string;
}

/**
 * A composed payload, or the reasons there is none.
 *
 * Targets return this rather than throwing. A plan is most useful when the
 * deck is least ready, so one missing field must not stop the other four
 * targets from being planned and printed.
 */
export type Composed<T> = { ok: true; value: T } | { ok: false; reasons: BlockedReason[] };

/** Everything the targets are composed from. */
export interface DeckSource {
  meta: DeckMetadata;
  slides: DeckSlide[];
  artifacts: Artifact[];
}

export function blocked(...reasons: BlockedReason[]): { ok: false; reasons: BlockedReason[] } {
  return { ok: false, reasons };
}

export function composed<T>(value: T): { ok: true; value: T } {
  return { ok: true, value };
}

/** Names a missing or unusable field, and the fix. */
export function reason(field: string, message: string): BlockedReason {
  return { field, message };
}

/** The first artifact of a kind, or undefined. Ordering is the caller's. */
export function artifactOf(source: DeckSource, kind: ArtifactKind): Artifact | undefined {
  return source.artifacts.find((artifact) => artifact.kind === kind);
}
