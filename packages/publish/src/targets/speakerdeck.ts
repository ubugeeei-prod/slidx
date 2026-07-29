/**
 * Speaker Deck, as a payload.
 *
 * Speaker Deck is a PDF host: the deck is the file, and everything else is the
 * page around it. That makes the failure mode specific — the upload is the
 * slowest step in publishing, and a title two characters over the cap fails
 * *after* the file has gone up.
 *
 * The caps themselves live in `slidx_publish::targets::speakerdeck`, next to
 * the fields they constrain, read conservatively on purpose. Being ten
 * characters under costs nothing; being one over costs a re-upload at the end
 * of a long day.
 */

import { ask, source, type Composed, type SourceInput, type SpeakerDeckUpload } from "../boundary";

export function composeSpeakerDeck(input: SourceInput): Composed<SpeakerDeckUpload> {
  return ask<Composed<SpeakerDeckUpload>>({ op: "composeSpeakerDeck", ...source(input) });
}

/** One line for a printed plan. */
export function describeSpeakerDeck(upload: SpeakerDeckUpload): string {
  return ask<string>({ op: "describeSpeakerDeck", upload });
}

export type { SpeakerDeckUpload } from "../boundary";
