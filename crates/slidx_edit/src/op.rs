//! The operation set, and how an operation names what it changes.
//!
//! # Why an enum rather than a trait
//!
//! Three things the editor will need are cheap for a closed set of data and
//! expensive for an open set of behaviours:
//!
//! - **Crossing the WASM boundary.** The editor's UI is TypeScript and the
//!   splice is computed in Rust. A trait object cannot be posted between them;
//!   `{ "op": "setHeading", "slide": 0, "text": "…" }` can.
//! - **Undo.** An operation and its [`Edit`](crate::Edit) are both values, so
//!   an undo stack is a list rather than a second model of the document.
//! - **Replay.** The animation timeline in M3 plays a deck forward through
//!   authored changes. That needs operations that can be stored, inspected,
//!   and re-run — which is to say, data.
//!
//! None of the three is built here. They are why the set is closed.
//!
//! # Naming a slide
//!
//! By index or by id, and both are honest about what they are for. An index is
//! what a canvas click produces and is stable for exactly one operation. An id
//! is the slug in the URL, and survives everything except retitling the slide.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use slidx_core::{ByteSpan, Mark, StepAction};

/// One change to a deck source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "op")]
pub enum EditOp {
    /// Replaces a slide's Markdown, leaving its frontmatter and notes alone.
    SetBody {
        slide: SlideRef,
        body: String,
    },
    /// Retitles a slide, keeping the heading level the author chose. A slide
    /// without a heading gains one above its body.
    SetHeading {
        slide: SlideRef,
        text: String,
    },
    /// Adds a slide at `at`, pushing the slide currently there down.
    InsertSlide {
        at: usize,
        body: String,
    },
    RemoveSlide {
        slide: SlideRef,
    },
    /// Moves a slide to position `to`, counted after the slide is lifted out.
    MoveSlide {
        slide: SlideRef,
        to: usize,
    },
    /// Writes a frontmatter key. The deck's own frontmatter is the first
    /// slide's, which is what the parser already believes.
    SetField {
        slide: SlideRef,
        key: String,
        value: JsonValue,
    },
    /// Wraps a range of a slide's body in a mark. `range` is measured in the
    /// slide's source body, which is what a text selection maps to.
    AddMark {
        slide: SlideRef,
        range: ByteSpan,
        attributes: MarkAttributes,
    },
    /// Rewrites a mark's attributes, leaving its text alone. Attributes that
    /// are empty unwrap the mark, because `[text]{}` is not something a person
    /// meant to write.
    SetMark {
        slide: SlideRef,
        mark: MarkRef,
        attributes: MarkAttributes,
    },
    RemoveMark {
        slide: SlideRef,
        mark: MarkRef,
    },
    /// Appends an action to the slide's `steps:` list, creating it if needed.
    AddStep {
        slide: SlideRef,
        action: StepAction,
    },
    RemoveStep {
        slide: SlideRef,
        index: usize,
    },
    /// Replaces everything the speaker says over this slide. An empty string
    /// removes the notes.
    SetNotes {
        slide: SlideRef,
        notes: String,
    },
}

/// Which slide an operation is about.
///
/// Untagged, so JavaScript sends `0` or `"introduction"` rather than a wrapper
/// object it would have to remember the shape of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SlideRef {
    /// Zero-based position. What a click on the outline produces.
    Index(usize),
    /// The slug, as it appears in a deep link.
    Id(String),
}

impl From<usize> for SlideRef {
    fn from(index: usize) -> Self {
        Self::Index(index)
    }
}

impl From<&str> for SlideRef {
    fn from(id: &str) -> Self {
        Self::Id(id.to_string())
    }
}

/// Which mark on a slide an operation is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MarkRef {
    /// Zero-based position in source order, matching [`Slide::marks`].
    ///
    /// [`Slide::marks`]: slidx_core::Slide::marks
    Index(usize),
    /// The mark's `#key`. Stable, but only marks that something refers to
    /// have one.
    Key(String),
}

impl From<usize> for MarkRef {
    fn from(index: usize) -> Self {
        Self::Index(index)
    }
}

impl From<&str> for MarkRef {
    fn from(key: &str) -> Self {
        Self::Key(key.to_string())
    }
}

/// A mark without its text: what the inspector panel holds.
///
/// Separate from [`Mark`] because the text is not the editor's to supply. It
/// is whatever the author selected, and an operation that carried both would
/// be able to disagree with itself about which of the two won.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MarkAttributes {
    pub key: Option<String>,
    pub classes: Vec<String>,
    pub properties: BTreeMap<String, String>,
}

