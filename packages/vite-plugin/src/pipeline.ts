/**
 * The WebAssembly pipeline, loaded once per process.
 *
 * The module is instantiated lazily and kept: instantiation is the only slow
 * part, and a dev server that reinstantiated on every keystroke would spend
 * more time loading the compiler than running it.
 */

import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";

import init, {
  buildDeck,
  deckSummary,
  probeImage as probeImageBytes,
  lintMeasured as lintMeasuredDeck,
  type AssetSize,
  type BuildDeckOptions,
  type BuildResult,
  type DeckSummary,
  type Finding,
} from "@slidxjs/wasm";

import type { Measurement } from "./overflow";

let ready: Promise<void> | undefined;

/**
 * Instantiates the pipeline, at most once.
 *
 * The wasm bytes are read from disk rather than fetched. `@slidxjs/wasm` is
 * built for the web target so one artifact serves Node, bundlers, and the
 * browser; in Node that means handing over the bytes ourselves.
 */
export function ensureReady(): Promise<void> {
  ready ??= (async () => {
    const require = createRequire(import.meta.url);
    const path = require.resolve("@slidxjs/wasm/slidx_bg.wasm");
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

/**
 * Turns what a browser measured into findings.
 *
 * The judgement stays in Rust with every other rule: what counts as clipped,
 * what is browser rounding, and how a clipped slide is worded are one set of
 * decisions, and a copy of them here would be a second set that drifts.
 */
export async function lintMeasured(
  source: string,
  measured: Measurement[],
  options: Pick<BuildDeckOptions, "separator" | "theme">,
): Promise<Finding[]> {
  await ensureReady();

  return lintMeasuredDeck(source, measured, {
    theme: options.theme,
    separator: options.separator,
  });
}

/**
 * What changed between two versions of a deck, said in slides.
 *
 * The comparison stays in Rust beside the deck model, with the rest of the
 * judgement: `slidx save` writes this same sentence into a commit message, and
 * a second wording on this side would let a talk's history and its own record
 * of itself disagree about what an author did.
 *
 * `before` absent is the deck's first commit — there is nothing to compare
 * against, which is a different answer from an empty deck.
 */
export async function summarise(
  before: string | undefined,
  after: string,
  options: Pick<BuildDeckOptions, "separator">,
): Promise<DeckSummary> {
  await ensureReady();

  return deckSummary(before, after, { separator: options.separator });
}

export type { DeckSummary };

/**
 * The intrinsic size in an image's header, or `null`.
 *
 * Synchronous, and deliberately not exported from `assets.ts` directly: the
 * module is instantiated once per process and every caller has to wait for
 * that, so the wait lives here with the rest of the boundary.
 *
 * The `path` comes back empty — the parser is answering about bytes and knows
 * nothing about where they came from. The caller fills it in.
 */
export async function probeImageHeader(head: Uint8Array): Promise<AssetSize | null> {
  await ensureReady();
  return probeImageBytes(head);
}
