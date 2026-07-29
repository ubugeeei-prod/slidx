//! Marks: the editor's "select some words and style them" gesture.
//!
//! A selection on the canvas is a range of characters, and a mark is the only
//! thing in Markdown that can name one. So this is the operation the visual
//! editor leans on hardest — colouring a phrase, giving a fragment a key so a
//! step can animate it, changing a font for three words — and all of it is one
//! splice over the selected bytes.
//!
//! Ranges are measured in the slide's **source body**, which is what a text
//! selection maps onto and what the editor can slice for itself. A range that
//! is not in the body, or that would cut a character in half, is an error: the
//! editor sends ranges built from a deck it parsed a keystroke ago.

use slidx_core::mark::FoundMark;
use slidx_core::{find_marks, ByteSpan};

use crate::edit::EditBuilder;
use crate::op::{EditError, MarkAttributes, MarkRef, SlideRef};
use crate::source::DeckSource;

pub(crate) fn add(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    range: ByteSpan,
    attributes: &MarkAttributes,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let text = body.slice(deck.source);

    let selected = text.get(range.start..range.end).ok_or(EditError::UnusableRange { range })?;

    builder.replace(range.shifted(body.start), attributes.onto(selected).to_source());
    Ok(())
}

pub(crate) fn set(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    mark: &MarkRef,
    attributes: &MarkAttributes,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let found = locate(body.slice(deck.source), mark)?;

    // Attributes that are empty write the words back on their own. `[text]{}`
    // is not something a person meant, so removing the last class removes the
    // mark — which is also what makes `remove` the same operation.
    builder.replace(
        ByteSpan::new(found.start, found.end).shifted(body.start),
        attributes.onto(found.mark.text).to_source(),
    );

    Ok(())
}

pub(crate) fn remove(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    mark: &MarkRef,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    set(deck, slide, mark, &MarkAttributes::default(), builder)
}

fn locate(body: &str, mark: &MarkRef) -> Result<FoundMark, EditError> {
    let mut found = find_marks(body);

    let position = match mark {
        MarkRef::Index(index) => (*index < found.len()).then_some(*index),
        MarkRef::Key(key) => {
            found.iter().position(|candidate| candidate.mark.key.as_deref() == Some(key.as_str()))
        }
    };

    position
        .map(|at| found.swap_remove(at))
        .ok_or_else(|| EditError::NoSuchMark { mark: mark.clone() })
}
