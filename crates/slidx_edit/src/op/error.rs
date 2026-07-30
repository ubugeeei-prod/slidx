//! What an operation named that the source does not have.
//!
//! Every one of these is a value rather than a panic, and that is the whole
//! reason they are worth a module of their own: the editor sends operations
//! built from a deck it parsed a keystroke ago, so a slide that has since been
//! deleted, a block that has since moved, and a range measured against bytes
//! that are no longer there are all *ordinary traffic*. A crate that panicked on
//! them would be a crate the editor had to guess ahead of.
//!
//! Each one says which thing was missing, in the words the editor shows an
//! author, because the answer is almost always "re-read the deck and try again"
//! and the sentence has to be enough to tell that from a real mistake.

use serde::{Deserialize, Serialize};

use slidx_core::ByteSpan;

use crate::op::{BlockRef, MarkRef, SlideRef};

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
    NoSuchBlock {
        block: BlockRef,
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
    /// A range that is not inside the slide's body, or that would cut a
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
            Self::NoSuchBlock { block } => match block {
                BlockRef::Index(index) => write!(formatter, "the slide has no block {index}"),
                BlockRef::Key(key) => write!(formatter, "the slide has no block `#{key}`"),
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

    #[test]
    fn every_error_says_which_thing_was_missing() {
        let errors = [
            EditError::NoSuchSlide { slide: 7.into() },
            EditError::NoSuchSlide { slide: "gone".into() },
            EditError::NoSuchMark { mark: 3.into() },
            EditError::NoSuchMark { mark: "hero".into() },
            EditError::NoSuchBlock { block: 3.into() },
            EditError::NoSuchBlock { block: "hero".into() },
            EditError::NoSuchStep { index: 4, present: 2 },
            EditError::NoSuchPosition { at: 9, slides: 3 },
            EditError::UnusableRange { range: ByteSpan::new(1, 2) },
        ];

        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn an_error_crosses_the_boundary_naming_itself_and_what_it_could_not_find() {
        // The editor shows the sentence and decides whether to re-read the deck,
        // so both halves have to survive the trip.
        let error = EditError::NoSuchBlock { block: "hero".into() };
        let json = serde_json::to_value(&error).unwrap();

        assert_eq!(json.get("error").and_then(|kind| kind.as_str()), Some("noSuchBlock"));
        assert_eq!(serde_json::from_value::<EditError>(json).unwrap(), error);
    }
}
