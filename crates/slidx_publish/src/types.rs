//! What a target is given, and what it may answer.
//!
//! The input types mirror [`slidx_core`]'s `DeckMeta`, `TalkMeta`, and `Slide`,
//! flattened. Flattening is deliberate: a target cares whether it has a `url`,
//! not whether the url lives under `talk`, and a plain record is what a CLI, a
//! plugin, or a hand-written script can all produce without loading the parser.
//!
//! Every field is optional except the ones that make a deck a deck, because
//! absence is the normal case this crate exists to report on. A deck for an
//! internal brown bag has no event, no hashtag, and no url, and must still
//! plan — with the steps that need those fields reported as blocked rather than
//! quietly emitting a post that links nowhere.
//!
//! [`slidx_core`]: https://docs.rs/slidx_core

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use ts_rs::TS;

/// A deck's metadata, as the author wrote it at proposal time.
///
/// Nothing here is derived or defaulted. A field that is absent stays absent
/// all the way into the plan, where it becomes a named reason rather than a
/// guess.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct DeckMetadata {
    #[ts(optional)]
    pub title: Option<String>,
    #[ts(optional)]
    pub description: Option<String>,
    #[ts(optional)]
    pub author: Option<String>,
    /// Conference or meetup name.
    #[ts(optional)]
    pub event: Option<String>,
    /// ISO-8601 date, kept as text so a plan never depends on a clock.
    #[ts(optional)]
    pub date: Option<String>,
    #[ts(optional)]
    pub venue: Option<String>,
    /// Without the leading `#`, which is added back per platform.
    #[ts(optional)]
    pub hashtag: Option<String>,
    /// Canonical URL of the published deck.
    #[ts(optional)]
    pub url: Option<String>,
    /// The recording, once one exists.
    ///
    /// The only field here that is normally added weeks after the talk, which
    /// is what the archive target is built around.
    #[ts(optional)]
    pub recording: Option<String>,
    /// Repository, listed on the resources page.
    #[ts(optional)]
    pub repo: Option<String>,
    /// Author-chosen tags. Never reordered, never rewritten.
    #[ts(optional)]
    pub tags: Option<Vec<String>>,
    /// Explicit path segment for the upload targets.
    ///
    /// Present when the author has pinned one — usually because the derived
    /// slug would change under them when the title is edited, and a slide URL
    /// that moves after it has been shared is a broken link in someone's notes.
    #[ts(optional)]
    pub slug: Option<String>,
}

/// One slide, reduced to what publishing reads from it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct DeckSlide {
    /// Zero-based position. Fixes the order of everything derived per slide.
    pub index: u32,
    #[ts(optional)]
    pub title: Option<String>,
    /// Markdown body, as authored. Links are read out of it.
    #[ts(optional)]
    pub content: Option<String>,
    /// Speaker notes, in source order. The blog scaffold is made of these.
    #[ts(optional)]
    pub notes: Option<Vec<String>>,
}

/// A file the build produced, offered to the targets that need one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub kind: ArtifactKind,
    /// Path as the build reported it. Never opened by this crate.
    pub path: String,
    /// Size in bytes, when the caller measured it. Checked against upload caps.
    ///
    /// Crosses as a `number` rather than the `bigint` a 64-bit integer would
    /// otherwise become: what a caller has is `statSync(path).size`, and a
    /// boundary that demanded `4194304n` would be a boundary nobody could hand
    /// the answer they already had.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub bytes: Option<u64>,
}

/// The kinds of artifact a target asks for by name.
///
/// An enum rather than a free string so a typo in a caller is a type error, not
/// a step that reports the PDF as missing on a build that produced one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactKind {
    Pdf,
    Html,
    Card,
    Video,
}

impl ArtifactKind {
    /// The name a reason uses for this artifact — the frontmatter-shaped token
    /// an author acts on, not the Rust spelling.
    pub fn as_field(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Card => "card",
            Self::Video => "video",
        }
    }
}

/// Why a step cannot run, and what would fix it.
///
/// `field` is the thing to add — a frontmatter key, or the build output that is
/// missing. Naming it is the whole point: "add `url:` to the frontmatter" is
/// actionable at 11pm after a talk, "social post unavailable" is not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BlockedReason {
    pub field: String,
    pub message: String,
}

/// Names a missing or unusable field, and the fix.
pub fn reason(field: impl Into<String>, message: impl Into<String>) -> BlockedReason {
    BlockedReason { field: field.into(), message: message.into() }
}

/// A composed payload, or the reasons there is none.
///
/// Targets return this rather than failing. A plan is most useful when the deck
/// is least ready, so one missing field must not stop the other four targets
/// from being planned and printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Composed<T> {
    Ready(T),
    Blocked(Vec<BlockedReason>),
}

