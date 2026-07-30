//! Publishing, reachable from JavaScript.
//!
//! `@ubugeeei/slidx-publish` was a thousand lines of TypeScript that answered questions
//! [`slidx_publish`] also answers — what Speaker Deck will accept as a title,
//! which field to name when a post overflows, whether a missing recording is a
//! problem or merely a Tuesday. Two implementations of a cap is two answers,
//! and the one that is wrong is discovered by a platform rejecting an upload at
//! the end of a long day. So the TypeScript is a wrapper and this is the door
//! it comes through.
//!
//! # One function
//!
//! Every operation crosses through [`publish_call`]. Twenty-odd
//! `#[wasm_bindgen]` exports would be twenty-odd signatures to keep in step
//! across two languages; the set of operations is instead declared once, in
//! Rust, as [`slidx_publish::Call`], and a wrapper that misspells one is told
//! so by serde rather than by `undefined` turning up somewhere else.
//!
//! **Nothing performed here needs a credential.** Planning is a pure function
//! from deck to plan — no clock, no filesystem, no network — which is the
//! property that lets a plan be read before it is meant. See the crate docs of
//! [`slidx_publish`] for why there is deliberately no HTTP client anywhere
//! under this call.

use serde::Serialize;
use slidx_publish::{
    ArchiveRecord, Artifact, ArtifactKind, BlockedReason, BlogScaffold, BlogSection, Call,
    DeckLink, DeckMetadata, DeckSlide, DeckSource, DocswellUpload, PlanOptions, PublishPlan,
    PublishTarget, ReadyPayload, ResourcesPage, SocialOptions, SocialPost, SpeakerDeckUpload,
    TalkIndex, TalkIndexOptions,
};
use ts_rs::Config;
use wasm_bindgen::prelude::*;

use crate::declarations::push;

// Appended verbatim to the `.d.ts` wasm-bindgen writes, the same way the deck
// types are, so `@ubugeeei/slidx-wasm` ships one self-contained description of its own
// boundary.
#[wasm_bindgen(typescript_custom_section)]
const PUBLISH_TYPES: &str = include_str!("../publish.d.ts");

/// Plans, composes, and formats — whatever the request names.
///
/// Never fails on the *deck*: a deck with nothing filled in still plans, with
/// every step reported as blocked and each blocked step naming the frontmatter
/// key that would fix it. The only error is a request this module does not
/// have an operation for, which is a bug in the caller rather than a state a
/// deck can be in.
///
/// The answer is typed `unknown` rather than as a union of every payload: the
/// caller already knows which question it asked, and a union would make every
/// call site narrow past a dozen shapes it ruled out by construction.
#[wasm_bindgen(js_name = publishCall, unchecked_return_type = "unknown")]
pub fn publish_call(
    #[wasm_bindgen(unchecked_param_type = "PublishCall")] request: JsValue,
) -> Result<JsValue, JsError> {
    let call: Call = serde_wasm_bindgen::from_value(request)
        .map_err(|error| JsError::new(&format!("invalid publish call: {error}")))?;

    call.answer().serialize(&answers()).map_err(|error| JsError::new(&error.to_string()))
}

/// `None` crosses as `null`, not as `undefined`.
///
/// The two are the same to `?.` and different to `===`, and a field that is
/// *known to be nothing* is a different claim from one that was never written:
/// a link collected from the frontmatter belongs to the talk rather than to a
/// slide, and `slide: null` says so where a missing key would read as an
/// oversight. Fields that mean "absent" say it by being absent — they carry
/// `skip_serializing_if` and never reach this serializer at all.
fn answers() -> serde_wasm_bindgen::Serializer {
    serde_wasm_bindgen::Serializer::new().serialize_missing_as_null(true)
}

const HEADER: &str = "\
// Publishing a deck, as it crosses out of Rust.
//
// Generated from the Rust types by `vp run generate:types`. Editing this file
// is pointless: `cargo test -p slidx_wasm` compares it against the types it
// came from, and the types win.
";

/// The two declarations written by hand, because each is a *use* of the
/// generated types rather than a second description of one.
///
/// `Composed` is generic, and a generic union has no Rust type to generate it
/// from. `PublishStep` is narrower than the wire format: the payload of a ready
/// step is decided by its `target`, so writing the union out is what lets
/// `step.target === "speakerdeck"` tell TypeScript that `step.payload` is a
/// Speaker Deck upload. A Rust enum tagged on one field cannot say that, and a
/// caller that has to cast to reach a payload has a payload that is not typed.
const FOOTER: &str = r#"
/**
 * A composed payload, or the reasons there is none.
 *
 * Targets answer with this rather than failing. A plan is most useful when the
 * deck is least ready, so one missing field must not stop the other four
 * targets from being planned and printed.
 */
