/**
 * The one call into the Rust planner.
 *
 * Everything this package exports is a name for one operation on the other side
 * of the WebAssembly boundary. Nothing here decides anything: if a cap, an
 * order, or the wording of a reason appears in this package, it is in the wrong
 * place — `crates/slidx_publish` owns all of it, and owning it once is the
 * point.
 *
 * ## Why the module loads itself
 *
 * Every function this package exports is synchronous, because planning is: it
 * reads no clock, opens no socket, and waits for nothing. Making the API
 * asynchronous to cover the one load at the start would put an `await` in front
 * of a pure computation, in every caller, for ever.
 *
 * So the load happens once, here, at import. That is a side effect of importing
 * the package and it is declared as one — but it is also the only honest
 * reading of what an import means for a module that cannot answer a single
 * question until the planner exists.
 */

import initWasm, { publishCall, type PublishCall } from "@ubugeeei/slidx-wasm";

/**
 * Instantiates the planner.
 *
 * The wasm bytes are read from disk under Node and fetched in a browser.
 * `@ubugeeei/slidx-wasm` is built for the web target so one artifact serves both, which
 * under Node means handing over the bytes ourselves rather than depending on
 * how a given version resolves `fetch` against a file path.
 */
async function load(): Promise<void> {
  if (typeof process === "undefined" || process.release?.name !== "node") {
    await initWasm();
    return;
  }

  const [{ readFile }, { createRequire }] = await Promise.all([
    import("node:fs/promises"),
    import("node:module"),
  ]);
  const require = createRequire(import.meta.url);

  await initWasm({
    module_or_path: await readFile(require.resolve("@ubugeeei/slidx-wasm/slidx_bg.wasm")),
  });
}

await load();

/** Asks the planner one question. */
export function ask<T>(call: PublishCall): T {
  return publishCall(call) as T;
}

/** A deck as the planner takes it, with the parts a caller may leave out. */
export interface SourceInput {
  meta: DeckMetadata;
  slides?: readonly DeckSlide[];
  artifacts?: readonly Artifact[];
}

/**
 * Fills in what the caller left out.
 *
 * The Rust side defaults these too, but spelling them here keeps the payload
 * the same shape on every call, which is what makes a wasm boundary cheap to
 * reason about when something does not arrive.
 */
export function source(input: SourceInput): DeckSource {
  return {
    meta: input.meta,
    slides: [...(input.slides ?? [])],
    artifacts: [...(input.artifacts ?? [])],
  };
}

export type {
  ArchiveRecord,
  Artifact,
  ArtifactKind,
  BlockedReason,
  BlogScaffold,
  BlogSection,
  Composed,
  DeckLink,
  DeckMetadata,
  DeckSlide,
  DeckSource,
  DocswellUpload,
  PublishCall,
  PublishPlan,
  PublishStep,
  PublishTarget,
  ReadyPayload,
  ResourcesPage,
  SocialOptions,
  SocialPost,
  SpeakerDeckUpload,
  TalkIndex,
  TalkIndexOptions,
} from "@ubugeeei/slidx-wasm";

import type { Artifact, DeckMetadata, DeckSlide, DeckSource } from "@ubugeeei/slidx-wasm";
