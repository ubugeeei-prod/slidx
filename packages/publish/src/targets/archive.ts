/**
 * The talk's permanent record — the one target whose input is not finished when
 * it first runs.
 *
 * Every other target composes something that is true the evening of the talk
 * and never changes again. The archive record is different: the slides go up
 * that night, and the recording appears when the conference gets round to
 * publishing it, which is weeks and sometimes never.
 *
 * So this target is built to be run twice, and it distinguishes two things the
 * other targets treat alike:
 *
 * - **Blocked** is a field the author can add right now. Only one thing blocks
 *   here, and it is having nothing at all to name the talk by.
 * - **Pending** is a field the world has not produced yet. The author cannot
 *   make a conference publish a video, so a missing recording is a reason to
 *   come back, not a reason to refuse.
 */

import { ask, source, type ArchiveRecord, type Composed, type SourceInput } from "../boundary";

export function composeArchive(input: SourceInput): Composed<ArchiveRecord> {
  return ask<Composed<ArchiveRecord>>({ op: "composeArchive", ...source(input) });
}

/** One line for a printed plan. */
export function describeArchive(record: ArchiveRecord): string {
  return ask<string>({ op: "describeArchive", record });
}

export type { ArchiveRecord } from "../boundary";
