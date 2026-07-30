//! Blocks: where a thing on a slide is, and what is said about it.
//!
//! These are the two operations direct manipulation is made of. Dragging a
//! block into a region is [`move_to`] carrying a region name; reordering inside
//! one is the same operation carrying none; and everything else an editor can
//! say about a whole block — a key so a step can address it, a style class, a
//! title — is [`set_attributes`].
//!
//! # Why placement travels with the move
//!
//! A drag is one gesture, and where a block *is* on a slide is two facts: which
//! region holds it, and where it sits among that region's blocks. An editor
//! that wrote those as two operations would make its primary gesture cost two
//! presses of undo to take back, which is the kind of detail that makes a tool
//! feel like it is fighting the person using it.
//!
//! # Why the layout is consulted
//!
//! A block that names no region lands in the layout's default one. So dropping
//! a block *into* the default region is written by taking its class away rather
//! than by writing one: the two say the same thing to the renderer, and only
//! one of them is a line in the diff. It is also what makes a drag out of a
//! region and back again cost nothing at all.
//!
//! Which class is a placement and which is styling cannot be decided from the
//! source: `.side` places a block and `.accent` colours one, and they are the
//! same shape of thing until something that knows the layout reads them. That
//! answer lives in `slidx_theme::layout`, and this asks it rather than keeping
//! a second copy.

use slidx_core::{find_blocks, parse_deck, Attributes, ByteSpan, DeckParseOptions, FoundBlock};
use slidx_theme::layout::{Layout, REGION_NAMES};

use crate::edit::EditBuilder;
use crate::op::{BlockRef, EditError, SlideRef};
use crate::source::DeckSource;

pub(crate) fn set_attributes(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    block: &BlockRef,
    attributes: &Attributes,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let found = find_blocks(body.slice(deck.source));
    let at = locate(&found, block)?;

    write_line(deck, body, &found[at], attributes, builder);
    Ok(())
}

pub(crate) fn move_to(
    deck: &DeckSource<'_>,
    options: &DeckParseOptions,
    slide: &SlideRef,
    block: &BlockRef,
    to: usize,
    region: Option<&str>,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let text = body.slice(deck.source);
    let found = find_blocks(text);
    let from = locate(&found, block)?;

    // A position past the last block lands it last, for the reason `AddStep`
    // gives about its own: the drop target below the last block is one past it,
    // and reaching it must not depend on the editor having counted the blocks
    // the same way this crate does.
    let to = to.min(found.len() - 1);

    let placed = region
        .map(|name| place(&found[from].block.attributes, name, &layout(deck, options, index)));

    if from == to {
        // The block did not move, so only what it says about itself can have
        // changed — and when that is unchanged too, this plans nothing at all,
        // which is what a drag that ends where it started has to cost.
        if let Some(attributes) = &placed {
            write_line(deck, body, &found[from], attributes, builder);
        }

        return Ok(());
    }

    let (low, high) = (from.min(to), from.max(to));
    let mut order: Vec<usize> = (low..=high).collect();
    let moved = order.remove(from - low);
    order.insert(to - low, moved);

    let mut rewritten = String::new();
    for (position, at) in order.iter().enumerate() {
        if position > 0 {
            rewritten.push_str(&gap(deck, text, &found, low + position - 1));
        }

        let attributes = (*at == from).then_some(placed.as_ref()).flatten();
        rewritten.push_str(&one(deck, text, &found[*at], attributes));
    }

    builder.replace(
        ByteSpan::new(outer(&found[low]).start, outer(&found[high]).end).shifted(body.start),
        rewritten,
    );

    Ok(())
}

/// The index a reference names.
fn locate(found: &[FoundBlock], block: &BlockRef) -> Result<usize, EditError> {
    let at = match block {
        BlockRef::Index(index) => (*index < found.len()).then_some(*index),
        BlockRef::Key(key) => found
            .iter()
            .position(|candidate| candidate.block.attributes.key.as_deref() == Some(key.as_str())),
    };

    at.ok_or_else(|| EditError::NoSuchBlock { block: block.clone() })
}

