/**
 * The write-up, started from what was already said.
 *
 * A speaker has already written the prose version of the talk: it is in the
 * speaker notes, one paragraph per slide, in order. The reason the blog post
 * usually never gets written is not that it is hard, it is that it starts from
 * an empty file at the end of a long day.
 *
 * So this is a scaffold and says so. Slide titles become section headings, and
 * notes become the body under them — a draft with the author's own sentences in
 * the author's own order, which is a thing to edit rather than a thing to
 * start. Nothing is rewritten, summarised, or generated: every word in the
 * output is a word the author already wrote.
 */

import { ask, source, type BlogScaffold, type Composed, type SourceInput } from "../boundary";

export function composeBlog(input: SourceInput): Composed<BlogScaffold> {
  return ask<Composed<BlogScaffold>>({ op: "composeBlog", ...source(input) });
}

/** One line for a printed plan. */
export function describeBlog(scaffold: BlogScaffold): string {
  return ask<string>({ op: "describeBlog", scaffold });
}

export type { BlogScaffold, BlogSection } from "../boundary";
