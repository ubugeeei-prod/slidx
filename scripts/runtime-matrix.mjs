/**
 * The same deck, built under whichever runtime is running this file.
 *
 * The roadmap asks for Node, Bun, and Deno to be *verified, not assumed*, and
 * the thing worth verifying is narrow: slidx's pipeline is a WebAssembly
 * module built for the web target, so every runtime has to be handed the bytes
 * itself rather than fetching them. That path — resolve the package, read the
 * file, instantiate — is the one place where the three runtimes genuinely
 * differ, and it is load-bearing for every build anyone runs.
 *
 * So this prints a digest of a complete build rather than a "it loaded" flag.
 * A runtime that instantiated the module but produced different HTML would
 * pass the weaker check and ship a different deck, which is the failure that
 * would actually reach a stage.
 *
 * Run it under each runtime and compare the last line. The slide and stop
 * counts are printed alongside so a mismatch says *what* differs rather than
 * only that something does.
 */

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/**
 * A deck that exercises the parts a runtime could plausibly break.
 *
 * Frontmatter, a step, an inline mark, a fenced block with a separator inside
 * it, and non-ASCII text — the last because a runtime that decoded the source
 * as anything but UTF-8 would produce a deck that renders mojibake, and a
 * digest is the only cheap way to notice.
 */
const DECK = `---
title: Runtime check
description: One deck, three runtimes.
theme: minimal
---

# Making Decks Fast

日本語のテキストも壊れないこと。

- first <!-- step -->
- [second]{#point .slidx-accent} <!-- step -->

\`\`\`rust
// --- not a slide separator
fn main() {}
\`\`\`
`;

/**
 * Resolved the way the Vite plugin resolves it.
 *
 * Package resolution is itself one of the things that differs between these
 * runtimes, so this deliberately goes through `createRequire` from the
 * plugin's own directory rather than reading a path out of the tree. A script
 * that hard-coded `packages/wasm/dist` would pass on a runtime where the real
 * plugin fails.
 */
const require = createRequire(pathToFileURL(join(root, "packages/vite-plugin/package.json")));

const { default: init, buildDeck } = await import(
  pathToFileURL(require.resolve("@slidxjs/wasm")).href
);

await init({
  module_or_path: await readFile(require.resolve("@slidxjs/wasm/slidx_bg.wasm")),
});

const result = buildDeck(DECK, { separator: "\n---\n", presenter: true, print: true });

const slides = result.slides.length;
const stops = result.slides.reduce((total, slide) => total + slide.stopCount, 0);

/**
 * Hashed over the rendered HTML, not over the whole result object.
 *
 * Key order in a structure that crossed the wasm boundary is not something
 * every runtime has to agree on, and it is not something a deck depends on.
 * The HTML is what ships, so the HTML is what has to match.
 */
const digest = createHash("sha256");
for (const slide of result.slides) {
  digest.update(slide.html ?? "");
  digest.update(slide.presenterHtml ?? "");
}
digest.update(result.printHtml ?? "");

process.stdout.write(`slides=${slides} stops=${stops}\n`);
process.stdout.write(`${digest.digest("hex")}\n`);
