/**
 * A deck to publish, and the two things every test does to it.
 *
 * `TALK` is a conference deck with every field filled in, because the
 * interesting cases are all subtractions from a complete one: what happens
 * when the url is gone, when the notes are gone, when the PDF was never built.
 * `without` is how a test performs that subtraction — `exactOptionalPropertyTypes`
 * means an absent field has to be genuinely absent, and a test that wrote
 * `{ url: undefined }` would be testing a shape the type system forbids callers
 * from producing.
 */

import type { Artifact, DeckMetadata, DeckSlide, DeckSource } from "../src/types";

/** Named separately so a test can assert against it without a `?? ""`. */
export const DESCRIPTION =
  "Why a deck should be plain HTML, and what it costs to keep it that way.";

/** A finished conference deck, with nothing missing. */
export const TALK: DeckMetadata = {
  title: "Zero-JavaScript Slides",
  description: DESCRIPTION,
  author: "ubugeeei",
  event: "SlidxConf 2026",
  date: "2026-07-29",
  venue: "Kyoto",
  hashtag: "slidxconf",
  url: "https://slidx.dev/talks/zero-js",
  repo: "https://github.com/ubugeeei-prod/slidx",
  tags: ["rust", "slides"],
};

/** The build output the upload targets ask for. */
export const PDF: Artifact = { kind: "pdf", path: "dist/deck.pdf", bytes: 4 * 1024 * 1024 };

export function deck(
  meta: DeckMetadata = TALK,
  options: { slides?: DeckSlide[]; artifacts?: Artifact[] } = {},
): DeckSource {
  return {
    meta,
    slides: options.slides ?? [],
    artifacts: options.artifacts ?? [PDF],
  };
}

/** A slide, positioned. Everything else is what the test is about. */
export function slide(index: number, rest: Omit<DeckSlide, "index"> = {}): DeckSlide {
  return { index, ...rest };
}

/** The same metadata with a field genuinely absent, as frontmatter would be. */
export function without<T extends object, K extends keyof T & string>(
  value: T,
  ...keys: K[]
): Omit<T, K> {
  const entries = Object.entries(value).filter(([key]) => !keys.includes(key as K));
  return Object.fromEntries(entries) as Omit<T, K>;
}

/** A string of `count` characters, for testing a cap. */
export function characters(count: number, fill = "a"): string {
  return fill.repeat(count);
}
