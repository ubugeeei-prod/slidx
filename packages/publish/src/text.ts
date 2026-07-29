/**
 * Counting and cutting text the way a publishing platform does.
 *
 * Every function here is one call into `slidx_publish::text`, which is where
 * the rules live. They are exported rather than kept private because the rest
 * of the workspace has to count characters the same way — a character is a code
 * point, never a UTF-16 unit — and a second implementation of that rule in
 * TypeScript is exactly how a title gets rejected for a limit it visibly did
 * not reach.
 */

import { ask } from "./boundary";

/** Characters as a person counts them, not as UTF-16 stores them. */
export function countCharacters(text: string): number {
  return ask<number>({ op: "countCharacters", text });
}

/** Clips `text` to at most `limit` characters, ellipsis included. */
export function truncate(text: string, limit: number): string {
  return ask<string>({ op: "truncate", text, limit });
}

/** A slug for a URL on a platform that is not ours. ASCII only. */
export function asciiSlug(text: string): string {
  return ask<string>({ op: "asciiSlug", text });
}

/** A slug for a file on the author's own disk. Keeps every script. */
export function fileSlug(text: string): string {
  return ask<string>({ op: "fileSlug", text });
}

/** Shortens a slug we derived, on a hyphen boundary. */
export function fitSlug(slug: string, limit: number): string {
  return ask<string>({ op: "fitSlug", slug, limit });
}

/** A tag as a platform stores one: no `#`, no spaces, case-folded. */
export function normalizeTag(tag: string): string {
  return ask<string>({ op: "normalizeTag", tag });
}

/** Keeps the first spelling of each value, dropping empties. */
export function uniqueTags(tags: readonly string[]): string[] {
  return ask<string[]>({ op: "uniqueTags", tags: [...tags] });
}

/** Collapses runs of blank lines so composed Markdown diffs cleanly. */
export function tidyBlock(text: string): string {
  return ask<string>({ op: "tidyBlock", text });
}
