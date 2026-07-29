/**
 * The talk's permanent record — the one target whose input is not finished
 * when it first runs.
 *
 * Every other target here composes something that is true the evening of the
 * talk and never changes again. The archive record is different: the slides go
 * up that night, and the recording appears when the conference gets round to
 * publishing it, which is weeks and sometimes never. By then the author has
 * moved on, and the video and the slides live in two places that never learn
 * about each other.
 *
 * So this target is built to be run twice. It composes from whatever exists
 * now, and it distinguishes two things the other targets treat alike:
 *
 * - **Blocked** is a field the author can add right now. Only one thing blocks
 *   here, and it is having nothing at all to name the talk by.
 * - **Pending** is a field the world has not produced yet. The author cannot
 *   make a conference publish a video, so a missing recording is a reason to
 *   come back, not a reason to refuse.
 *
 * The second property that follows from running twice: adding the recording
 * months later must change exactly one line of the record. That is why the
 * recording appears in the frontmatter and nowhere else — a body that also
 * linked it would make the eventual diff two changes instead of one, and a
 * diff an author cannot skim is a diff they stop reading.
 */

import { fileSlug } from "../text";
import {
  blocked,
  composed,
  reason,
  type BlockedReason,
  type Composed,
  type DeckSource,
} from "../types";

/** A talk, as it will be remembered. */
export interface ArchiveRecord {
  /** File name stem. Stable, so re-running overwrites rather than piles up. */
  slug: string;
  path: string;
  title: string;
  event?: string;
  /** ISO-8601, as authored. Kept as text so ordering never needs a clock. */
  date?: string;
  venue?: string;
  author?: string;
  description?: string;
  /** Where the slides ended up. */
  deck?: string;
  /** The recording, once there is one. */
  recording?: string;
  repo?: string;
  tags: string[];
  /** What is not here *yet*, and when to come back for it. */
  pending: BlockedReason[];
  markdown: string;
}

/** Where the records live, relative to the deck. */
const ARCHIVE_DIRECTORY = "talks";

export function composeArchive(source: DeckSource): Composed<ArchiveRecord> {
  const { meta } = source;

  const title = text(meta.title) ?? text(meta.event);

  // The only thing that blocks. A record with no title and no event cannot be
  // filed, cannot be listed, and cannot be found again — it is not a record of
  // anything.
  if (title === undefined) {
    return blocked(
      reason(
        "title",
        "nothing names this talk — add `title:` to the deck frontmatter, or `event:`",
      ),
    );
  }

  const pending: BlockedReason[] = [];
  const slug = resolveArchiveSlug(title, text(meta.slug), text(meta.date), pending);
  const date = resolveDate(text(meta.date), pending);

  const deck = text(meta.url);
  if (deck === undefined) {
    pending.push(reason("url", "add `url:` once the deck is published — usually the same evening"));
  }

  const recording = text(meta.recording);
  if (recording === undefined) {
    pending.push(reason("recording", "add `recording:` when the conference publishes the video"));
  }

  const record: ArchiveRecord = {
    slug,
    path: `${ARCHIVE_DIRECTORY}/${slug}.md`,
    title,
    ...optional("event", text(meta.event)),
    ...optional("date", date),
    ...optional("venue", text(meta.venue)),
    ...optional("author", text(meta.author)),
    ...optional("description", text(meta.description)),
    ...optional("deck", deck),
    ...optional("recording", recording),
    ...optional("repo", text(meta.repo)),
    tags: [...(meta.tags ?? [])],
    pending,
    markdown: "",
  };

  return composed({ ...record, markdown: renderRecord(record) });
}

/**
 * The file name.
 *
 * `fileSlug` rather than `asciiSlug`: this file lives on the author's own
 * disk, so a Japanese talk gets a Japanese file name instead of being reduced
 * to nothing. A title that yields no slug at all — punctuation, or an emoji —
 * falls back to the date, and says so, because a file called `-.md` is a file
 * nobody finds twice.
 */
