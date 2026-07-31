//! Copying a block, which is the one block operation that writes new bytes.
//!
//! Every other one in [`super`] rearranges or rewrites what an author already
//! typed. This adds a second copy of it — and doing that honestly is entirely
//! about the two things a copy must *not* carry over.
//!
//! # A copy has no key
//!
//! `{#result}` is a name, and two things on one slide cannot answer to one name.
//! A step that reveals `#result` would address whichever the compiler saw first,
//! silently, and an author who duplicated a block to make a second version of it
//! would find their animation pointing at the wrong one. So a copy keeps the
//! block's classes and its properties — its width, its colour, the region it is
//! in — and drops its key. Naming the copy is one field in the inspector, and it
//! is a decision the author has to make rather than one to inherit.
//!
//! # A copy lands next to its original
//!
//! Immediately after it, in the same region, because that is the only position
//! that needs no further gesture to be useful and the only one an author can
//! predict. Moving it somewhere else is [`super::move_to`], which already
//! exists — so this operation stays one idea, and the pair of them is still one
//! press of undo each.

use slidx_core::{find_blocks, Attributes, ByteSpan};

use crate::edit::EditBuilder;
use crate::op::{BlockRef, EditError, SlideRef};
use crate::source::DeckSource;

use super::{line, locate, outer};

pub(crate) fn duplicate(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    block: &BlockRef,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let text = body.slice(deck.source);
    let found = find_blocks(text);
    let at = locate(&found, block)?;

    let source = &found[at];
    let copied = named(&source.block.attributes);

    // The attribute line is written from the attributes rather than copied,
    // because the key has been taken out of them. A block that had nothing to
    // say about itself still has nothing to say, and `line` gives it no line at
    // all rather than an empty `{}`.
    let written = format!("{}{}", line(&copied, deck.newline()), source.block.span.slice(text));

    // Inserted at the end of the block it copies, with a blank line in front of
    // it: an author's own spacing further down the slide is not this
    // operation's to change, and the one gap being added is the one being
    // created.
    let end = outer(source).end;
    builder
        .replace(ByteSpan::new(end, end).shifted(body.start), format!("{}{written}", deck.blank()));

    Ok(())
}

/// The copy's attributes: everything the original said except its name.
fn named(attributes: &Attributes) -> Attributes {
    Attributes { key: None, ..attributes.clone() }
}

#[cfg(test)]
mod tests {
    use slidx_core::DeckParseOptions;

    use crate::op::{EditError, EditOp};
    use crate::{apply, plan};

    fn copied(slide: usize, block: usize) -> EditOp {
        EditOp::DuplicateBlock { slide: slide.into(), block: block.into() }
    }

    fn edited(source: &str, op: &EditOp) -> String {
        apply(source, &DeckParseOptions::default(), op).expect("the fixture has what it names")
    }

    #[test]
    fn a_copy_lands_immediately_after_the_block_it_copies() {
        let source = "# One\n\nA paragraph.\n\nAnother.\n";

        assert_eq!(
            edited(source, &copied(0, 1)),
            "# One\n\nA paragraph.\n\nA paragraph.\n\nAnother.\n"
        );
    }

    #[test]
    fn a_copy_keeps_what_the_block_says_about_itself_except_its_name() {
        // A step revealing `#result` would address whichever of the two the
        // compiler reached first, which is not a thing anybody can debug.
        let source = "---\nlayout: split\n---\n\n# One\n\n{#result .accent}\nIt worked.\n";

        assert_eq!(
            edited(source, &copied(0, 1)),
            "---\nlayout: split\n---\n\n# One\n\n{#result .accent}\nIt worked.\n\n{.accent}\nIt worked.\n"
        );
    }

    #[test]
    fn a_copy_of_a_block_that_says_nothing_gets_no_attribute_line() {
        // Rather than an empty `{}`, which is not something a person writes.
        let source = "# One\n\nA paragraph.\n";

        assert_eq!(edited(source, &copied(0, 1)), "# One\n\nA paragraph.\n\nA paragraph.\n");
    }

    #[test]
    fn a_copy_of_a_block_whose_only_attribute_was_its_name_loses_the_line_with_it() {
        let source = "# One\n\n{#result}\nIt worked.\n";

        assert_eq!(edited(source, &copied(0, 1)), "# One\n\n{#result}\nIt worked.\n\nIt worked.\n");
    }

    #[test]
    fn a_copy_of_a_code_block_comes_out_fence_and_all() {
        // The bytes are taken verbatim, so a fence, its language and its
        // indentation survive rather than being re-emitted by something that
        // has opinions about them.
        let source = "# One\n\n```rust\nlet x = 1;\n```\n";

        assert_eq!(
            edited(source, &copied(0, 1)),
            "# One\n\n```rust\nlet x = 1;\n```\n\n```rust\nlet x = 1;\n```\n"
        );
    }

    #[test]
    fn nothing_further_down_the_slide_moves() {
        // The author's own spacing below the copy is not this operation's to
        // normalise: only the gap it creates is written.
        let source = "# One\n\nFirst.\n\n\n\nFar below.\n";

        assert_eq!(edited(source, &copied(0, 1)), "# One\n\nFirst.\n\nFirst.\n\n\n\nFar below.\n");
    }

    #[test]
    fn the_last_block_can_be_copied() {
        let source = "# One\n\nThe last thing.\n";

        assert_eq!(edited(source, &copied(0, 1)), "# One\n\nThe last thing.\n\nThe last thing.\n");
    }

    #[test]
    fn a_block_on_another_slide_is_addressed_by_its_own_index() {
        let source = "# One\n\n---\n\n# Two\n\nSecond slide.\n";

        assert_eq!(
            edited(source, &copied(1, 1)),
            "# One\n\n---\n\n# Two\n\nSecond slide.\n\nSecond slide.\n"
        );
    }

    #[test]
    fn a_block_that_is_not_there_is_refused_rather_than_guessed_at() {
        let refused = plan("# One\n", &DeckParseOptions::default(), &copied(0, 4));

        assert!(matches!(refused, Err(EditError::NoSuchBlock { .. })));
    }

    #[test]
    fn a_slide_that_is_not_there_is_refused_too() {
        let refused = plan("# One\n", &DeckParseOptions::default(), &copied(9, 0));

        assert!(matches!(refused, Err(EditError::NoSuchSlide { .. })));
    }
}
