/**
 * Which slide a URL asks for, and what to say when there is no slide at all.
 *
 * The deck's public shape lives here: one URL per slide, one-based because that
 * is how a person counts them, with `/presenter` and `/print` alongside. Dev
 * and build both go through this, so a route that works while writing is the
 * route that ships.
 */

/** A slide, and which of its views was asked for. */
export interface SlideRequest {
  index: number;
  presenter: boolean;
  /** The whole deck as one printable document, rather than one slide. */
  print?: boolean;
  /** Every slide at once, as a page of links. */
  overview?: boolean;
  /** The phone remote. One page per deck. */
  remote?: boolean;
}

/**
 * Which slide a URL asks for, or `null` when the URL is not ours.
 *
 * Returning `null` rather than a 404 lets everything else in the project —
 * assets, other plugins, the dev client — keep working alongside a deck.
 */
export function slideRequestFor(url: string, base: string): SlideRequest | null {
  const path = url.split("?")[0]!.replace(/\/+$/, "");
  const prefix = base ? `/${base}` : "";

  if (!path.startsWith(prefix)) return null;

  let rest = path
    .slice(prefix.length)
    .replace(/^\//, "")
    .replace(/\/index\.html$/, "");
  if (rest === "index.html") rest = "";

  if (rest === "print") return { index: 0, presenter: false, print: true };
  if (rest === "overview") return { index: 0, presenter: false, overview: true };
  if (rest === "remote") return { index: 0, presenter: false, remote: true };

  const presenter = rest === "presenter" || rest.endsWith("/presenter");
  if (presenter) rest = rest.replace(/\/?presenter$/, "");

  if (rest === "") return { index: 0, presenter };

  const match = /^(\d+)$/.exec(rest);
  if (!match) return null;

  // Slides are one-based in a URL because that is how a person counts them.
  const number = Number(match[1]);
  return number >= 2 ? { index: number - 1, presenter } : null;
}

/**
 * What to say when there is nothing to show.
 *
 * An empty deck is the state every new project starts in, so this is the first
 * thing many people will see. It says what to do next rather than what went
 * wrong.
 */
export function emptyDeckMessage(count: number, srcDir: string): string {
  if (count > 0) return "No slide at this number.";

  return (
    `No slides found in ./${srcDir}.\n\n` +
    `Create ./${srcDir}/0001.md and this page will reload:\n\n` +
    "  # My first slide\n\n" +
    "  - a point\n"
  );
}