impl<T> Composed<T> {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Ready(value) => Some(value),
            Self::Blocked(_) => None,
        }
    }

    pub fn reasons(&self) -> &[BlockedReason] {
        match self {
            Self::Ready(_) => &[],
            Self::Blocked(reasons) => reasons,
        }
    }
}

/// The discriminated union JavaScript reads, spelled out.
///
/// Written by hand because serde has no representation for a boolean tag, and
/// `ok` has to be a boolean: it is what every consumer narrows on, and
/// `if (result.ok)` is the line that makes the payload's type known to the
/// compiler on the other side.
///
/// A struct rather than a map, which is the difference between an object and a
/// `Map` once this crosses into JavaScript through `serde_wasm_bindgen`. A
/// `Map` has no `.ok`, so every consumer would read `undefined` and take the
/// blocked branch of a result that was fine.
impl<T: Serialize> Serialize for Composed<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut composed = serializer.serialize_struct("Composed", 2)?;

        match self {
            Self::Ready(value) => {
                composed.serialize_field("ok", &true)?;
                composed.serialize_field("value", value)?;
            }
            Self::Blocked(reasons) => {
                composed.serialize_field("ok", &false)?;
                composed.serialize_field("reasons", reasons)?;
            }
        }

        composed.end()
    }
}

/// Everything the targets are composed from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct DeckSource {
    pub meta: DeckMetadata,
    pub slides: Vec<DeckSlide>,
    pub artifacts: Vec<Artifact>,
}

impl DeckSource {
    /// The first artifact of a kind, or nothing. Ordering is the caller's.
    pub fn artifact(&self, kind: ArtifactKind) -> Option<&Artifact> {
        self.artifacts.iter().find(|artifact| artifact.kind == kind)
    }

    /// The slides in deck order.
    ///
    /// Sorted on the way out rather than in place, so a caller's array is never
    /// reordered under it — a plan is a function of the deck and must not be a
    /// mutation of one.
    pub fn ordered_slides(&self) -> Vec<&DeckSlide> {
        let mut slides: Vec<&DeckSlide> = self.slides.iter().collect();
        slides.sort_by_key(|slide| slide.index);
        slides
    }
}

/// A trimmed value, or nothing at all — never an empty string.
pub fn text(value: Option<&String>) -> Option<&str> {
    value.map(|value| value.trim()).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck() -> DeckSource {
        DeckSource {
            slides: vec![
                DeckSlide { index: 2, ..DeckSlide::default() },
                DeckSlide { index: 0, ..DeckSlide::default() },
            ],
            artifacts: vec![
                Artifact { kind: ArtifactKind::Pdf, path: "a.pdf".into(), bytes: None },
                Artifact { kind: ArtifactKind::Card, path: "a.png".into(), bytes: None },
            ],
            ..DeckSource::default()
        }
    }

    #[test]
    fn an_artifact_is_found_by_the_kind_a_target_asks_for() {
        assert_eq!(deck().artifact(ArtifactKind::Card).map(|a| a.path.as_str()), Some("a.png"));
        assert!(deck().artifact(ArtifactKind::Video).is_none());
    }

    #[test]
    fn slides_come_back_in_deck_order_without_the_callers_order_being_touched() {
        // A plan is a function of the deck. Sorting the caller's array in place
        // would make planning twice differ from planning once.
        let source = deck();

        assert_eq!(source.ordered_slides().iter().map(|s| s.index).collect::<Vec<_>>(), [0, 2]);
        assert_eq!(source.slides[0].index, 2, "the caller's order survived");
    }

    #[test]
    fn a_ready_result_crosses_as_a_union_javascript_can_narrow_on() {
        let composed: Composed<&str> = Composed::Ready("payload");

        assert_eq!(serde_json::to_string(&composed).unwrap(), r#"{"ok":true,"value":"payload"}"#);
    }

    #[test]
    fn a_blocked_result_carries_reasons_rather_than_a_value() {
        let composed: Composed<&str> = Composed::Blocked(vec![reason("url", "add `url:`")]);

        assert_eq!(
            serde_json::to_string(&composed).unwrap(),
            r#"{"ok":false,"reasons":[{"field":"url","message":"add `url:`"}]}"#
        );
    }

    #[test]
    fn a_field_of_nothing_but_spaces_is_the_same_as_no_field_at_all() {
        // Frontmatter written as `title:` with nothing after it parses to an
        // empty string, and a deck titled "" is a deck with no title.
        assert_eq!(text(Some(&"  ".to_string())), None);
        assert_eq!(text(Some(&" a ".to_string())), Some("a"));
        assert_eq!(text(None), None);
    }
}
