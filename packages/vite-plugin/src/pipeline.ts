/**
 * The WebAssembly pipeline, loaded once per process.
 *
 * The module is instantiated lazily and kept: instantiation is the only slow
 * part, and a dev server that reinstantiated on every keystroke would spend
 * more time loading the compiler than running it.
 */

import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";

import init, { buildDeck, type BuildDeckOptions, type BuildResult } from "@slidx/wasm";

let ready: Promise<void> | undefined;

/**
 * Instantiates the pipeline, at most once.
 *
 * The wasm bytes are read from disk rather than fetched. `@slidx/wasm` is
 * built for the web target so one artifact serves Node, bundlers, and the
 * browser; in Node that means handing over the bytes ourselves.
 */
export function ensureReady(): Promise<void> {
  ready ??= (async () => {
    const require = createRequire(import.meta.url);
    const path = require.resolve("@slidx/wasm/slidx_bg.wasm");
    await init({ module_or_path: await readFile(path) });
  })();

  return ready;
}

export type { BuildDeckOptions };

/**
 * Parses, lints, and renders a deck.
 *
 * The options go straight through. They used to be restated here field by
 * field, which is how the plugin ended up describing a payload it does not
 * own; `BuildDeckOptions` is generated from the Rust struct, so the only thing
 * left to do with it is pass it on. Every default lives in one place, on the
 * side that acts on it.
 */
export async function build(source: string, options: BuildDeckOptions): Promise<BuildResult> {
  await ensureReady();

  return buildDeck(source, options);
}
