/**
 * How a question is stored, identified, and ordered.
 *
 * Split out of the room because it is the one part with opinions of its own:
 * what an id looks like, which list a question belongs to, and what order a
 * speaker reads them in. The room decides *whether* something may be stored;
 * this decides what the stored thing is.
 *
 * Each question lives under its own storage key rather than inside one record.
 * A single record holding two hundred questions would run at the edge of a
 * Durable Object's per-value size limit, and every upvote would rewrite all of
 * it.
 */

import type { PublishedQuestion } from "./protocol";

export interface StoredQuestion {
  id: string;
  text: string;
  name?: string;
  votes: number;
  at: number;
  /** False while the speaker has not let it through. */
  published: boolean;
}

export const QUESTION_PREFIX = "q:";

export const questionKey = (id: string): string => QUESTION_PREFIX + id;

/**
 * An id from the room's sequence number.
 *
 * Padded, so the storage keys sort in ask order. Listing a prefix returns keys
 * lexicographically, and `10` sorting before `9` would hand a speaker their
 * queue shuffled.
 */
export const questionId = (sequence: number): string => String(sequence).padStart(6, "0");

/** The public shape. Nothing about who asked survives the conversion. */
function view(question: StoredQuestion): PublishedQuestion {
  return {
    id: question.id,
    text: question.text,
    ...(question.name === undefined ? {} : { name: question.name }),
    votes: question.votes,
    at: question.at,
  };
}

/**
 * What everyone sees, most-voted first.
 *
 * The point of an upvote is to change the order. A queue that ignores its own
 * votes has a decorative button on it.
 */
export function publishedQuestions(stored: Iterable<StoredQuestion>): PublishedQuestion[] {
  return [...stored]
    .filter((question) => question.published)
    .sort((left, right) => right.votes - left.votes || left.at - right.at)
    .map(view);
}

/**
 * What the speaker still has to decide about, in ask order.
 *
 * Not sorted by votes, because nobody can vote on what nobody can see — and
 * because a moderation queue is a queue, not a chart.
 */
export function pendingQuestions(stored: Iterable<StoredQuestion>): PublishedQuestion[] {
  return [...stored]
    .filter((question) => !question.published)
    .sort((left, right) => left.at - right.at)
    .map(view);
}