function resolveArchiveSlug(
  title: string,
  pinned: string | undefined,
  date: string | undefined,
  pending: BlockedReason[],
): string {
  if (pinned !== undefined) return pinned;

  const derived = fileSlug(title);
  if (derived !== "") return derived;

  const fallback = date === undefined ? "talk" : `talk-${fileSlug(date)}`;
  pending.push(
    reason(
      "slug",
      `the title yields no file name — add \`slug:\`, or this is filed as ${fallback}`,
    ),
  );

  return fallback;
}

/**
 * A date the index can order by.
 *
 * Checked rather than trusted, because the failure is silent: `2026-7-9` sorts
 * as text *before* `2026-11-01`, so a talk index with one sloppy date puts
 * November ahead of July and nobody reads it carefully enough to notice.
 */
function resolveDate(date: string | undefined, pending: BlockedReason[]): string | undefined {
  if (date === undefined) {
    pending.push(reason("date", "add `date:` so the talk sorts into the index"));
    return undefined;
  }

  if (!isOrderableDate(date)) {
    pending.push(
      reason("date", `\`${date}\` is not an ISO-8601 date — write it as YYYY-MM-DD so it sorts`),
    );
  }

  return date;
}

/**
 * True when a date sorts correctly as plain text.
 *
 * Zero-padded ISO-8601 is the one format where lexical order and chronological
 * order agree, which is what lets the index sort without parsing a date and
 * without a time zone entering the picture.
 */
export function isOrderableDate(date: string): boolean {
  const match = /^(\d{4})-(\d{2})-(\d{2})(?:[T ].*)?$/.exec(date);
  if (match === null) return false;

  const month = Number(match[2]);
  const day = Number(match[3]);

  return month >= 1 && month <= 12 && day >= 1 && day <= 31;
}

/** The record as a file a static site can read without knowing about slidx. */
function renderRecord(record: ArchiveRecord): string {
  const front = [
    yamlEntry("title", record.title),
    yamlEntry("event", record.event),
    yamlEntry("date", record.date),
    yamlEntry("venue", record.venue),
    yamlEntry("author", record.author),
    yamlEntry("description", record.description),
    yamlEntry("deck", record.deck),
    yamlEntry("recording", record.recording),
    yamlEntry("repo", record.repo),
    record.tags.length === 0 ? undefined : `tags: [${record.tags.map(yamlString).join(", ")}]`,
  ].filter((line): line is string => line !== undefined);

  const body = [`# ${record.title}`, record.description].filter(
    (line): line is string => line !== undefined,
  );

  return `---\n${front.join("\n")}\n---\n\n${body.join("\n\n")}\n`;
}

/**
 * A key omitted rather than emitted empty.
 *
 * `recording: ""` reads to a site template as "there is no recording", which
 * is a different claim from "not yet" and the one thing this target exists to
 * keep straight.
 */
function yamlEntry(key: string, value: string | undefined): string | undefined {
  return value === undefined ? undefined : `${key}: ${yamlString(value)}`;
}

/**
 * Always quoted.
 *
 * A talk title with a colon in it is normal and would otherwise become two
 * keys, so quoting unconditionally costs nothing and removes the whole class.
 */
function yamlString(value: string): string {
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

/** A trimmed value, or nothing at all — never an empty string. */
function text(value: string | undefined): string | undefined {
  const trimmed = value?.trim() ?? "";
  return trimmed === "" ? undefined : trimmed;
}

/** A key present only when it has a value, as `exactOptionalPropertyTypes` wants. */
function optional<K extends string, T>(key: K, value: T | undefined): Partial<Record<K, T>> {
  return value === undefined ? {} : ({ [key]: value } as Record<K, T>);
}

/** One line for a printed plan. */
export function describeArchive(record: ArchiveRecord): string {
  if (record.pending.length === 0) return `write ${record.path}`;

  const fields = [...new Set(record.pending.map((entry) => entry.field))];
  return `write ${record.path} — awaiting ${fields.join(", ")}`;
}
