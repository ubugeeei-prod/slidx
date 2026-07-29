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

import { optionalText, requireArtifact, requiredText, resolveSlug, resolveTags } from "../fields";
import { blocked, composed, type BlockedReason, type Composed, type DeckSource } from "../types";

const PLATFORM = "Docswell";

const TITLE_LIMIT = 100;
const OVERVIEW_LIMIT = 1000;
const PATH_LIMIT = 50;
/** Docswell will not address a deck by one or two characters. */
const PATH_MINIMUM = 3;
const TAG_COUNT_LIMIT = 10;
const TAG_LENGTH_LIMIT = 20;
const FILE_BYTES_LIMIT = 100 * 1024 * 1024;

export interface DocswellUpload {
  title: string;
  /** Docswell's name for the description. */
  overview: string;
  /** Path segment under the author's namespace. */
  path: string;
  tags: string[];
  /** Path to the built PDF. Never read by this package. */
  file: string;
  /** Where the talk was given, shown under the title. */
  presentedAt?: string;
}

export function composeDocswell(source: DeckSource): Composed<DocswellUpload> {
  const reasons: BlockedReason[] = [];
  const { meta } = source;

  const title = requiredText(
    meta.title,
    { name: "title", limit: TITLE_LIMIT, platform: PLATFORM },
    reasons,
  );
  const overview = optionalText(
    meta.description,
    { name: "description", limit: OVERVIEW_LIMIT, platform: PLATFORM },
    reasons,
  );
  const path = resolveSlug(
    meta,
    { limit: PATH_LIMIT, minimum: PATH_MINIMUM, platform: PLATFORM },
    reasons,
  );
  const tags = resolveTags(
    meta,
    { count: TAG_COUNT_LIMIT, length: TAG_LENGTH_LIMIT, platform: PLATFORM },
    reasons,
  );
  const file = requireArtifact(
    source,
    "pdf",
    {
      byteLimit: FILE_BYTES_LIMIT,
      platform: PLATFORM,
      howToBuild: "set `pdf: true` in the slidx plugin options and build again",
    },
    reasons,
  );

  if (reasons.length > 0) return blocked(...reasons);

  const presentedAt = meta.event?.trim();

  return composed({
    title,
    overview,
    path,
    tags,
    file,
    ...(presentedAt !== undefined && presentedAt !== "" ? { presentedAt } : {}),
  });
}

/** One line for a printed plan. */
export function describeDocswell(upload: DocswellUpload): string {
  return `upload ${upload.file} as "${upload.title}" (/${upload.path}), ${upload.tags.length} tag(s)`;
}
