/**
 * Durations, as a speaker reads them in a report.
 *
 * The runtime has its own formatter for the presenter clock, and this is
 * deliberately not that one. A rehearsal report is produced by a CLI, a build
 * step, or a page that never loads the presenter view, and none of those should
 * have to pull in a clock to print a number.
 *
 * The formats differ too, and that is the real reason there are two. A clock
 * reads `0:09` because it is a clock; a report reads `9s` because it is a
 * sentence, and `0:09` inside a sentence reads as a time of day.
 */

/**
 * A span, rounded to the unit a speaker can act on.
 *
 * Nothing is reported more finely than a second. No speaker paces a talk in
 * milliseconds, so a report that printed them would be claiming a precision the
 * measurement does not have.
 */
export function formatSpan(ms: number): string {
  const total = Math.round(Math.abs(ms) / 1000);
  const seconds = total % 60;
  const minutes = Math.floor(total / 60);

  if (minutes === 0) return `${seconds}s`;
  if (seconds === 0) return `${minutes}m`;

  return `${minutes}m ${seconds}s`;
}

/**
 * A difference against a budget, signed so the direction is never inferred.
 *
 * A difference that rounds to zero seconds is reported as landing on budget
 * rather than as "0s over", which is arithmetically true and reads as a failure
 * to hit a target the speaker in fact hit.
 */
export function formatDelta(ms: number): string {
  const rounded = Math.round(ms / 1000);

  if (rounded === 0) return "on budget";

  return `${formatSpan(ms)} ${rounded > 0 ? "over" : "under"}`;
}

/**
 * A list, as English writes one.
 *
 * The advice is read aloud in a speaker's head, so `4, 9 and 12` beats
 * `4, 9, 12`. Without this the report would name the right slides in a voice
 * nobody wants to read after a bad rehearsal.
 */
export function formatList(items: readonly string[]): string {
  if (items.length === 0) return "";
  if (items.length === 1) return items[0] ?? "";

  return `${items.slice(0, -1).join(", ")} and ${items.at(-1)}`;
}
