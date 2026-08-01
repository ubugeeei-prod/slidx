//! Removing one authored block without rewriting any neighbour.
//!
//! A block includes its attribute line and any invisible marker or note the
//! source parser attached to it. Deleting less would leave a region class or a
//! step anchor pointing at the next block; deleting more would make a local
//! gesture reformat an author's surrounding Markdown.

use slidx_core::{find_blocks, ByteSpan};

use crate::edit::EditBuilder;
use crate::op::{BlockRef, EditError, SlideRef};
use crate::source::DeckSource;

use super::{locate, outer};

pub(crate) fn remove(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    block: &BlockRef,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let found = find_blocks(body.slice(deck.source));
    let at = locate(&found, block)?;
    let target = outer(&found[at]);

    // For every block except the first, the separator before it leaves with it.
    // The separator after it then becomes the gap between the surviving
    // neighbours, byte for byte. The first has no previous neighbour, so its
    // following separator leaves instead and the next block becomes the start.
    let span = if at == 0 && found.len() > 1 {
        ByteSpan::new(target.start, outer(&found[1]).start)
    } else if at > 0 {
        ByteSpan::new(outer(&found[at - 1]).end, target.end)
    } else {
        target
    };

    builder.delete(span.shifted(body.start));
    Ok(())
}

#[cfg(test)]
mod tests {
    use slidx_core::DeckParseOptions;

    use crate::op::{EditError, EditOp};
    use crate::{apply, plan};

    fn removed(source: &str, slide: usize, block: usize) -> String {
        apply(
            source,
            &DeckParseOptions::default(),
            &EditOp::RemoveBlock { slide: slide.into(), block: block.into() },
        )
        .expect("the fixture has what it names")
    }

    #[test]
    fn a_middle_block_leaves_the_following_gap_between_its_neighbours() {
        let source = "# One\n\nRemove me.\n\n\n\nKeep me.\n";

        assert_eq!(removed(source, 0, 1), "# One\n\n\n\nKeep me.\n");
    }

    #[test]
    fn the_first_block_takes_its_following_separator() {
        assert_eq!(removed("# Remove\n\nKeep.\n", 0, 0), "Keep.\n");
    }

    #[test]
    fn the_last_block_takes_the_separator_before_it() {
        assert_eq!(removed("# Keep\n\nRemove.\n", 0, 1), "# Keep\n");
    }

    #[test]
    fn an_attribute_line_and_hidden_marker_leave_with_the_block() {
        let source = "# Keep\n\n{#gone .right}\nRemove.\n<!-- step -->\n\nAfter.\n";

        assert_eq!(removed(source, 0, 1), "# Keep\n\nAfter.\n");
    }

    #[test]
    fn removing_the_only_visible_block_preserves_slide_local_style_source() {
        let source = concat!(
            "<style data-slidx>\n",
            ":root { --slidx-layout: split; }\n",
            "</style>\n\n",
            "# Remove\n",
        );

        assert_eq!(
            removed(source, 0, 0),
            "<style data-slidx>\n:root { --slidx-layout: split; }\n</style>\n\n\n"
        );
    }

    #[test]
    fn line_endings_outside_the_removed_block_stay_crlf() {
        assert_eq!(removed("# Keep\r\n\r\nRemove.\r\n", 0, 1), "# Keep\r\n");
    }

    #[test]
    fn a_missing_block_is_refused_rather_than_guessed() {
        let op = EditOp::RemoveBlock { slide: 0.into(), block: 9.into() };

        assert!(matches!(
            plan("# One\n", &DeckParseOptions::default(), &op),
            Err(EditError::NoSuchBlock { .. })
        ));
    }
}
