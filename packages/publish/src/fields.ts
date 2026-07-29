/**
 * Turning a deck's metadata and build output into one platform's fields, under
 * that platform's caps.
 *
 * The caps themselves are not here. Each target declares its own, next to the
 * payload they constrain, because a limit stated anywhere other than where the
 * field is documented is a limit that drifts. What lives here is the policy
 * every target shares, which is one sentence:
 *
 * **What the author wrote is passed through or reported; what slidx derived is
 * fitted.**
 *
 * So a 120-character title blocks the step and names `title` — shortening it
 * would publish a sentence the author did not write. A derived slug that is
 * too long is cut on a hyphen, and a suggested tag that does not fit in what
 * is left is dropped, because neither was ever asked for.
 *
 * Reasons accumulate rather than short-circuit. An author fixing a deck at
 * 11pm should learn about all three missing fields at once, not discover the
 * second one after fixing the first.
 */

import { asciiSlug, countCharacters, fitSlug, normalizeTag, uniqueTags } from "./text";
import {
  artifactOf,
  reason,
  type ArtifactKind,
  type BlockedReason,
  type DeckMetadata,
  type DeckSource,
} from "./types";

/** An upload's file, as a platform documents what it accepts. */
export interface FileField {
  /** Largest upload the platform takes, in bytes. */
  byteLimit: number;
  platform: string;
  /** How the build is told to produce it, named in the message. */
  howToBuild: string;
}

/**
 * The path of a built artifact, if there is one within the size cap.
 *
 * The file is never opened. A size the caller measured is checked because an
 * upload rejected for being 4MB over is a failure discovered at the end of the
 * slowest step in the process; a size the caller did not measure is not
 * guessed at, because reading the file would make planning an IO operation.
 */
export function requireArtifact(
  source: DeckSource,
  kind: ArtifactKind,
  file: FileField,
  reasons: BlockedReason[],
): string {
  const artifact = artifactOf(source, kind);

  if (artifact === undefined) {
    reasons.push(reason(kind, `${file.platform} needs the built ${kind} — ${file.howToBuild}`));
    return "";
  }

  if (artifact.bytes !== undefined && artifact.bytes > file.byteLimit) {
    reasons.push(
      reason(
        kind,
        `${artifact.path} is ${megabytes(artifact.bytes)}MB; ${file.platform} accepts ` +
          `${megabytes(file.byteLimit)}MB — compress the images or split the deck`,
      ),
    );
  }

  return artifact.path;
}

/** One decimal place, which is the precision a person acts on. */
function megabytes(bytes: number): string {
  return (Math.round((bytes / (1024 * 1024)) * 10) / 10).toString();
}

/** One text field, as a platform documents it. */
export interface TextField {
  /** Frontmatter key, so a reason can name the fix. */
  name: string;
  /** Maximum characters the platform accepts. */
  limit: number;
  /** Named in the message, because the same field has different caps. */
  platform: string;
}

/**
 * A field the platform will not accept an upload without.
 *
 * Returns an empty string alongside a recorded reason, so a caller can keep
 * collecting the rest of the problems instead of unwinding on the first.
 */
export function requiredText(
  text: string | undefined,
  field: TextField,
  reasons: BlockedReason[],
): string {
  const trimmed = text?.trim() ?? "";

  if (trimmed === "") {
    reasons.push(
      reason(
        field.name,
        `${field.platform} needs a ${field.name} — add \`${field.name}:\` to the deck frontmatter`,
      ),
    );
    return "";
  }

  return withinLimit(trimmed, field, reasons);
}

/** A field the platform accepts empty. Still capped when present. */
export function optionalText(
  text: string | undefined,
  field: TextField,
  reasons: BlockedReason[],
): string {
  const trimmed = text?.trim() ?? "";
  return trimmed === "" ? "" : withinLimit(trimmed, field, reasons);
}

