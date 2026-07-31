/**
 * The page a registry shows for each thing this repository publishes.
 *
 * crates.io and npm both render the README from inside the tarball and nothing
 * else. A workspace with one README at its root therefore publishes 28 blank
 * pages — which is what a publish dry run found here, on a project whose whole
 * argument is that the writing matters.
 *
 * # Generated, and checked rather than trusted
 *
 * Every one of these says the same four things: what this piece is, that it is
 * part of something larger, what most people install instead, and where the
 * documentation is. Written by hand that would be 28 copies of one paragraph
 * drifting — the same failure `licensed.mjs` guards against, and the same
 * answer: compose them from what the manifests already say, and let CI compare.
 *
 * The one sentence that differs between them is the `description` each manifest
 * already carries, so a crate's page is wrong only if its own description is.
 *
 * # Why the front doors are named on every page
 *
 * Somebody arriving at `slidx_edit` from a search result has almost certainly
 * not gone looking for an edit-operation model. They want to make a deck. Both
 * install lines are on every page for that reader, including on the two pages
 * that are themselves a front door — where the line they need is the one they
 * are already looking at.
 */

const REPOSITORY = "https://github.com/ubugeeei-prod/slidx";

/** The name both registries look for, and the only one this writes. */
export const PAGE_FILE = "README.md";

/** The name a page claims to be about: the text of its first heading. */
export function firstHeading(page) {
  const found = /^#\s+(.+?)\s*$/m.exec(page);

  return found === null ? undefined : found[1];
}

/**
 * What slidx is, in the two sentences a registry page has to spend them on.
 *
 * Deliberately not the README's opening, which is written for somebody who has
 * already arrived. This is written for somebody who has not.
 */
const WHAT = [
  `[slidx](${REPOSITORY}) is a DX framework for the whole life of a talk: writing`,
  "the deck, editing it visually, catching what a projector will do to it,",
  "presenting it, and publishing it afterwards. A deck is Markdown, the visual",
  "editor writes that same Markdown, and a built deck asks nothing of anywhere",
  "but itself.",
].join("\n");

/** The two things nearly everybody installs, and nothing else. */
const START = ["```", "npm i -D @slidxjs/vite-plugin", "npm i -g slidx", "```"].join("\n");

/**
 * One registry page.
 *
 * `description` comes from the manifest rather than from a table here, so this
 * cannot describe a crate as something other than what its own `Cargo.toml`
 * says it is.
 */
export function registryPage({ name, description }) {
  const sentence = description.endsWith(".") ? description : `${description}.`;

  return [
    `# ${name}`,
    "",
    sentence,
    "",
    "## Where this fits",
    "",
    WHAT,
    "",
    "Most people install these two and nothing else:",
    "",
    START,
    "",
    "## Documentation",
    "",
    `${REPOSITORY}#readme`,
    "",
    "## License",
    "",
    `MIT. The notice is in this package, and at ${REPOSITORY}/blob/main/LICENSE.`,
    "",
  ].join("\n");
}
