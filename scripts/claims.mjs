/**
 * Claims this project is not allowed to make about itself.
 *
 * Not a style rule. Each entry here is a sentence that was written, believed,
 * read by somebody who checked it, and found to be wider than the thing that
 * proves it. Writing it again is easy — it is the shorter sentence, and it is
 * the one that sounds better — so the correction is kept next to the phrase
 * rather than in a review comment nobody will find in six months.
 *
 * The rule for adding one: it has to have been *wrong*, and there has to be a
 * true sentence to write instead. A phrase that is merely clumsy does not
 * belong here, and neither does one whose replacement is "say less".
 */

import { execFileSync } from "node:child_process";

/**
 * Where a claim about slidx can be made.
 *
 * Prose and doc comments, which is where these sentences live: a README, a
 * documentation page, a module header, the text an agent is handed. Test files
 * are excluded because proving one of these requires writing the wrong version
 * down — `scripts/test/claims.test.mjs` is made of exactly that — and this file
 * is excluded because it is the one that names them.
 */
const READABLE = /\.(md|rs|ts|tsx|mjs)$/;
const TESTS = /(^|\/)tests?\/|\.(test|spec)\.[cm]?[jt]sx?$/;
export const EXEMPT = ["scripts/claims.mjs"];

/**
 * The phrases, why each one is wrong, and what is true instead.
 *
 * `zero network requests` was in the README for months, in three crates and on
 * the documentation page for the night before a talk. A reader asked the
 * obvious question — *what about images?* — and they were right: a deck with
 * three images requests three images, and always will. What the browser matrix
 * actually measures is that nothing is requested from another origin, which is
 * both true and the thing anybody cares about, because the failure being
 * promised against is a venue with no working Wi-Fi.
 */
export const CLAIMS = [
  {
    phrase: "zero network requests",
    wrong: "a deck with three images requests three images, and always will",
    instead: "asks nothing of anywhere but itself, or fetches nothing from another origin",
  },
  {
    phrase: "no network requests",
    wrong: "same claim, same reason: its own assets are still requests",
    instead: "asks nothing of anywhere but itself, or fetches nothing from another origin",
  },
];

export function readableFiles() {
  return execFileSync("git", ["ls-files", "-z"], { encoding: "utf8" })
    .split("\0")
    .filter(Boolean)
    .filter((file) => READABLE.test(file))
    .filter((file) => !TESTS.test(file))
    .filter((file) => !EXEMPT.includes(file));
}

/** Every line of `text` that makes one of the claims. */
export function overstatements(text, claims = CLAIMS) {
  return text.split("\n").flatMap((line, index) => {
    const lowered = line.toLowerCase();

    return claims
      .filter((claim) => lowered.includes(claim.phrase))
      .map((claim) => ({ line: index + 1, text: line.trim(), claim }));
  });
}