export type Composed<T> = { ok: true; value: T } | { ok: false; reasons: BlockedReason[] };

/**
 * One step of a plan: everything a destination needs, or the fields that would
 * unblock it.
 *
 * `summary` is one line for a printed plan. A blocked step's summary names the
 * fields, because that is what the author acts on.
 */
export type PublishStep =
  | { status: "ready"; target: "speakerdeck"; summary: string; payload: SpeakerDeckUpload }
  | { status: "ready"; target: "docswell"; summary: string; payload: DocswellUpload }
  | { status: "ready"; target: "social"; summary: string; payload: SocialPost }
  | { status: "ready"; target: "blog"; summary: string; payload: BlogScaffold }
  | { status: "ready"; target: "resources"; summary: string; payload: ResourcesPage }
  | { status: "ready"; target: "archive"; summary: string; payload: ArchiveRecord }
  | { status: "blocked"; target: PublishTarget; summary: string; reasons: BlockedReason[] };
"#;

/// Every publish declaration, in one file, in a stable order.
pub fn generate() -> String {
    let cfg = Config::default();
    let mut file = String::from(HEADER);

    // What a deck looks like on the way in.
    push::<DeckMetadata>(&mut file, &cfg);
    push::<DeckSlide>(&mut file, &cfg);
    push::<Artifact>(&mut file, &cfg);
    push::<ArtifactKind>(&mut file, &cfg);
    push::<DeckSource>(&mut file, &cfg);
    push::<BlockedReason>(&mut file, &cfg);

    // The plan.
    push::<PlanOptions>(&mut file, &cfg);
    push::<PublishPlan>(&mut file, &cfg);
    push::<PublishTarget>(&mut file, &cfg);
    push::<ReadyPayload>(&mut file, &cfg);

    // One payload per destination.
    push::<SpeakerDeckUpload>(&mut file, &cfg);
    push::<DocswellUpload>(&mut file, &cfg);
    push::<SocialOptions>(&mut file, &cfg);
    push::<SocialPost>(&mut file, &cfg);
    push::<BlogScaffold>(&mut file, &cfg);
    push::<BlogSection>(&mut file, &cfg);
    push::<ResourcesPage>(&mut file, &cfg);
    push::<ArchiveRecord>(&mut file, &cfg);

    // The resources page's input, and the index built from many records.
    push::<DeckLink>(&mut file, &cfg);
    push::<TalkIndex>(&mut file, &cfg);
    push::<TalkIndexOptions>(&mut file, &cfg);

    // The door itself, last, because it names everything above it.
    push::<Call>(&mut file, &cfg);

    file.push_str(FOOTER);
    file
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_publish::PUBLISH_TARGETS;

    /// The declarations as they were last generated and committed.
    const COMMITTED: &str = include_str!("../publish.d.ts");

    #[test]
    fn the_committed_publish_declarations_are_what_the_rust_types_generate() {
        crate::declarations::check_committed("publish.d.ts", COMMITTED, &generate());
    }

    #[test]
    fn the_hand_written_step_union_covers_every_destination_a_plan_lists() {
        // The one declaration that is not generated, and therefore the one that
        // can go stale. A destination added in Rust and forgotten here would
        // reach TypeScript as a step whose payload does not narrow.
        for target in PUBLISH_TARGETS {
            assert!(
                FOOTER.contains(&format!("target: \"{}\"", target.as_token())),
                "{} is planned but has no ready step in the declaration",
                target.as_token()
            );
        }
    }

    #[test]
    fn a_request_that_is_not_an_operation_is_refused_rather_than_answered() {
        // Reached from JavaScript only — the Rust enum makes it unspellable —
        // so the runtime check is what catches it there.
        let call = serde_json::from_str::<Call>(r#"{"op":"tweet"}"#);

        assert!(call.is_err());
    }

    #[test]
    fn a_deck_with_nothing_filled_in_still_plans() {
        // The whole reason a plan is worth printing: it is most useful when the
        // deck is least ready.
        let call: Call = serde_json::from_str(r#"{"op":"plan","meta":{}}"#).expect("a call");
        let answer = serde_json::to_value(call.answer()).expect("an answer");

        assert_eq!(answer["deck"], "Untitled deck");
        assert_eq!(answer["steps"].as_array().expect("steps").len(), PUBLISH_TARGETS.len());
    }
}
