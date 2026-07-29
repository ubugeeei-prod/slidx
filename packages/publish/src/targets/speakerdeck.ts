/**
 * Speaker Deck, as a payload.
 *
 * Speaker Deck is a PDF host: the deck is the file, and everything else is the
 * page around it. That makes the failure mode specific — the upload is the
 * slowest step in publishing, and a title two characters over the cap fails
 * *after* the file has gone up.
 *
 * The numbers below are the platform's documented limits, read conservatively
 * on purpose. Being ten characters under costs nothing; being one over costs a
 * re-upload at the end of a long day.
 */

import { requiredText, requireArtifact, optionalText, resolveSlug, resolveTags } from "../fields";
import { blocked, composed, type BlockedReason, type Composed, type DeckSource } from "../types";

const PLATFORM = "Speaker Deck";

const TITLE_LIMIT = 100;
const DESCRIPTION_LIMIT = 4000;
const SLUG_LIMIT = 100;
const TAG_COUNT_LIMIT = 20;
const TAG_LENGTH_LIMIT = 30;
const PDF_BYTES_LIMIT = 100 * 1024 * 1024;

/**
 * What an upload consists of.
 *
 * Field names are Speaker Deck's, not slidx's. The whole value of a typed
 * payload is that it can be handed to whatever performs the upload without a
 * second mapping step in between, where a renamed field goes missing.
 */
export interface SpeakerDeckUpload {
  title: string;
  description: string;
  /** Path segment under the author's profile. */
  slug: string;
  tags: string[];
  /** Path to the built PDF. Never read by this package. */
  pdf: string;
  /** Talk date, shown on the deck page. ISO-8601, as authored. */
  date?: string;
}

export function composeSpeakerDeck(source: DeckSource): Composed<SpeakerDeckUpload> {
  const reasons: BlockedReason[] = [];
  const { meta } = source;

  const title = requiredText(
    meta.title,
    { name: "title", limit: TITLE_LIMIT, platform: PLATFORM },
    reasons,
  );
  const description = optionalText(
    meta.description,
    { name: "description", limit: DESCRIPTION_LIMIT, platform: PLATFORM },
    reasons,
  );
  const slug = resolveSlug(meta, { limit: SLUG_LIMIT, platform: PLATFORM }, reasons);
  const tags = resolveTags(
    meta,
    { count: TAG_COUNT_LIMIT, length: TAG_LENGTH_LIMIT, platform: PLATFORM },
    reasons,
  );
  const pdf = requireArtifact(
    source,
    "pdf",
    {
      byteLimit: PDF_BYTES_LIMIT,
      platform: PLATFORM,
      howToBuild: "set `pdf: true` in the slidx plugin options and build again",
    },
    reasons,
  );

  if (reasons.length > 0) return blocked(...reasons);

  const date = meta.date?.trim();

  return composed({
    title,
    description,
    slug,
    tags,
    pdf,
    ...(date !== undefined && date !== "" ? { date } : {}),
  });
}

/** One line for a printed plan. */
export function describeSpeakerDeck(upload: SpeakerDeckUpload): string {
  return `upload ${upload.pdf} as "${upload.title}" (/${upload.slug}), ${upload.tags.length} tag(s)`;
}
