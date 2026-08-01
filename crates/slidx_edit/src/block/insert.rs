//! Adding common authored blocks without making the browser a Markdown writer.
//!
//! The visual editor names an intention — heading, text, list, or quote — and
//! this module owns the smallest useful source for it. That boundary matters:
//! if a button carried `## New heading`, the browser would be a second Markdown
//! serializer and its line endings and spacing would eventually disagree with
//! every other edit operation.

use slidx_core::find_blocks;

use crate::edit::EditBuilder;
use crate::op::{BlockKind, EditError, SlideRef};
use crate::source::DeckSource;

use super::outer;

pub(crate) fn insert(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    at: usize,
    kind: BlockKind,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let body_source = body.slice(deck.source);
    let found = find_blocks(body_source);
    let source = source(kind, deck.newline());

    if found.is_empty() {
        let prefix = if body_source.trim().is_empty() { String::new() } else { deck.blank() };
        builder.insert(body.end, format!("{prefix}{source}"));
        return Ok(());
    }

    if at >= found.len() {
        let end = outer(found.last().expect("the list is not empty")).end;
        builder.insert(body.start + end, format!("{}{source}", deck.blank()));
    } else {
        let start = outer(&found[at]).start;
        builder.insert(body.start + start, format!("{source}{}", deck.blank()));
    }

    Ok(())
}

/** The smallest useful authored form of each visual intention. */
fn source(kind: BlockKind, newline: &str) -> String {
    match kind {
        BlockKind::Heading => "## New heading".into(),
        BlockKind::Text => "Write something.".into(),
        BlockKind::List => format!("- First point{newline}- Second point"),
        BlockKind::Quote => "> Key takeaway".into(),
    }
}

#[cfg(test)]
mod tests {
    use slidx_core::DeckParseOptions;

    use crate::{apply, BlockKind, EditOp};

    fn inserted(source: &str, at: usize, kind: BlockKind) -> String {
        apply(
            source,
            &DeckParseOptions::default(),
            &EditOp::InsertBlock { slide: 0.into(), at, kind },
        )
        .expect("the fixture has the slide it names")
    }

    #[test]
    fn a_heading_lands_between_the_blocks_it_was_placed_between() {
        assert_eq!(
            inserted("# One\n\nLast.\n", 1, BlockKind::Heading),
            "# One\n\n## New heading\n\nLast.\n"
        );
    }

    #[test]
    fn text_appends_after_the_last_block() {
        assert_eq!(
            inserted("# One\n\nLast.\n", 99, BlockKind::Text),
            "# One\n\nLast.\n\nWrite something.\n"
        );
    }

    #[test]
    fn a_list_uses_the_decks_line_endings() {
        assert_eq!(
            inserted("# One\r\n", 1, BlockKind::List),
            "# One\r\n\r\n- First point\r\n- Second point\r\n"
        );
    }

    #[test]
    fn a_quote_can_be_the_first_content_on_an_empty_slide() {
        assert_eq!(inserted("", 0, BlockKind::Quote), "> Key takeaway");
    }

    #[test]
    fn source_outside_the_new_block_is_byte_identical() {
        let source = "#   One\n\n*  Hand-spaced\n\n\n\nFar below.\n";

        assert_eq!(
            inserted(source, 1, BlockKind::Text),
            "#   One\n\nWrite something.\n\n*  Hand-spaced\n\n\n\nFar below.\n"
        );
    }
}