/// The bytes a block occupies, its attribute line included.
///
/// Body-local, because that is what [`find_blocks`] is given and what a move
/// rearranges.
fn outer(found: &FoundBlock) -> ByteSpan {
    let start = found.attribute_line.map_or(found.block.span.start, |line| line.start);

    ByteSpan::new(start, found.block.span.end.max(start))
}

/// One block as it should be written, with an attribute line substituted.
fn one(
    deck: &DeckSource<'_>,
    text: &str,
    found: &FoundBlock,
    attributes: Option<&Attributes>,
) -> String {
    match attributes {
        Some(attributes) => {
            format!("{}{}", line(attributes, deck.newline()), found.block.span.slice(text))
        }
        // Verbatim, so a block that only rode along in a reorder comes out of
        // it byte for byte — blank line between its attribute line and itself
        // included, if that is how the author wrote it.
        None => outer(found).slice(text).to_string(),
    }
}

/// What goes between two blocks after a move.
///
/// The gap already at this position is kept whenever it still separates, which
/// is what leaves an author's extra blank line where they put it. It separates
/// when it holds a blank line; a single newline only ends a block when an
/// attribute line follows it, and the block that follows one after a move is
/// not the block that did before.
fn gap(deck: &DeckSource<'_>, text: &str, found: &[FoundBlock], position: usize) -> String {
    let existing = &text[outer(&found[position]).end..outer(&found[position + 1]).start];

    if existing.matches('\n').count() >= 2 {
        existing.to_string()
    } else {
        deck.blank()
    }
}

/// The attribute line for a block, or nothing at all.
///
/// Empty attributes take the line away rather than leaving `{}` behind, which
/// is the rule a mark already holds: `[text]{}` is not something a person meant
/// to write, and neither is a group on a line of its own that says nothing.
fn line(attributes: &Attributes, newline: &str) -> String {
    if attributes.is_empty() {
        return String::new();
    }

    format!("{{{}}}{newline}", attributes.to_source())
}

fn write_line(
    deck: &DeckSource<'_>,
    body: ByteSpan,
    found: &FoundBlock,
    attributes: &Attributes,
    builder: &mut EditBuilder<'_>,
) {
    let text = line(attributes, deck.newline());

    match found.attribute_line {
        Some(span) => builder.replace(span.shifted(body.start), text),
        None => builder.insert(body.start + found.block.span.start, text),
    }
}

/// These attributes, placing the block in `region`.
///
/// The new class takes the position the old one had, so a block that read
/// `{.side .accent}` does not become `{.accent .main}` and turn a reorder into
/// a diff about styling.
fn place(attributes: &Attributes, region: &str, layout: &Layout) -> Attributes {
    let mut placed = attributes.clone();
    let at = placed.classes.iter().position(|class| is_region(class));

    placed.classes.retain(|class| !is_region(class));

    // A block that names no region already lands in the default one, so naming
    // it would be a line in the diff that changes nothing on the slide.
    if region != layout.fallback().name {
        placed.classes.insert(at.unwrap_or(0), region.to_string());
    }

    placed
}

/// True for a class that places a block rather than styling one.
///
/// Every region name any layout uses, not only this slide's: a block still
/// carrying `.side` after its slide moved to `layout: split` is an author who
/// changed the layout and left a class behind, and dropping that block
/// somewhere else has to take the stale class with it rather than leave two.
fn is_region(class: &str) -> bool {
    REGION_NAMES.contains(&class)
}

