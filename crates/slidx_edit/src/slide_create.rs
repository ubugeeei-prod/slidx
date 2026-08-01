//! Useful first drafts for a slide, owned by the source writer.
//!
//! A visual surface should ask for a narrative shape, not send a Markdown
//! template. Keeping the four authored forms here means line endings, layout
//! declarations, region attributes, and placeholder copy cross the same writer
//! and undo boundary as every other edit.

use crate::edit::EditBuilder;
use crate::op::{EditError, SlideKind};
use crate::source::DeckSource;

pub(crate) fn create(
    deck: &DeckSource<'_>,
    at: usize,
    kind: SlideKind,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    super::slide::insert(deck, at, &source(kind, deck.newline()), builder)
}

/** The smallest useful first draft of each narrative shape. */
fn source(kind: SlideKind, newline: &str) -> String {
    let lines: &[&str] = match kind {
        SlideKind::TitleBody => &[
            "<style data-slidx>",
            ":root {",
            "  --slidx-layout: stack;",
            "}",
            "</style>",
            "",
            "{.title}",
            "## New slide",
            "",
            "Make the point in one clear sentence.",
        ],
        SlideKind::Statement => &["# One clear idea", "", "Explain why it matters."],
        SlideKind::Comparison => &[
            "<style data-slidx>",
            ":root {",
            "  --slidx-layout: split;",
            "}",
            "</style>",
            "",
            "## First option",
            "",
            "What makes it strong.",
            "",
            "{.right}",
            "## Second option",
            "",
            "{.right}",
            "What makes it different.",
        ],
        SlideKind::Points => &[
            "<style data-slidx>",
            ":root {",
            "  --slidx-layout: stack;",
            "}",
            "</style>",
            "",
            "{.title}",
            "## Three things to remember",
            "",
            "- First point",
            "- Second point",
            "- Third point",
        ],
    };

    lines.join(newline)
}

#[cfg(test)]
mod tests {
    use slidx_core::{parse_deck, DeckParseOptions};

    use crate::{apply, EditOp, SlideKind};

    fn created(source: &str, at: usize, kind: SlideKind) -> String {
        apply(source, &DeckParseOptions::default(), &EditOp::CreateSlide { at, kind })
            .expect("the fixture has the position it names")
    }

    #[test]
    fn title_and_body_start_in_the_two_regions_they_name() {
        let result = created("# One\n", 1, SlideKind::TitleBody);
        let deck = parse_deck(&result, &DeckParseOptions::default());
        let slide = &deck.slides[1];

        assert_eq!(slide.title.as_deref(), Some("New slide"));
        assert_eq!(slide.layout.as_deref(), Some("stack"));
        assert_eq!(slide.blocks[0].attributes.classes, ["title"]);
        assert!(result.starts_with("# One\n\n---\n\n<style data-slidx>"));
    }

    #[test]
    fn a_statement_is_a_complete_first_thought_without_layout_noise() {
        assert_eq!(
            created("# One\n", 1, SlideKind::Statement),
            "# One\n\n---\n\n# One clear idea\n\nExplain why it matters.\n"
        );
    }

    #[test]
    fn a_comparison_places_its_second_side_in_the_right_region() {
        let result = created("# One\n", 1, SlideKind::Comparison);
        let deck = parse_deck(&result, &DeckParseOptions::default());
        let slide = &deck.slides[1];

        assert_eq!(slide.layout.as_deref(), Some("split"));
        assert_eq!(slide.blocks[2].attributes.classes, ["right"]);
        assert_eq!(slide.blocks[3].attributes.classes, ["right"]);
    }

    #[test]
    fn key_points_begin_as_one_list_and_keep_the_decks_line_endings() {
        let result = created("# One\r\n", 1, SlideKind::Points);

        assert!(!result.replace("\r\n", "").contains('\n'));
        assert!(result.contains("- First point\r\n- Second point\r\n- Third point"));
    }

    #[test]
    fn a_layout_recipe_can_be_inserted_before_the_opening_slide() {
        let result = created("---\ntitle: Deck\n---\n\n# One\n", 0, SlideKind::TitleBody);
        let deck = parse_deck(&result, &DeckParseOptions::default());

        assert_eq!(deck.slides.len(), 2);
        assert_eq!(deck.slides[0].title.as_deref(), Some("New slide"));
        assert_eq!(deck.slides[0].layout.as_deref(), Some("stack"));
        assert_eq!(deck.slides[1].title.as_deref(), Some("One"));
    }
}
