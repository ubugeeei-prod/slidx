/**
 * What a built deck is allowed to weigh, and what it is allowed to load.
 *
 * slidx makes two claims about output that a reader has no way to check: that a
 * slide without steps fetches nothing, and that a deck is small. Both are the
 * kind of claim that stays true until the afternoon somebody adds one import to
 * a shell, and then stays *written down* for a year.
 *
 * The first of those used to be stronger — no JavaScript at all — and it was
 * kept for a reason nobody had noticed: a slide with no steps had no navigation
 * of any kind, so there was nothing for a script to do. The budget below is the
 * shape that claim should always have had, in two figures that fail
 * differently.
 *
 * So they are budgets rather than sentences, and CI fails on them.
 *
 * # Why bytes and not milliseconds
 *
 * Because a byte is the same on every machine and a millisecond is not. A time
 * budget on a shared runner fails on a slow afternoon, and a gate that fails
 * for reasons nobody caused is a gate somebody switches off — which costs more
 * than never having had it. `scripts/bench-build.mjs` measures time and reports
 * it, and that is the right shape for a number that moves on its own.
 *
 * Output size does not move on its own. Every byte in these figures was put
 * there by a change in this repository.
 *
 * # Moving one
 *
 * Deliberately, in review, with the reason in the commit. That is the whole
 * point of the number being here rather than in somebody's memory: a build that
 * grew 40% has either earned it or nobody noticed, and those look identical
 * until a budget makes someone say which.
 */

/**
 * The deck every figure below is measured against.
 *
 * Small and fixed. A budget measured against a deck that changes is a budget
 * that reports on the deck rather than on the pipeline — and these numbers are
 * about what slidx puts around a slide, not about what an author wrote in one.
 *
 * The three slides are the three cases with different answers: nothing that
 * moves, something that moves, and something that has to be fetched.
 */
export const FIXTURE = {
  "0001.md": [
    "---",
    "title: Budget",
    "duration: 10m",
    "---",
    "",
    "# A deck that has to stay small",
    "",
    "A paragraph of the length prose usually is, so the page is not measured",
    "empty. It mentions `inline code` and **emphasis**, because both reach the",
    "highlighter and the theme.",
    "",
  ].join("\n"),
  "0002.md": [
    "---",
    "autoSteps: list",
    "---",
    "",
    "## A slide that moves",
    "",
    "- The first point",
    "- The second, with [a marked phrase]{#result .accent}",
    "- The third",
    "",
  ].join("\n"),
  "0003.md": ["## A slide with a picture", "", "![A one-pixel square](./square.png)", ""].join(
    "\n",
  ),
  "0004.md": [
    "## A slide with code to take away",
    "",
    "```rust {#retry .share}",
    "fn retry(attempts: usize) -> bool {",
    "    attempts < 3",
    "}",
    "```",
    "",
  ].join("\n"),
};

/**
 * The figures, in bytes, and what each one protects.
 *
 * Gzipped where a server would gzip, because that is what a room downloads over
 * conference wifi. Raw where the number is about what was emitted rather than
 * what was sent.
 */
export const BUDGETS = [
  {
    name: "javascript a slide with no steps fetches",
    limit: 0,
    protects:
      "the half of the claim that was ever load-bearing. A finished slide asks for no module, " +
      "no bundle and no request — which is what makes it open from a USB stick and survive a " +
      "venue with no working wifi",
  },
  {
    name: "javascript inlined into a slide with no steps",
    // Raised from 1,600 for the one line that stops this running inside a
    // frame. The editor draws every slide in the outline as a live frame of
    // its own page, and without the guard a single position on the mirror
    // channel pulled every preview onto the same slide.
    limit: 1_700,
    protects:
      "the other half, which used to be zero. It was zero because such a slide could not be " +
      "advanced: no key, no link, and no listener for the presenter's mirror — see " +
      "`slidx_render::navigation`. A number rather than a sentence, because the pressure on " +
      "this one is a line at a time",
  },
  {
    name: "an audience slide, gzipped",
    limit: 9_000,
    protects:
      "what a room downloads per slide, on the wifi a venue has rather than the one it advertises",
  },
  {
    name: "everything an audience downloads, gzipped",
    limit: 56_000,
    protects:
      "the whole of what a room fetches for this deck — every page, and every stylesheet, " +
      "script and image those pages actually reference",
  },
  {
    name: "a shared snippet page, gzipped",
    limit: 4_000,
    protects:
      "the page a phone reaches from a QR code on a slide, over whatever signal a room has " +
      "in the ninety seconds before the speaker moves on",
  },
  {
    name: "the step runtime, gzipped",
    limit: 24_000,
    protects:
      "the one module a slide with steps waits for. It grows an import at a time, and nothing " +
      "else in the output is on the path between a keypress and the next stop",
  },
];

