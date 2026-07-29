/**
 * What a target is given, and what it may answer.
 *
 * Every type here is generated from the Rust it describes — see
 * `crates/slidx_wasm/publish.d.ts` — so a field added to the planner arrives in
 * TypeScript as a compile error rather than as `undefined` somewhere
 * downstream.
 *
 * The shapes mirror `DeckMeta`, `TalkMeta`, and `Slide` in `slidx_core`,
 * flattened. Flattening is deliberate: a target cares whether it has a `url`,
 * not whether the url lives under `talk`, and a plain object is what a CLI, a
 * plugin, or a hand-written script can all produce without loading the parser.
 *
 * Every field is optional except the ones that make a deck a deck, because
 * absence is the normal case this package exists to report on. A deck for an
 * internal brown bag has no event, no hashtag, and no url, and must still
 * plan — with the steps that need those fields reported as blocked rather than
 * quietly emitting a post that links nowhere.
 */

export type {
  Artifact,
  ArtifactKind,
  BlockedReason,
  Composed,
  DeckMetadata,
  DeckSlide,
  DeckSource,
} from "./boundary";
