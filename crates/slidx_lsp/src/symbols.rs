//! The deck outline: slides, and the steps inside them.
//!
//! This is the editor's outline pane, its breadcrumb bar, and the jump-to-slide
//! a sixty-slide deck needs. A deck is the one kind of document where scrolling
//! is never the right way to find something — slides are addressed by name in
//! the talk itself, and by number in every note the author wrote about it.
//!
//! # Where a step points
//!
//! At its slide. A [`slidx_core::StepAction`] has no source span of its own:
//! a marker-derived action came from a comment that has already been rewritten
//! into an anchor, and a declared one came from a YAML list item whose line
//! nobody recorded. Rather than invent a position, a step symbol shares its
//! slide's range, so selecting one goes to the slide it stages. Giving actions
//! real spans upstream is what would improve this, and nothing here would have
//! to change but the two lines that build the range.

use serde::{Deserialize, Serialize};
use slidx_core::{mark, markers, Slide, StepAction};

use crate::analysis::{Analysis, LineSpan};
use crate::position::{LineIndex, PositionEncoding, Range};

/// The `SymbolKind` values this server uses, which are numbers on the wire.
mod kind {
    /// A container in every editor's icon set, which is what a slide is.
    pub const MODULE: u8 = 2;
    /// Something that happens, which is what a step is.
    pub const EVENT: u8 = 24;
}

/// One node of the outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: String,
    pub kind: u8,
    /// Everything the symbol covers, which is what a breadcrumb reads.
    pub range: Range,
    /// What is selected when the symbol is chosen. Must sit inside `range`.
    pub selection_range: Range,
    pub children: Vec<DocumentSymbol>,
}

/// Builds the outline for a document.
///
/// Ranges are measured against the text on screen, so an outline carried over
/// from an earlier analysis — see [`crate::document::TextDocument::outline_analysis`] —
/// still points at positions that exist.
pub fn outline(
    analysis: &Analysis,
    text: &str,
    index: &LineIndex,
    encoding: PositionEncoding,
) -> Vec<DocumentSymbol> {
    analysis
        .deck
        .slides
        .iter()
        .enumerate()
        .map(|(at, slide)| {
            let extent = analysis.slides.get(at).copied().unwrap_or(LineSpan { first: 1, last: 1 });
            let range = index.lines_range(text, extent.first, extent.last, encoding);
            let selection =
                index.line_range(text, analysis.content_line(at, text, index), encoding);

            DocumentSymbol {
                name: slide.display_title(),
                detail: describe(slide),
                kind: kind::MODULE,
                range,
                selection_range: selection,
                children: slide
                    .steps
                    .actions
                    .iter()
                    .map(|action| step_symbol(action, range, selection))
                    .collect(),
            }
        })
        .collect()
}

/// What a slide costs a presenter, in one line.
///
/// Shown in the outline and again on hover, from here both times: two
/// renderings of the same fact that disagreed would be worse than one.
pub fn describe(slide: &Slide) -> String {
    let stops = slide.stop_count();
    let mut detail = match stops {
        1 => "1 stop".to_string(),
        many => format!("{many} stops"),
    };

    if let Some(budget) = slide.budget_seconds {
        detail.push_str(&format!(" · {budget}s"));
    }
    if slide.optional {
        detail.push_str(" · optional");
    }

    detail
}

fn step_symbol(action: &StepAction, range: Range, selection: Range) -> DocumentSymbol {
    DocumentSymbol {
        name: name_of(action),
        detail: action
            .options()
            .preset
            .map(|preset| preset.as_token().to_string())
            .unwrap_or_default(),
        kind: kind::EVENT,
        range,
        selection_range: selection,
        children: match action {
            StepAction::Group { actions, .. } => {
                actions.iter().map(|inner| step_symbol(inner, range, selection)).collect()
            }
            _ => Vec::new(),
        },
    }
}

fn name_of(action: &StepAction) -> String {
    let verb = match action {
        StepAction::Reveal { .. } => "reveal",
        StepAction::Hide { .. } => "hide",
        StepAction::Emphasize { .. } => "emphasize",
        StepAction::Set { .. } => "set",
        StepAction::Group { actions, .. } => {
            return format!("group of {}", actions.len());
        }
    };

    match action.targets().first() {
        Some(target) => format!("{verb} {}", readable(target)),
        None => verb.to_string(),
    }
}

/// Turns a compiled selector back into something an author recognises.
///
/// A marker becomes `[data-slidx-step="3"]` on its way through the pipeline
/// and a mark becomes `[data-slidx-mark="count"]`. Neither is what the author
/// wrote, and an outline full of attribute selectors is an outline nobody
/// reads.
fn readable(target: &str) -> String {
    if let Some(id) = attribute_value(target, markers::ANCHOR_ATTRIBUTE) {
        return format!("step {id}");
    }
    if let Some(key) = attribute_value(target, mark::MARK_ATTRIBUTE) {
        return format!("#{key}");
    }

    target.to_string()
}

