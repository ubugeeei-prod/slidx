/**
 * Every talk, on one page.
 *
 * The index is the reason the per-talk record is worth writing. An author who
 * has given thirty talks has thirty decks in thirty repositories and no list of
 * them, and assembles one by hand the week they need a speaker bio. The records
 * already carry everything that list needs, so building it is a collection job
 * rather than an authoring one.
 *
 * The ordering is `slidx_publish::talks`': most recent first, because that is
 * what a speaking page is read for, and an undated talk kept after the dated
 * ones rather than dropped or given a date nobody wrote.
 */

import { ask, type ArchiveRecord, type TalkIndex, type TalkIndexOptions } from "./boundary";

export function buildTalkIndex(
  records: readonly ArchiveRecord[],
  options: TalkIndexOptions = {},
): TalkIndex {
  return ask<TalkIndex>({ op: "buildTalkIndex", records: [...records], options });
}

export type { TalkIndex, TalkIndexOptions } from "./boundary";
