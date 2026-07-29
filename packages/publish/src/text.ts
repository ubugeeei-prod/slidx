/**
 * Counting and cutting text the way a publishing platform does.
 *
 * Every cap in this package is a count of *characters*, and JavaScript has
 * three plausible answers to how many characters a string has. `String#length`
 * counts UTF-16 units, so one emoji costs two and a deck titled with one is
 * rejected for a limit it visibly did not reach. Code points are the count a
 * platform's own validator agrees with often enough to be safe, and are what
 * this module uses everywhere.
 *
 * The two slug functions look like duplicates and are not. A path segment on
 * someone else's platform has to survive their URL rules, which in practice
 * means ASCII; a file on the author's own disk does not, and a Japanese deck
 * deserves a Japanese filename rather than `slide-deck-2`.
 */

/** Characters as a person counts them, not as UTF-16 stores them. */
export function countCharacters(text: string): number {
  return Array.from(text).length;
}

/** The ellipsis is one code point, so it costs one character of the budget. */
const ELLIPSIS = "…";

/**
 * Clips `text` to at most `limit` characters, ellipsis included.
 *
 * Cuts on a word boundary when there is one in the second half of the budget.
 * The restriction matters for scripts that do not space their words: a
 * Japanese sentence has no boundary to find, and honouring the first space in
 * a mostly-CJK string would throw away most of the budget to keep one Latin
 * word intact.
 */
export function truncate(text: string, limit: number): string {
  if (limit <= 0) return "";

  const characters = Array.from(text);
  if (characters.length <= limit) return text;
  if (limit === 1) return ELLIPSIS;

  const budget = limit - 1;
  const candidate = characters.slice(0, budget).join("");
  const lastBreak = candidate.search(/\s+\S*$/u);
  const broken = lastBreak > budget / 2 ? candidate.slice(0, lastBreak) : candidate;

  return `${broken.trimEnd()}${ELLIPSIS}`;
}

/**
 * A slug for a URL on a platform that is not ours.
 *
 * ASCII only. Returns an empty string when nothing survives — a title written
 * entirely in kana has no Latin slug, and inventing one from the slide index
 * would produce a URL that means nothing and changes silently when a slide
 * moves. Callers report the empty result and name `slug` as the fix.
 */
export function asciiSlug(text: string): string {
  let slug = "";

  for (const character of text) {
    if (/[a-zA-Z0-9]/.test(character)) {
      slug += character.toLowerCase();
    } else if (!slug.endsWith("-")) {
      slug += "-";
    }
  }

  return trimHyphens(slug);
}

/**
 * A slug for a file on the author's own disk.
 *
 * Keeps letters and digits from any script, case-folded by the Unicode rules
 * rather than the ASCII ones, matching `slug.rs` in `slidx_core` so a deck's
 * anchors and its blog draft are named alike.
 */
export function fileSlug(text: string): string {
  let slug = "";

  for (const character of text) {
    if (/[\p{L}\p{N}]/u.test(character)) {
      slug += character.toLowerCase();
    } else if (!slug.endsWith("-")) {
      slug += "-";
    }
  }

  return trimHyphens(slug);
}

/**
 * Shortens a slug we derived, on a hyphen boundary.
 *
 * Only ever applied to a slug this package invented. A slug the author wrote
 * is theirs, and a URL half of which we chose is worse than a reported cap.
 */
export function fitSlug(slug: string, limit: number): string {
  if (slug.length <= limit) return slug;

  // Always on a hyphen when there is one: a slug is read as words, and half a
  // word in a URL looks like a bug rather than a shortening.
  const clipped = slug.slice(0, limit);
  const lastHyphen = clipped.lastIndexOf("-");

  return trimHyphens(lastHyphen > 0 ? clipped.slice(0, lastHyphen) : clipped);
}

function trimHyphens(slug: string): string {
  return slug.replace(/^-+|-+$/g, "");
}

/**
 * A tag as a platform stores one: no `#`, no spaces, case-folded.
 *
 * Case folding is what makes deduplication work. `Rust` and `rust` are one tag
 * everywhere they are actually stored, so treating them as two would publish a
 * list with a visible duplicate in it.
 */
export function normalizeTag(tag: string): string {
  return tag.trim().replace(/^#+/, "").replace(/\s+/gu, "-").toLowerCase();
}

/** Keeps the first spelling of each value, dropping empties. */
export function uniqueTags(tags: readonly string[]): string[] {
  const seen = new Set<string>();
  const kept: string[] = [];

  for (const tag of tags) {
    const normalized = normalizeTag(tag);
    if (normalized === "" || seen.has(normalized)) continue;
    seen.add(normalized);
    kept.push(normalized);
  }

  return kept;
}

/** Collapses runs of blank lines so composed Markdown diffs cleanly. */
export function tidyBlock(text: string): string {
  return text
    .replace(/\r\n?/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}
