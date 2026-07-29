/**
 * Every talk, on one page.
 *
 * The index is the reason the per-talk record is worth writing. An author who
 * has given thirty talks has thirty decks in thirty repositories and no list of
 * them, and assembles one by hand the week they need a speaker bio. The records
 * already carry everything that list needs, so building it is a collection job
 * rather than an authoring one.
 *
 * Two ordering decisions, both about not losing anything:
 *
 * **Most recent first.** That is what a speaking page is read for. Because
 * dates are zero-padded ISO-8601 text, sorting is a string comparison and
 * involves no clock and no time zone.
 *
 * **An undated talk still appears.** It goes after the dated ones, in the order
 * it was given in. Dropping it would lose a talk, and inventing a date to sort
 * it by would put a fabrication in a permanent record.
 */

import { isOrderableDate, type ArchiveRecord } from "./targets/archive";

export interface TalkIndexOptions {
  /** Heading of the page. */
  title?: string;
  path?: string;
}

export interface TalkIndex {
  title: string;
  path: string;
  /** Most recent first; undated last, in the order they were given. */
  talks: ArchiveRecord[];
  /**
   * How many records still have no recording.
   *
   * The one number worth surfacing: it is a to-do list of conferences to chase,
   * and it is the only thing about an archive that changes after the fact.
   */
  awaitingRecording: number;
  markdown: string;
}

const DEFAULT_TITLE = "Talks";
const DEFAULT_PATH = "talks/index.md";

export function buildTalkIndex(
  records: readonly ArchiveRecord[],
  options: TalkIndexOptions = {},
): TalkIndex {
  const talks = order(records);
  const title = options.title?.trim() || DEFAULT_TITLE;

  return {
    title,
    path: options.path?.trim() || DEFAULT_PATH,
    talks,
    awaitingRecording: talks.filter((talk) => talk.recording === undefined).length,
    markdown: render(title, talks),
  };
}

/**
 * Dated descending, then undated in input order.
 *
 * `Array#sort` is stable, so two talks on the same day keep the order they
 * arrived in — a morning and an afternoon slot at the same conference read
 * correctly rather than swapping between runs.
 */
function order(records: readonly ArchiveRecord[]): ArchiveRecord[] {
  const dated = records.filter(isDated);
  const undated = records.filter((record) => !isDated(record));

  return [...dated.sort((a, b) => (b.date ?? "").localeCompare(a.date ?? "")), ...undated];
}

/**
 * A date that can be ordered, rather than merely present.
 *
 * A malformed date sorts wrongly and silently, so a record carrying one is
 * listed as undated instead. The record itself already reports the problem as
 * pending, which is where the author is told to fix it.
 */
function isDated(record: ArchiveRecord): boolean {
  return record.date !== undefined && isOrderableDate(record.date);
}

function render(title: string, talks: readonly ArchiveRecord[]): string {
  const lines = [`# ${title}`];

  let heading: string | undefined;

  for (const talk of talks) {
    const group = isDated(talk) ? (talk.date ?? "").slice(0, 4) : "Undated";

    if (group !== heading) {
      heading = group;
      lines.push("", `## ${group}`, "");
    }

    lines.push(`- ${entry(talk)}`);
  }

  return `${lines.join("\n")}\n`;
}

/**
 * One talk, as a line.
 *
 * A link appears only when its URL does. An empty `[video]()` is a link that
 * navigates to the page it is on, which is worse than the absence it was
 * standing in for.
 */
function entry(talk: ArchiveRecord): string {
  const where = [talk.event, talk.venue].filter(isPresent).join(", ");
  const links = [
    link("slides", talk.deck),
    link("video", talk.recording),
    link("code", talk.repo),
  ].filter(isPresent);

  return [talk.date, `**${talk.title}**`, where === "" ? undefined : where, ...links]
    .filter(isPresent)
    .join(" · ");
}

function link(label: string, url: string | undefined): string | undefined {
  return url === undefined ? undefined : `[${label}](${url})`;
}

function isPresent(value: string | undefined): value is string {
  return value !== undefined && value !== "";
}
