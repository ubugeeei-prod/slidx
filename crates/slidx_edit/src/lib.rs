//! # slidx edit
//!
//! The operation model the visual editor writes Markdown through.
//!
//! ## The law
//!
//! **An operation changes exactly what it names and leaves every other byte of
//! the source alone.**
//!
//! That rules out the obvious implementation. Parsing a deck to a model,
//! mutating the model, and serialising it back produces a file where the
//! author's blank lines have been regularised, their `*` bullets have become
//! `-`, their setext heading has become an ATX one, and their hand-wrapped
//! paragraph has become one long line. Every one of those is invisible in the
//! editor and enormous in the diff, and a diff nobody can review is the end of
//! the promise that a slidx deck is still Markdown you own.
//!
//! So an edit is a **byte-range splice into the original source**, and this
//! crate exists to compute the range. The model is how an operation finds its
//! bytes; it is never what gets written back.
//!
//! ```
//! use slidx_core::DeckParseOptions;
//! use slidx_edit::{apply, EditOp};
//!
//! let source = "---\ntitle: Fast Decks\n---\n\n#   Introduction\n\n- a\n- b\n";
//! let op = EditOp::SetHeading { slide: 0.into(), text: "Where decks go wrong".into() };
//!
//! assert_eq!(
//!     apply(source, &DeckParseOptions::default(), &op).unwrap(),
//!     "---\ntitle: Fast Decks\n---\n\n#   Where decks go wrong\n\n- a\n- b\n",
//! );
//! ```
//!
//! The three spaces after the `#` survive. Nothing in this crate is allowed to
//! have an opinion about them.
//!
//! ## What an operation is
//!
//! [`EditOp`] is a closed enum of plain data rather than a trait, because the
//! three things the editor needs next all want values:
//!
//! - **The WASM boundary.** The canvas is TypeScript, the splice is computed
//!   here, and only data crosses between them.
//! - **Undo.** [`Edit::invert`] turns an applied edit into the one that takes
//!   it back, so an undo stack is a list rather than a second document model.
//! - **Replay.** The animation timeline (M3) plays a deck forward through
//!   authored changes, which needs operations that can be stored and re-run.
//!
//! Only the first is exercised here. The other two are why the shape is what
//! it is, and adding an operation should stay a matter of adding a variant.
//!
//! ## Planning and applying
//!
//! [`plan`] returns the [`Edit`] — the byte ranges and their replacements —
//! without touching anything, which is what makes minimality testable and what
//! a preview pane wants. [`apply`] is `plan` followed by
//! [`Edit::apply`]. An operation that asks for what the source already says
//! plans an *empty* edit, so idempotence is a property of the crate rather
//! than of each operation.
//!
//! Nothing here fails loudly. An operation naming a slide that is no longer
//! there returns an [`EditError`], because the editor sends operations built
//! from a deck it parsed a keystroke ago and that race is ordinary traffic.

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

mod edit;
mod frontmatter;
mod inline;
mod notes;
mod op;
mod slide;
mod source;
mod step;

pub use edit::{Edit, EditBuilder, Splice};
pub use op::{EditError, EditOp, MarkAttributes, MarkRef, SlideRef};

use serde::{Deserialize, Serialize};

use slidx_core::{ByteSpan, DeckParseOptions};

use source::DeckSource;

/// Works out which bytes an operation changes, without changing them.
pub fn plan(source: &str, options: &DeckParseOptions, op: &EditOp) -> Result<Edit, EditError> {
    let deck = DeckSource::read(source, options);
    let mut builder = EditBuilder::new(source);

    match op {
        EditOp::SetBody { slide, body } => slide::set_body(&deck, slide, body, &mut builder)?,
        EditOp::SetHeading { slide, text } => slide::set_heading(&deck, slide, text, &mut builder)?,
        EditOp::InsertSlide { at, body } => slide::insert(&deck, *at, body, &mut builder)?,
        EditOp::RemoveSlide { slide } => slide::remove(&deck, slide, &mut builder)?,
        EditOp::MoveSlide { slide, to } => slide::move_to(&deck, slide, *to, &mut builder)?,
        EditOp::SetField { slide, key, value } => {
            frontmatter::set_field(&deck, slide, key, value, &mut builder)?
        }
        EditOp::AddMark { slide, range, attributes } => {
            inline::add(&deck, slide, *range, attributes, &mut builder)?
        }
        EditOp::SetMark { slide, mark, attributes } => {
            inline::set(&deck, slide, mark, attributes, &mut builder)?
        }
        EditOp::RemoveMark { slide, mark } => inline::remove(&deck, slide, mark, &mut builder)?,
        EditOp::AddStep { slide, action } => {
            step::add(&deck, options, slide, action, &mut builder)?
        }
        EditOp::RemoveStep { slide, index } => {
            step::remove(&deck, options, slide, *index, &mut builder)?
        }
        EditOp::SetNotes { slide, notes } => notes::set(&deck, slide, notes, &mut builder)?,
    }

    Ok(builder.build())
}

/// The source with one operation applied.
pub fn apply(source: &str, options: &DeckParseOptions, op: &EditOp) -> Result<String, EditError> {
    Ok(plan(source, options, op)?.apply(source))
}

/// Where one slide's bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideSpans {
    /// Everything the slide is, its own frontmatter block included.
    pub content: ByteSpan,
    /// The Markdown body alone, which is what a mark's range is measured in
    /// and what an editor puts in front of an author.
    pub body: ByteSpan,
}

/// Where each slide's own bytes are, in source order.
///
/// A deck is usually stored as one file per slide and edited as one joined
/// source, so whoever writes the source back to disk has to know which file
/// each byte came from. These spans are that map: they are the same ones an
/// operation splices into, so a caller cutting the result along them is cutting
/// where the operations already agreed the seams are.
pub fn slide_spans(source: &str, options: &DeckParseOptions) -> Vec<SlideSpans> {
    DeckSource::read(source, options)
        .slides
        .iter()
        .map(|slide| SlideSpans { content: slide.content, body: slide.body })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(source: &str) -> Vec<&str> {
        slide_spans(source, &DeckParseOptions::default())
            .iter()
            .map(|slide| slide.content.slice(source))
            .collect()
    }

    fn bodies(source: &str) -> Vec<&str> {
        slide_spans(source, &DeckParseOptions::default())
            .iter()
            .map(|slide| slide.body.slice(source))
            .collect()
    }

    #[test]
    fn a_slides_span_is_the_bytes_an_operation_would_splice() {
        assert_eq!(content("# One\n\n---\n\n# Two\n\nBody.\n"), ["# One", "# Two\n\nBody."]);
    }

    #[test]
    fn a_slide_carrying_its_own_frontmatter_spans_the_block_too() {
        // The `---` opening the block is the same line that ends the slide
        // before, so a caller cutting on these spans keeps the two apart.
        let source = "# One\n\n---\nlayout: split\n---\n\n# Two\n";
        assert_eq!(content(source), ["# One", "---\nlayout: split\n---\n\n# Two"]);
    }

    #[test]
    fn a_slides_body_leaves_its_frontmatter_behind() {
        // A selection in the canvas is a range in the body, never in the block
        // of keys above it.
        let source = "# One\n\n---\nlayout: split\n---\n\n# Two\n";
        assert_eq!(bodies(source), ["# One", "# Two"]);
    }

    #[test]
    fn the_decks_own_frontmatter_belongs_to_no_slides_span() {
        assert_eq!(content("---\ntitle: T\n---\n\n# One\n"), ["# One"]);
    }
}