fn attribute_value(target: &str, attribute: &str) -> Option<String> {
    let inner = target.strip_prefix('[')?.strip_suffix(']')?;
    let value = inner.strip_prefix(attribute)?.strip_prefix("=\"")?.strip_suffix('"')?;

    Some(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;

    fn build(source: &str) -> Vec<DocumentSymbol> {
        let analysis = analyze(source);
        outline(&analysis, source, &LineIndex::new(source), PositionEncoding::Utf16)
    }

    #[test]
    fn every_slide_is_a_symbol_named_by_its_heading() {
        let symbols = build("# Opening\n\n---\n\n# The Problem\n");

        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "Opening");
        assert_eq!(symbols[1].name, "The Problem");
    }

    #[test]
    fn a_slide_without_a_heading_is_still_findable() {
        let symbols = build("# One\n\n---\n\n```rust\nfn main() {}\n```\n");
        assert_eq!(symbols[1].name, "Slide 2");
    }

    #[test]
    fn a_title_keeps_its_words_and_loses_its_styling() {
        // The same rule the outline, the OG image, and the PDF bookmark use.
        assert_eq!(build("# Making [decks]{.accent} fast\n")[0].name, "Making decks fast");
    }

    #[test]
    fn a_slide_says_how_many_advances_it_costs() {
        let symbols = build("# One\n\n- a <!-- step -->\n- b <!-- step -->\n");
        assert_eq!(symbols[0].detail, "3 stops");

        assert_eq!(build("# One\n")[0].detail, "1 stop");
    }

    #[test]
    fn a_budgeted_or_optional_slide_says_so_in_the_outline() {
        // Both are things a speaker looks for while cutting a deck to length.
        let symbols = build("---\nbudget: 90s\noptional: true\n---\n\n# Deep Dive\n");
        assert_eq!(symbols[0].detail, "1 stop · 90s · optional");
    }

    #[test]
    fn steps_are_children_of_the_slide_they_stage() {
        let symbols = build("# One\n\n- a <!-- step -->\n- b <!-- step: zoom -->\n");

        assert_eq!(symbols[0].children.len(), 2);
        assert_eq!(symbols[0].children[0].name, "reveal step 1");
        assert_eq!(symbols[0].children[1].detail, "zoom");
    }

    #[test]
    fn a_step_naming_a_mark_shows_the_key_the_author_wrote() {
        // `[data-slidx-mark="count"]` is what the compiler made of `#count`,
        // and an outline of attribute selectors is one nobody reads.
        let source = "---\nsteps:\n  - emphasize: \"#count\"\n---\n\n[42]{#count}\n";
        let symbols = build(source);

        assert_eq!(symbols[0].children[0].name, "emphasize #count");
    }

    #[test]
    fn a_step_naming_a_class_is_left_as_written() {
        let symbols = build("---\nsteps:\n  - reveal: \".chart\"\n---\n\n# One\n");
        assert_eq!(symbols[0].children[0].name, "reveal .chart");
    }

    #[test]
    fn actions_that_land_together_nest_under_one_stop() {
        let source =
            "---\nsteps:\n  - group:\n      - reveal: \".a\"\n      - hide: \".b\"\n---\n\n# One\n";
        let symbols = build(source);

        assert_eq!(symbols[0].children[0].name, "group of 2");
        assert_eq!(symbols[0].children[0].children.len(), 2);
        assert_eq!(symbols[0].children[0].children[1].name, "hide .b");
    }

    #[test]
    fn a_slide_spans_its_own_source_and_selects_its_heading() {
        let symbols = build("---\ntitle: T\n---\n\n# One\n\n---\n\n# Two\n");

        assert_eq!(symbols[0].range.start.line, 0, "including the deck frontmatter");
        assert_eq!(symbols[0].selection_range.start.line, 4, "the heading, not the frontmatter");
        assert_eq!(symbols[1].selection_range.start.line, 8, "not the blank line above it");
    }

    #[test]
    fn every_selection_sits_inside_the_range_it_belongs_to() {
        // An editor is entitled to assume this, and rejects the whole response
        // when it does not hold.
        for symbol in build("---\ntitle: T\n---\n\n# One\n\n---\nbudget: 30s\n---\n\n# Two\n") {
            assert!(symbol.selection_range.start.line >= symbol.range.start.line, "{symbol:?}");
            assert!(symbol.selection_range.end.line <= symbol.range.end.line, "{symbol:?}");
        }
    }

    #[test]
    fn a_japanese_outline_measures_its_ranges_in_code_units() {
        let source = "# 高速なデッキ\n\n---\n\n# まとめ\n";
        let symbols = build(source);

        assert_eq!(symbols[0].name, "高速なデッキ");
        assert_eq!(symbols[0].selection_range.end.character, 8, "not the 20 bytes it takes");
    }

    #[test]
    fn an_empty_document_has_one_symbol_rather_than_none() {
        assert_eq!(build("").len(), 1);
    }

    #[test]
    fn symbols_serialise_with_the_field_names_the_protocol_uses() {
        let json = serde_json::to_value(&build("# One\n")[0]).unwrap();
        assert!(json.get("selectionRange").is_some(), "{json}");
        assert_eq!(json["kind"], 2);
    }
}
