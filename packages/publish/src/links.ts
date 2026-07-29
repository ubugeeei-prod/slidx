/**
 * Every link a deck mentions, in the order the audience met it.
 *
 * This is the only place that knows what a link looks like in Markdown. The
 * resources page is built from it, and the thing that makes such a page worth
 * generating is that it is exhaustive: a link an author read out loud from a
 * slide and never wrote down anywhere else is exactly the one an attendee
 * comes looking for afterwards.
 *
 * Code is excluded. A URL inside a fenced block is usually an example endpoint
 * or an import path, and listing `https://api.example.com/v1` as a resource
 * sends people somewhere that does not exist. Fence awareness is the same rule
 * `scanner.rs` applies to slide separators, for the same reason.
 */

import type { DeckSource } from "./types";

/** A link, attributed to where it first appeared. */
export interface DeckLink {
  /** As authored, minus trailing punctuation that belonged to the sentence. */
  url: string;
  /** Link text where there was some, otherwise the URL without its scheme. */
  label: string;
  /** Slide index, or null for a link that came from the frontmatter. */
  slide: number | null;
}

/**
 * Deck links, deduplicated, in slide order.
 *
 * The repository comes first because it is deck-level: it belongs to the talk
 * rather than to any one slide. The deck's own canonical url is deliberately
 * absent — a resources page that links to the page it is part of is a loop,
 * not a resource.
 *
 * First mention wins on both label and position. The first time a link appears
 * is where it was introduced, which is where its text is most likely to say
 * what it is.
 */
export function collectLinks(source: DeckSource): DeckLink[] {
  const found: DeckLink[] = [];

  if (source.meta.repo !== undefined && isHttp(source.meta.repo)) {
    found.push({ url: source.meta.repo, label: labelForUrl(source.meta.repo), slide: null });
  }

  const slides = [...source.slides].sort((left, right) => left.index - right.index);

  for (const slide of slides) {
    // Body before notes: the audience saw the slide, the speaker read the
    // notes, and a link in both should be attributed to the one on screen.
    const blocks = [slide.content ?? "", ...(slide.notes ?? [])];

    for (const block of blocks) {
      for (const link of extractLinks(block)) {
        found.push({ ...link, slide: slide.index });
      }
    }
  }

  return dedupe(found);
}

function dedupe(links: readonly DeckLink[]): DeckLink[] {
  const seen = new Set<string>();
  const kept: DeckLink[] = [];

  for (const link of links) {
    const key = canonicalKey(link.url);
    if (seen.has(key)) continue;
    seen.add(key);
    kept.push(link);
  }

  return kept;
}

/**
 * The identity two links share when they are the same resource.
 *
 * Scheme and host case-fold, because they are case-insensitive by spec. Path,
 * query, and fragment are left exactly as written: a fragment addresses a
 * section, and collapsing two anchors of one long page into a single entry
 * loses the part that made each of them worth listing.
 */
function canonicalKey(url: string): string {
  try {
    return new URL(url).href;
  } catch {
    // Something the URL parser rejects is still a string an author wrote, and
    // an exact match is the only claim about it that is safe to make.
    return url;
  }
}

/** Links in one block of Markdown, in source order. */
export function extractLinks(markdown: string): Array<{ url: string; label: string }> {
  const text = withoutCode(markdown);
  const found: Array<{ url: string; label: string }> = [];

  for (const match of text.matchAll(LINK_PATTERN)) {
    const groups = match.groups ?? {};

    // An image was matched only so its URL is consumed before the bare-URL
    // branch can see it. An asset is not a resource.
    if (groups.image !== undefined) continue;

    const url = groups.inlineUrl ?? groups.refUrl ?? groups.autoUrl ?? groups.bareUrl;
    if (url === undefined) continue;

    const trimmed = trimTrailingPunctuation(url);
    if (!isHttp(trimmed)) continue;

    const authored = groups.inlineText ?? groups.refId;
    const label = authored === undefined ? "" : cleanLabel(authored);

    found.push({ url: trimmed, label: label === "" ? labelForUrl(trimmed) : label });
  }

  return found;
}

/**
 * Every link syntax, as one alternation.
 *
 * One pass, so the result is in source order without sorting, and so an
 * earlier branch consumes the text a later one would otherwise re-match: the
 * URL inside `[docs](https://…)` must not also be found as a bare URL.
 */
const LINK_PATTERN = new RegExp(
  [
    // Images, matched to be discarded.
    String.raw`(?<image>!\[[^\]]*\]\([^)]*\))`,
    // [text](url), with an optional title.
    String.raw`\[(?<inlineText>[^\]]*)\]\(\s*<?(?<inlineUrl>[^\s)>]+)>?(?:\s+["'][^"']*["'])?\s*\)`,
    // [id]: url — a reference definition, always on its own line.
    String.raw`^\s{0,3}\[(?<refId>[^\]]+)\]:\s*<?(?<refUrl>\S+?)>?\s*$`,
    // <https://…>
    String.raw`<(?<autoUrl>[a-zA-Z][a-zA-Z0-9+.-]*://[^\s>]+)>`,
    // A URL written into prose.
    String.raw`(?<bareUrl>[a-zA-Z][a-zA-Z0-9+.-]*://[^\s<>"'\]]+)`,
  ].join("|"),
  "gmu",
);

/**
 * Markdown with code removed.
 *
 * Line-based rather than a regex over the whole block: a fence is closed by a
 * marker of the same character and at least the same length, which is a rule
 * about lines and reads as one.
 */
function withoutCode(markdown: string): string {
  const kept: string[] = [];
  let fence: string | null = null;

  for (const line of markdown.split(/\r\n?|\n/)) {
    const marker = /^\s{0,3}(`{3,}|~{3,})/.exec(line)?.[1];

    if (fence === null) {
      if (marker !== undefined) {
        fence = marker;
        continue;
      }
      kept.push(line.replace(/`[^`\n]*`/g, ""));
      continue;
    }

    if (marker !== undefined && marker[0] === fence[0] && marker.length >= fence.length) {
      fence = null;
    }
  }

  return kept.join("\n");
}

/**
 * Drops the punctuation that ended the sentence rather than the URL.
 *
 * A closing bracket is kept when the URL opened one, because Wikipedia and
 * MDN both publish paths that contain balanced parentheses.
 */
function trimTrailingPunctuation(url: string): string {
  let trimmed = url.replace(/[.,;:!?"']+$/u, "");

  while (trimmed.endsWith(")") && countOf(trimmed, ")") > countOf(trimmed, "(")) {
    trimmed = trimmed.slice(0, -1);
  }

  return trimmed;
}

function countOf(text: string, character: string): number {
  let count = 0;
  for (const found of text) if (found === character) count += 1;
  return count;
}

/** Link text, flattened to one line and stripped of emphasis markers. */
function cleanLabel(text: string): string {
  return text.replace(/[*_`]/g, "").replace(/\s+/gu, " ").trim();
}

/** A readable stand-in for link text: the URL without the noise. */
export function labelForUrl(url: string): string {
  return url
    .replace(/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//, "")
    .replace(/^www\./, "")
    .replace(/\/$/, "");
}

/**
 * Only web links are resources.
 *
 * `mailto:` is an address, a relative path is part of the deck, and neither is
 * something an attendee can open from a page of links.
 */
export function isHttp(url: string): boolean {
  return /^https?:\/\//i.test(url);
}
