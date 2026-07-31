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

mod block;
mod edit;
pub mod frontmatter;
mod inline;
mod media;
mod notes;
mod op;
mod slide;
mod source;
mod spans;
mod step;
mod style;
mod text;

pub use edit::{Edit, EditBuilder, Splice};
pub use media::MediaKind;
pub use op::{BlockRef, EditError, EditOp, MarkAttributes, MarkRef, SlideRef};
pub use spans::{slide_spans, BlockSpans, MarkSpans, SlideSpans};

use slidx_core::DeckParseOptions;

use source::DeckSource;

/// Works out which bytes an operation changes, without changing them.
pub fn plan(source: &str, options: &DeckParseOptions, op: &EditOp) -> Result<Edit, EditError> {
    let deck = DeckSource::read(source, options);
    let mut builder = EditBuilder::new(source);

    match op {
        EditOp::SetBody { slide, body } => slide::set_body(&deck, slide, body, &mut builder)?,
        EditOp::SetHeading { slide, text } => slide::set_heading(&deck, slide, text, &mut builder)?,
        EditOp::SetText { slide, range, text } => {
            text::set(&deck, slide, *range, text, &mut builder)?
        }
        EditOp::InsertSlide { at, body } => slide::insert(&deck, *at, body, &mut builder)?,
        EditOp::DuplicateSlide { slide, after } => {
            slide::duplicate(&deck, slide, after.as_ref(), &mut builder)?
        }
        EditOp::RemoveSlide { slide } => slide::remove(&deck, slide, &mut builder)?,
        EditOp::MoveSlide { slide, to } => slide::move_to(&deck, slide, *to, &mut builder)?,
        EditOp::SetField { slide, key, value } => {
            frontmatter::set_field(&deck, slide, key, value, &mut builder)?
        }
        EditOp::SetStyle { slide, property, value } => {
            style::set(&deck, slide, property, value.as_deref(), &mut builder)?
        }
        EditOp::SetBlockStyle { slide, block, property, value } => {
            if slidx_core::style::block_style_property("element", property).is_none() {
                return Err(EditError::InvalidStyleProperty { property: property.clone() });
            }

            let create = value.as_deref().is_some_and(|value| !value.trim().is_empty());
            let Some(key) = block::style_key(&deck, slide, block, create)? else {
                return Ok(builder.build());
            };
            let managed = slidx_core::style::block_style_property(&key.key, property)
                .expect("the property name was validated before resolving the block");

            // The style insertion is planned first. When the first block starts
            // at the same byte as a newly created style block, stable splice
            // ordering keeps the block above the attribute line it owns.
            style::set(&deck, slide, &managed, value.as_deref(), &mut builder)?;
            block::assign_style_key(&deck, slide, block, &key, &mut builder)?;
        }
        EditOp::AddMark { slide, range, attributes } => {
            inline::add(&deck, slide, *range, attributes, &mut builder)?
        }
        EditOp::SetMark { slide, mark, attributes } => {
            inline::set(&deck, slide, mark, attributes, &mut builder)?
        }
        EditOp::RemoveMark { slide, mark } => inline::remove(&deck, slide, mark, &mut builder)?,
        EditOp::SetBlockAttributes { slide, block, attributes } => {
            block::set_attributes(&deck, slide, block, attributes, &mut builder)?
        }
        EditOp::SetBlockWidth { slide, block, width } => {
            block::set_width(&deck, slide, block, *width, &mut builder)?
        }
        EditOp::DuplicateBlock { slide, block } => {
            block::duplicate(&deck, slide, block, &mut builder)?
        }
        EditOp::MoveBlock { slide, block, to, region } => {
            block::move_to(&deck, options, slide, block, *to, region.as_deref(), &mut builder)?
        }
        EditOp::InsertMedia { slide, at, kind, src, alt, region } => media::insert(
            &deck,
            options,
            slide,
            *at,
            media::Media { kind: *kind, src, alt },
            region.as_deref(),
            &mut builder,
        )?,
        EditOp::AddStep { slide, at, action } => {
            step::add(&deck, options, slide, *at, action, &mut builder)?
        }
        EditOp::RemoveStep { slide, index } => {
            step::remove(&deck, options, slide, *index, &mut builder)?
        }
        EditOp::MoveStep { slide, from, to } => {
            step::move_to(&deck, options, slide, *from, *to, &mut builder)?
        }
        EditOp::SetStep { slide, index, action } => {
            step::set(&deck, options, slide, *index, action, &mut builder)?
        }
        EditOp::AdoptSteps { slide } => step::adopt(&deck, options, slide, &mut builder)?,
        EditOp::SetNotes { slide, notes } => notes::set(&deck, slide, notes, &mut builder)?,
    }

    Ok(builder.build())
}

/// The source with one operation applied.
pub fn apply(source: &str, options: &DeckParseOptions, op: &EditOp) -> Result<String, EditError> {
    Ok(plan(source, options, op)?.apply(source))
}
