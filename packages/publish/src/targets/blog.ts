/**
 * The write-up, started from what was already said.
 *
 * A speaker has already written the prose version of the talk: it is in the
 * speaker notes, one paragraph per slide, in order. The reason the blog post
 * usually never gets written is not that it is hard, it is that it starts from
 * an empty file at the end of a long day.
 *
 * So this is a scaffold and says so. Slide titles become section headings, and
 * notes become the body under them — a draft with the author's own sentences
 * in the author's own order, which is a thing to edit rather than a thing to
 * start. Nothing is rewritten, summarised, or generated: every word in the
 * output is a word the author already wrote.
 */

import { fileSlug, tidyBlock } from "../text";
import {
  blocked,
  composed,
  reason,
  type BlockedReason,
  type Composed,
  type DeckSource,
} from "../types";

/** One slide's worth of draft. */
export interface BlogSection {
  heading: string;
  /** The slide's notes, joined. Never edited. */
  body: string;
  /** Slide the section came from, so an editor can jump back. */
  slide: number;
}

export interface BlogScaffold {
  /** Suggested file name, dated so drafts sort by talk. */
  path: string;
  title: string;
  sections: BlogSection[];
  /** The whole file, frontmatter included. */
  markdown: string;
}

export function composeBlog(source: DeckSource): Composed<BlogScaffold> {
  const reasons: BlockedReason[] = [];
  const title = source.meta.title?.trim() ?? "";

  if (title === "") {
    reasons.push(reason("title", "a draft needs a title — add `title:` to the deck frontmatter"));
  }

  const sections = sectionsOf(source);

  // A scaffold of empty headings is worse than no scaffold: it looks like work
  // that has been done. Say the notes are missing instead.
  if (sections.length === 0) {
    reasons.push(
      reason(
        "notes",
        "the deck has no speaker notes — a draft is assembled from them, so there is " +
          "nothing to assemble",
      ),
    );
  }

  if (reasons.length > 0) return blocked(...reasons);

  return composed({
    path: pathFor(source, title),
    title,
    sections,
    markdown: render(source, title, sections),
  });
}

/**
 * One section per slide that has notes.
 *
 * Slides without notes are skipped rather than emitted as bare headings. A
 * title slide, a section divider, and a slide that is one image all belong to
 * the talk and none of them belongs to the write-up.
 */
function sectionsOf(source: DeckSource): BlogSection[] {
  const slides = [...source.slides].sort((left, right) => left.index - right.index);
  const sections: BlogSection[] = [];

  for (const slide of slides) {
    const body = tidyBlock((slide.notes ?? []).join("\n\n"));
    if (body === "") continue;

    sections.push({
      // A slide with no heading still has a place in the draft, and "Slide 4"
      // is a placeholder the author will replace — which is what a scaffold is.
      heading: slide.title?.trim() || `Slide ${slide.index + 1}`,
      body,
      slide: slide.index,
    });
  }

  return sections;
}

function render(source: DeckSource, title: string, sections: readonly BlogSection[]): string {
  const { meta } = source;

  // A fixed order, and only the keys the deck has a value for. A frontmatter
  // block full of empty strings is something the author has to delete before
  // the draft is publishable.
  const front = (
    [
      ["title", title],
      ["date", meta.date],
      ["event", meta.event],
      ["slides", meta.url],
    ] as const
  )
    .map(([key, value]) => [key, value?.trim() ?? ""] as const)
    .filter(([, value]) => value !== "")
    .map(([key, value]) => `${key}: ${yamlString(value)}`);

  if (meta.tags !== undefined && meta.tags.length > 0) {
    front.push(`tags: [${meta.tags.map(yamlString).join(", ")}]`);
  }

  const blocks = [`---\n${front.join("\n")}\n---`];

  // The deck's description is already a one-paragraph summary of the talk,
  // which is exactly what the top of the post needs.
  if (meta.description !== undefined && meta.description.trim() !== "") {
    blocks.push(meta.description.trim());
  }

  for (const section of sections) {
    blocks.push(`## ${section.heading}`, section.body);
  }

  return `${blocks.join("\n\n")}\n`;
}

/**
 * Quoted, always.
 *
 * A title containing a colon — which is most conference talk titles — is not
 * valid YAML unquoted, and the failure surfaces in whatever static site
 * generator reads the draft rather than here.
 */
function yamlString(value: string): string {
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

/**
 * A file name for the draft, on the author's own disk.
 *
 * Unicode is kept: this is a local file, not a URL on someone else's site, and
 * a deck written in Japanese should not become `deck-2.md`. The date leads so
 * a directory of drafts sorts by talk.
 */
function pathFor(source: DeckSource, title: string): string {
  const slug = fileSlug(title) || fileSlug(source.meta.event ?? "") || "deck";
  const date = source.meta.date?.trim() ?? "";

  return date === "" ? `${slug}.md` : `${date}-${slug}.md`;
}

/** One line for a printed plan. */
export function describeBlog(scaffold: BlogScaffold): string {
  return `write ${scaffold.path} from ${scaffold.sections.length} slide(s) of notes`;
}