/// The layout a slide's regions come from.
fn layout(deck: &DeckSource<'_>, options: &DeckParseOptions, index: usize) -> Layout {
    parse_deck(deck.source, options)
        .slides
        .get(index)
        .map_or_else(slidx_theme::layout::default_layout, slidx_theme::layout::of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::EditOp;
    use crate::{apply, plan};

    fn edited(source: &str, op: &EditOp) -> String {
        apply(source, &DeckParseOptions::default(), op).expect("the fixture has what it names")
    }

    fn planned(source: &str, op: &EditOp) -> Result<crate::Edit, EditError> {
        plan(source, &DeckParseOptions::default(), op)
    }

    fn moved(slide: usize, block: usize, to: usize, region: Option<&str>) -> EditOp {
        EditOp::MoveBlock {
            slide: slide.into(),
            block: block.into(),
            to,
            region: region.map(String::from),
        }
    }

    fn attributed(block: usize, attributes: Attributes) -> EditOp {
        EditOp::SetBlockAttributes { slide: 0.into(), block: block.into(), attributes }
    }

    #[test]
    fn a_block_dropped_in_a_region_gains_the_class_that_names_it() {
        let source = "---\nlayout: split\n---\n\n# One\n\nBeside it.\n";
        let result = edited(source, &moved(0, 1, 1, Some("right")));

        assert_eq!(result, "---\nlayout: split\n---\n\n# One\n\n{.right}\nBeside it.\n");
    }

    #[test]
    fn a_block_dropped_back_in_the_default_region_loses_the_class_rather_than_swapping_it() {
        // `{.left}` on a `split` slide says what the block already does, so
        // writing it would be a line in the diff that changes nothing on the
        // slide — and a drag out and back would not come out where it started.
        let source = "---\nlayout: split\n---\n\n# One\n\n{.right}\nBeside it.\n";
        let result = edited(source, &moved(0, 1, 1, Some("left")));

        assert_eq!(result, "---\nlayout: split\n---\n\n# One\n\nBeside it.\n");
    }

    #[test]
    fn a_drag_that_ends_where_it_started_is_not_an_edit_at_all() {
        // The editor's undo stack ignores an empty inverse, so this is what
        // keeps a drag that changed nothing from costing a press of undo.
        let source = "---\nlayout: split\n---\n\n# One\n\n{.right}\nBeside it.\n";

        assert!(planned(source, &moved(0, 1, 1, Some("right"))).unwrap().is_empty());
        assert!(planned(source, &moved(0, 0, 0, Some("left"))).unwrap().is_empty());
    }

    #[test]
    fn a_block_that_moves_takes_its_region_with_it() {
        let source = "---\nlayout: split\n---\n\n# One\n\n{.right}\nBeside it.\n";
        let result = edited(source, &moved(0, 1, 0, None));

        assert_eq!(result, "---\nlayout: split\n---\n\n{.right}\nBeside it.\n\n# One\n");
    }

    #[test]
    fn reordering_inside_a_region_swaps_two_blocks_and_leaves_the_rest() {
        let source = "# One\n\nSecond.\n\nThird.\n\nFourth.\n";
        let result = edited(source, &moved(0, 2, 1, None));

        assert_eq!(result, "# One\n\nThird.\n\nSecond.\n\nFourth.\n");
    }

    #[test]
    fn a_move_past_the_last_block_lands_it_last() {
        // The drop target below the last block is one past it, and the editor
        // counted the blocks off a page it rendered a keystroke ago.
        let source = "# One\n\nSecond.\n\nThird.\n";

        assert_eq!(edited(source, &moved(0, 0, 99, None)), "Second.\n\nThird.\n\n# One\n");
    }

    #[test]
    fn a_reorder_keeps_the_blank_lines_the_author_left_between_the_blocks() {
        // Only the blocks named move. An operation that regularised the spacing
        // around them would put the whole slide in the diff.
        let source = "# One\n\n\n\nSecond.\n\nThird.\n";
        let result = edited(source, &moved(0, 1, 0, None));

        assert_eq!(result, "Second.\n\n\n\n# One\n\nThird.\n");
    }

    #[test]
    fn a_move_gives_two_blocks_a_blank_line_when_only_an_attribute_line_had_separated_them() {
        // `{.side}` ends the block above it, so a single newline was enough
        // before the move. After it the two would be one block, and the second
        // would be rendered as part of the first.
        let source = "---\nlayout: aside\n---\n\n# One\n{.side}\nBeside it.\n";
        let result = edited(source, &moved(0, 1, 0, None));

        assert_eq!(result, "---\nlayout: aside\n---\n\n{.side}\nBeside it.\n\n# One\n");
    }

    #[test]
    fn a_stale_class_from_another_layout_goes_with_the_block_rather_than_staying() {
        // The author changed `layout:` and left `.side` behind. Dropping the
        // block into a region this layout does have must not leave two
        // placements on one block for the renderer to choose between.
        let source = "---\nlayout: split\n---\n\n{.side}\n# One\n\nSecond.\n";
        let result = edited(source, &moved(0, 0, 0, Some("right")));

        assert_eq!(result, "---\nlayout: split\n---\n\n{.right}\n# One\n\nSecond.\n");
    }

    #[test]
    fn a_style_class_survives_a_block_changing_region() {
        // `.accent` is the theme's and says nothing about placement. A move
        // that dropped it would be a drag that silently restyled a block.
        let source = "---\nlayout: split\n---\n\n{.accent}\n# One\n\nSecond.\n";
        let result = edited(source, &moved(0, 0, 0, Some("right")));

        assert_eq!(result, "---\nlayout: split\n---\n\n{.right .accent}\n# One\n\nSecond.\n");
    }

    #[test]
    fn the_region_class_keeps_the_place_the_author_wrote_it_in() {
        let source = "---\nlayout: split\n---\n\n{.accent .left}\n# One\n\nSecond.\n";
        let result = edited(source, &moved(0, 0, 0, Some("right")));

        assert_eq!(result, "---\nlayout: split\n---\n\n{.accent .right}\n# One\n\nSecond.\n");
    }

    #[test]
    fn setting_attributes_on_a_block_that_had_none_writes_the_line_above_it() {
        let result =
            edited("# One\n\nSecond.\n", &attributed(1, Attributes::default().with_key("hero")));

        assert_eq!(result, "# One\n\n{#hero}\nSecond.\n");
    }

    #[test]
    fn setting_a_blocks_attributes_to_nothing_takes_the_line_away() {
        // `{}` is not something a person meant to write, so the last attribute
        // removed removes the line — the rule a mark already holds.
        let result = edited("{#hero .side}\n# One\n", &attributed(0, Attributes::default()));

        assert_eq!(result, "# One\n");
    }

    #[test]
    fn a_block_is_named_by_its_key_as_well_as_by_its_position() {
        let source = "# One\n\n{#hero}\nSecond.\n";
        let op = EditOp::MoveBlock { slide: 0.into(), block: "hero".into(), to: 0, region: None };

        assert_eq!(edited(source, &op), "{#hero}\nSecond.\n\n# One\n");
    }

    #[test]
    fn naming_a_block_the_slide_does_not_have_is_an_error_rather_than_a_panic() {
        // The editor sends operations built off a page it rendered a keystroke
        // ago, so a block that has since gone is ordinary traffic.
        let source = "# One\n";

        assert_eq!(
            planned(source, &moved(0, 9, 0, None)),
            Err(EditError::NoSuchBlock { block: 9.into() })
        );
        assert!(planned(source, &attributed(0, Attributes::default().with_class("side"))).is_ok());
        assert_eq!(
            planned(
                source,
                &EditOp::SetBlockAttributes {
                    slide: 0.into(),
                    block: "gone".into(),
                    attributes: Attributes::default(),
                }
            ),
            Err(EditError::NoSuchBlock { block: "gone".into() })
        );
    }

    #[test]
    fn a_block_moved_on_a_windows_file_is_written_with_windows_line_endings() {
        let source = "---\r\nlayout: split\r\n---\r\n\r\n# One\r\n\r\nSecond.\r\n";
        let result = edited(source, &moved(0, 1, 0, Some("right")));

        assert!(!result.contains("\n\n"), "an LF line was written into a CRLF file: {result:?}");
        assert!(result.contains("{.right}\r\nSecond."));
    }

    #[test]
    fn moving_a_block_leaves_every_other_slide_alone() {
        let source = "# One\n\nSecond.\n\n---\n\n# Two\n\nAlso second.\n";
        let result = edited(source, &moved(1, 1, 0, None));

        assert_eq!(result, "# One\n\nSecond.\n\n---\n\nAlso second.\n\n# Two\n");
    }

    #[test]
    fn a_block_inside_a_fence_is_code_and_moves_as_one_piece() {
        // A talk about slidx shows an attribute line on a slide, and a move
        // that read it as one would tear the code block in half.
        let source = "# One\n\n```md\n{.side}\n# Example\n```\n";
        let result = edited(source, &moved(0, 1, 0, None));

        assert_eq!(result, "```md\n{.side}\n# Example\n```\n\n# One\n");
    }
}
