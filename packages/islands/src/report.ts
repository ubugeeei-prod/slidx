/**
 * How an island says it went wrong.
 *
 * Nothing in this package throws at the deck. A speaker on stage cannot debug
 * a framework, so every failure an island can have is turned into a report and
 * the slide carries on presenting. That makes reporting the only channel there
 * is, which is why it is a value with a kind rather than a formatted string:
 * a dev overlay wants to group by kind and point at an element, and CI wants to
 * fail on any of them.
 *
 * The reporter is injected. Bound to `console` it would be untestable, and a
 * deck embedded in a larger app has somewhere better to send this.
 */

/** Which failure happened. Each one is a different mistake with a different fix. */
export type IslandProblemKind =
  /** The markup marked an island but named none. A compiler bug, not an authoring one. */
  | "missing-name"
  /** The slide asked for a framework nothing registered. Almost always a typo. */
  | "unknown-island"
  /** The props attribute was not a JSON object. The island mounts without props. */
  | "invalid-props"
  /** The integration's `mount` threw or rejected. The placeholder stays. */
  | "mount-failed"
  /** `unmount` threw. Whatever it holds is now leaked and cannot be reclaimed. */
  | "unmount-failed"
  /** `mount` resolved with something that cannot be unmounted. A leak, once per island. */
  | "invalid-handle";

export interface IslandProblem {
  readonly kind: IslandProblemKind;
  /** The name as written in the markup. Empty when the markup gave none. */
  readonly name: string;
  /** The island's element, so an overlay can point at the slide it is on. */
  readonly element: HTMLElement;
  /** One line, addressed to whoever wrote the slide. */
  readonly message: string;
  /** The thrown value, where there was one. */
  readonly cause?: unknown;
}

export type IslandReporter = (problem: IslandProblem) => void;

/** Prefixed so a problem is attributable in a console a deck shares with a host app. */
const PREFIX = "slidx islands:";

/**
 * The default: a warning, not an error.
 *
 * `warn` rather than `error` because none of these stop the deck, and a red
 * console during a live talk reads as something worse than it is. The console
 * is a parameter so a test can assert what was written.
 */
export function consoleReporter(target: Pick<Console, "warn"> = console): IslandReporter {
  return (problem) => {
    target.warn(`${PREFIX} ${problem.message}`, problem.element, problem.cause ?? "");
  };
}

/**
 * Wraps a reporter so it cannot take the slide down.
 *
 * The reporter is supplied by whoever embeds the deck, and it runs on the path
 * that exists to survive failure. A reporter that throws while reporting a
 * broken island would turn one dead island into a dead slide — the exact
 * outcome the isolation is for.
 */
export function guardReporter(report: IslandReporter): IslandReporter {
  return (problem) => {
    try {
      report(problem);
    } catch {
      // Deliberately silent. There is nowhere left to report to.
    }
  };
}
