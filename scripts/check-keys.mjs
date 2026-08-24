/**
 * The two key tables that ship, held to the same names.
 *
 * A slide with steps takes its keys from `packages/runtime/src/navigate.ts`,
 * and a slide without them takes them from `slidx_render::keys` — which also
 * renders the list a speaker reads on the presenter view. Those are different
 * languages, so no compiler can put them side by side, and the two things a
 * speaker would notice are both invisible in review:
 *
 * - **A key that works on one kind of slide and not the other.** Somebody adds
 *   `n` to the runtime and the deck stops answering it halfway through.
 * - **A list that names a key nothing binds**, which is worse than no list. A
 *   speaker who has been told `PageDown` works and finds it does not has
 *   learned that the panel is not to be trusted, and they learn it on a stage.
 *
 * It is a comparison rather than a generator on purpose. Generating the
 * TypeScript from the Rust would make one of them a build artefact, and this
 * repository commits generated files precisely so a change to a boundary
 * arrives in review as a diff — for two sets of nine strings, the diff *is* the
 * artefact, and a check is smaller than a pipeline.
 *
 * It reads the source rather than a build, so it runs before anything is
 * compiled and says the same thing in a clean checkout.
 */

import { readFileSync } from "node:fs";

const RUST = "crates/slidx_render/src/keys.rs";
const TYPESCRIPT = "packages/runtime/src/navigate.ts";

/**
 * Which command each runtime key set stands for.
 *
 * The runtime calls them by direction and the table by command; the mapping is
 * one line rather than a rename, because `next`/`previous` is what a help panel
 * says and `FORWARD`/`BACKWARD` is what a key handler reads.
 */
const SETS = { FORWARD: "next", BACKWARD: "previous" };

/** The keys `slidx_render::keys` binds to a command, in declaration order. */
function rustKeys(source, command) {
  const at = source.indexOf(`command: "${command}"`);
  if (at === -1) throw new Error(`${RUST} declares no binding for ${command}`);

  const clause = source.slice(at).match(/keys: &\[([^\]]*)\]/);
  if (!clause) throw new Error(`${RUST}'s ${command} binding declares no keys`);

  return [...clause[1].matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((match) => match[1]);
}

/** The keys a runtime set holds, in declaration order. */
function runtimeKeys(source, name) {
  const clause = source.match(new RegExp(`const ${name} = new Set\\(\\[([^\\]]*)\\]`));
  if (!clause) throw new Error(`${TYPESCRIPT} declares no ${name}`);

  return [...clause[1].matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((match) => match[1]);
}

const rust = readFileSync(RUST, "utf8");
const runtime = readFileSync(TYPESCRIPT, "utf8");

const problems = [];

for (const [set, command] of Object.entries(SETS)) {
  const declared = rustKeys(rust, command);
  const bound = runtimeKeys(runtime, set);

  for (const key of declared.filter((one) => !bound.includes(one))) {
    problems.push(
      `${RUST} lists ${JSON.stringify(key)} for ${command}, and ${TYPESCRIPT}'s ${set} does not bind it. ` +
        `A staged slide would not answer a key the presenter view says it does.`,
    );
  }

  for (const key of bound.filter((one) => !declared.includes(one))) {
    problems.push(
      `${TYPESCRIPT}'s ${set} binds ${JSON.stringify(key)} and ${RUST} does not list it for ${command}. ` +
        `An unstaged slide would not answer it, and no list would ever name it.`,
    );
  }
}

if (problems.length === 0) {
  const counted = Object.values(SETS).reduce(
    (total, command) => total + rustKeys(rust, command).length,
    0,
  );

  console.log(`keys: ${counted} movement keys, bound the same way by both halves of a deck.`);
  process.exit(0);
}

for (const problem of problems) console.error(`keys: ${problem}`);

console.error("");
console.error("keys: a deck has two key handlers — the runtime's on a staged slide, and the one");
console.error("  `slidx_render::navigation` inlines on every other. They are in two languages, so");
console.error("  nothing but this compares them, and a key that works on half a deck is a key a");
console.error("  speaker finds out about on a stage.");
process.exit(1);