function withinLimit(text: string, field: TextField, reasons: BlockedReason[]): string {
  const length = countCharacters(text);

  if (length > field.limit) {
    reasons.push(
      reason(
        field.name,
        `${field.name} is ${length} characters; ${field.platform} accepts ${field.limit} — shorten it`,
      ),
    );
  }

  return text;
}

/** Tag rules, as a platform documents them. */
export interface TagField {
  count: number;
  length: number;
  platform: string;
}

/**
 * The author's tags, plus the ones the talk itself implies.
 *
 * The hashtag and the event are the two tags a conference deck always wants
 * and no author remembers to write twice. They are appended only while there
 * is room under the platform's cap: they are slidx's suggestion, so they yield
 * to anything the author chose, and to the cap itself.
 *
 * Too many *authored* tags is a different thing entirely, and blocks. Dropping
 * the tail of a list someone wrote by hand publishes a deck tagged with what
 * happened to sort first.
 */
export function resolveTags(
  meta: DeckMetadata,
  field: TagField,
  reasons: BlockedReason[],
): string[] {
  const authored = uniqueTags(meta.tags ?? []);
  const overlong = authored.filter((tag) => countCharacters(tag) > field.length);

  if (overlong.length > 0) {
    reasons.push(
      reason(
        "tags",
        `tag \`${overlong[0] ?? ""}\` is longer than the ${field.length} characters ` +
          `${field.platform} allows — shorten it`,
      ),
    );
  }

  if (authored.length > field.count) {
    reasons.push(
      reason(
        "tags",
        `the deck has ${authored.length} tags; ${field.platform} accepts ${field.count} — remove ` +
          `${authored.length - field.count}`,
      ),
    );
  }

  const suggested = [meta.hashtag, meta.event]
    .filter((value): value is string => value !== undefined && value.trim() !== "")
    .map(normalizeTag)
    .filter((tag) => countCharacters(tag) <= field.length);

  const tags = [...authored];

  for (const tag of uniqueTags(suggested)) {
    if (tags.length >= field.count) break;
    if (!tags.includes(tag)) tags.push(tag);
  }

  return tags;
}

/** Slug rules, as a platform documents them. */
export interface SlugField {
  limit: number;
  /** Shortest path segment the platform will store. */
  minimum?: number;
  platform: string;
}

/**
 * The path segment the deck will live at.
 *
 * An author who pinned a slug gets it verbatim or gets told why it will not
 * work — a URL is an address other people have already written down, and one
 * silently reshaped by us is a link that stops resolving.
 *
 * A derived slug is fitted. A title with no Latin characters yields nothing to
 * derive from, which is reported rather than filled with the slide index: an
 * address that means nothing is worse than an address the author chooses.
 */
export function resolveSlug(
  meta: DeckMetadata,
  field: SlugField,
  reasons: BlockedReason[],
): string {
  const minimum = field.minimum ?? 1;
  const authored = meta.slug?.trim();

  if (authored !== undefined && authored !== "") {
    if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(authored)) {
      reasons.push(
        reason(
          "slug",
          `slug \`${authored}\` is not a path ${field.platform} accepts — use lowercase ` +
            `letters, digits, and single hyphens`,
        ),
      );
    } else if (authored.length > field.limit) {
      reasons.push(
        reason(
          "slug",
          `slug \`${authored}\` is ${authored.length} characters; ${field.platform} accepts ` +
            `${field.limit} — shorten it`,
        ),
      );
    } else if (authored.length < minimum) {
      reasons.push(
        reason(
          "slug",
          `slug \`${authored}\` is shorter than the ${minimum} characters ${field.platform} ` +
            `requires — lengthen it`,
        ),
      );
    }

    return authored;
  }

  const derived = fitSlug(asciiSlug(meta.title ?? ""), field.limit);

  if (derived.length < minimum) {
    reasons.push(
      reason(
        "slug",
        `the title yields no ${field.platform} path of at least ${minimum} characters — ` +
          "add `slug:` to the deck frontmatter",
      ),
    );
  }

  return derived;
}