impl MarkAttributes {
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.classes.push(class.into());
        self
    }

    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    /// True when the mark would render as plain text and should be unwrapped.
    pub fn is_empty(&self) -> bool {
        self.key.is_none() && self.classes.is_empty() && self.properties.is_empty()
    }

    /// These attributes applied to a selection.
    pub fn onto(&self, text: impl Into<String>) -> Mark {
        Mark {
            text: text.into(),
            key: self.key.clone(),
            classes: self.classes.clone(),
            properties: self.properties.clone(),
        }
    }
}

impl From<&Mark> for MarkAttributes {
    fn from(mark: &Mark) -> Self {
        Self {
            key: mark.key.clone(),
            classes: mark.classes.clone(),
            properties: mark.properties.clone(),
        }
    }
}

/// An operation that names something the source does not have.
///
/// Every one of these is a value rather than a panic. The editor sends
/// operations built from a deck it parsed a keystroke ago, so naming a slide
/// that has since been deleted is ordinary traffic, not a bug to crash on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "error")]
pub enum EditError {
    NoSuchSlide {
        slide: SlideRef,
    },
    NoSuchMark {
        mark: MarkRef,
    },
    NoSuchStep {
        index: usize,
        present: usize,
    },
    /// A slide position outside `0..=slides`. Inserting *at* `slides` appends,
    /// which is why the range is inclusive at the top.
    NoSuchPosition {
        at: usize,
        slides: usize,
    },
    /// A mark range that is not inside the slide's body, or that would cut a
    /// character in half.
    UnusableRange {
        range: ByteSpan,
    },
}

impl std::fmt::Display for EditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchSlide { slide } => match slide {
                SlideRef::Index(index) => write!(formatter, "there is no slide at index {index}"),
                SlideRef::Id(id) => write!(formatter, "there is no slide with the id `{id}`"),
            },
            Self::NoSuchMark { mark } => match mark {
                MarkRef::Index(index) => write!(formatter, "the slide has no mark {index}"),
                MarkRef::Key(key) => write!(formatter, "the slide has no mark `#{key}`"),
            },
            Self::NoSuchStep { index, present } => {
                write!(formatter, "the slide declares {present} steps, so there is no step {index}")
            }
            Self::NoSuchPosition { at, slides } => {
                write!(formatter, "{at} is outside a deck of {slides} slides")
            }
            Self::UnusableRange { range } => {
                write!(
                    formatter,
                    "bytes {}..{} are not a selection in this slide",
                    range.start, range.end
                )
            }
        }
    }
}

impl std::error::Error for EditError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_operation_crosses_the_boundary_as_plain_json() {
        let op = EditOp::SetHeading { slide: 2.into(), text: "Fast Decks".into() };

        assert_eq!(
            serde_json::to_value(&op).unwrap(),
            json!({ "op": "setHeading", "slide": 2, "text": "Fast Decks" })
        );
        assert_eq!(
            serde_json::from_value::<EditOp>(serde_json::to_value(&op).unwrap()).unwrap(),
            op
        );
    }

    #[test]
    fn a_slide_is_named_by_a_bare_number_or_a_bare_string() {
        let by_id: EditOp =
            serde_json::from_value(json!({ "op": "removeSlide", "slide": "intro" })).unwrap();

        assert_eq!(by_id, EditOp::RemoveSlide { slide: SlideRef::Id("intro".into()) });
    }

    #[test]
    fn attributes_take_their_text_from_the_selection_rather_than_carrying_one() {
        let attributes = MarkAttributes::default().with_key("hero").with_class("accent");
        assert_eq!(attributes.onto("three words").to_source(), "[three words]{#hero .accent}");
    }

    #[test]
    fn attributes_read_back_off_a_mark_unchanged() {
        let mark = Mark::new("x").with_key("a").with_class("b").with_property("c", "d");
        assert_eq!(MarkAttributes::from(&mark).onto("x"), mark);
    }

    #[test]
    fn empty_attributes_are_the_ones_that_unwrap_a_mark() {
        assert!(MarkAttributes::default().is_empty());
        assert!(!MarkAttributes::default().with_class("a").is_empty());
    }

    #[test]
    fn every_error_says_which_thing_was_missing() {
        let errors = [
            EditError::NoSuchSlide { slide: 7.into() },
            EditError::NoSuchSlide { slide: "gone".into() },
            EditError::NoSuchMark { mark: 3.into() },
            EditError::NoSuchMark { mark: "hero".into() },
            EditError::NoSuchStep { index: 4, present: 2 },
            EditError::NoSuchPosition { at: 9, slides: 3 },
            EditError::UnusableRange { range: ByteSpan::new(1, 2) },
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