/**
 * What is deliberately outside every figure above.
 *
 * Each of these is emitted by a build and downloaded by nobody in the room: the
 * presenter view is one laptop, the print shell is a PDF nobody fetches over
 * the network, the rehearsal runtime belongs to the presenter view, and a
 * social card is fetched by a crawler once. Counting them would make the
 * numbers about the wrong reader, and a budget nobody can act on is a budget
 * that gets raised rather than met.
 */
export const NOT_THE_AUDIENCE = ["presenter", "print", "rehearsal", "og-", "og."];

/**
 * The script types a browser executes.
 *
 * Anything else in a `<script>` is data the page carries — every page slidx
 * emits has an `application/ld+json` block so a crawler can read the talk — and
 * counting data as code reports the no-JavaScript claim broken while it is
 * being kept. That is not hypothetical: it is what the first version of this
 * check did, on the very first run.
 */
const JAVASCRIPT = /^(?:module|text\/javascript|application\/javascript|text\/ecmascript)$/i;

/** Every `<script>` on a page that a browser would run, as `{ attributes, body }`. */
export function executableScripts(page) {
  return [...page.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/g)]
    .map(([, attributes, body]) => ({ attributes, body: body.trim() }))
    .filter(({ attributes }) => {
      const type = /\btype="([^"]+)"/.exec(attributes);
      return type === null || JAVASCRIPT.test(type[1]);
    });
}

/**
 * Every path a page asks a browser to fetch.
 *
 * Stylesheets, scripts and images by their attribute, and modules by their
 * `import` — because that last one is how the step runtime arrives and no other.
 * A staged slide carries an inline module whose first line imports it, so a
 * reader that follows only `src` misses the largest thing a room downloads and
 * reports a deck at a third of its weight.
 *
 * Absolute URLs and `data:` are left out: one is somebody else's server, which
 * the offline rule forbids and the linter catches, and the other is already
 * counted inside the page.
 */
export function referencesIn(page) {
  const found = [
    ...page.matchAll(/<link\b[^>]*\brel="stylesheet"[^>]*\bhref="([^"]+)"/g),
    ...page.matchAll(/<script\b[^>]*\bsrc="([^"]+)"/g),
    ...page.matchAll(/<img\b[^>]*\bsrc="([^"]+)"/g),
    ...page.matchAll(/\bimport\s[^;]*?from\s*["']([^"']+)["']/g),
    ...page.matchAll(/\bimport\(\s*["']([^"']+)["']\s*\)/g),
  ].map(([, reference]) => reference);

  return [...new Set(found.filter((reference) => !/^(?:https?:)?\/\/|^data:/.test(reference)))];
}

/**
 * The pages a room loads, split by which figure each belongs to.
 *
 * A snippet page is downloaded by the audience and is not a slide. It counts
 * towards what a room fetches, and it must stay out of the per-slide average —
 * it is a fraction of the weight of a slide, so averaging it in makes every
 * slide look lighter the more code a deck shares, which is backwards.
 */
export function splitPages(paths) {
  return {
    slides: paths.filter((path) => !path.includes("/snippets/")),
    snippets: paths.filter((path) => path.includes("/snippets/")),
  };
}

/** Which measurements are over, with the amount and the reason it matters. */
export function overBudget(measured, budgets = BUDGETS) {
  return budgets
    .filter((budget) => measured[budget.name] !== undefined)
    .filter((budget) => measured[budget.name] > budget.limit)
    .map((budget) => ({
      ...budget,
      measured: measured[budget.name],
      over: measured[budget.name] - budget.limit,
    }));
}

/** Budgets nothing measured, which is how a check quietly stops checking. */
export function unmeasured(measured, budgets = BUDGETS) {
  return budgets
    .filter((budget) => measured[budget.name] === undefined)
    .map((budget) => budget.name);
}
