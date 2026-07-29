//! Markdown to HTML, on the Ox Content engine.
//!
//! Ox Content does the parsing and rendering: an arena-allocated, zero-copy
//! CommonMark and GFM engine. slidx adds only what a slide needs on top —
//! nothing here re-implements Markdown.
//!
//! # The one thing that must survive
//!
//! Step anchors are raw `<span data-slidx-step="N" hidden>` elements embedded
//! in the Markdown by [`slidx_core::markers`]. They have to reach the output
//! intact and in the right place, because the whole step pipeline hangs off
//! them. That is asserted here rather than assumed, in both positions the
//! anchor contract distinguishes: inside a list item, and alone in a block.

use ox_content_allocator::Allocator;
use ox_content_parser::{Parser, ParserOptions};
use ox_content_renderer::{HtmlRenderer, HtmlRendererOptions};
use serde::{Deserialize, Serialize};

/// How to read a slide's Markdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownOptions {
    /// Tables, task lists, strikethrough, autolinks, and footnotes.
    ///
    /// On by default: every one of those appears in technical talks, and a
    /// table that renders as raw pipes is a slide nobody can read.
    pub gfm: bool,
    /// Colour fenced code blocks in the languages slidx scans.
    ///
    /// On by default, because the alternative is a wall of one colour that
    /// nobody in row fifteen can find the shape of. Off is for the deck whose
    /// code is not code — a grammar, a diff, a log — where the scanner's
    /// opinion about what a word means is noise.
    pub highlight: bool,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        Self { gfm: true, highlight: true }
    }
}

/// Renders one slide body to HTML.
pub fn render(source: &str, options: &MarkdownOptions) -> String {
    // Sizing the arena to the source means the whole slide is parsed without a
    // single reallocation, which is where the throughput comes from.
    let allocator = Allocator::for_source_len(source.len());
    let parser_options = if options.gfm { ParserOptions::gfm() } else { ParserOptions::default() };

    let parser = Parser::with_options(&allocator, source, parser_options);
    let Ok(document) = parser.parse() else {
        // Ox Content's parser is total for valid UTF-8; this arm exists so a
        // future fallible path cannot take a deck down mid-talk.
        return String::new();
    };

    let html = HtmlRenderer::with_options(HtmlRendererOptions::new()).render(&document);

    if options.highlight {
        crate::highlight::highlight_code_blocks(&html)
    } else {
        html
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{markers, DeckParseOptions};

    fn html(source: &str) -> String {
        render(source, &MarkdownOptions::default())
    }

    #[test]
    fn common_slide_content_renders() {
        let output = html("# Title\n\nA paragraph with **strong** text.\n\n- one\n- two\n");

        assert!(output.contains("<h1"));
        assert!(output.contains("<strong>strong</strong>"));
        assert!(output.contains("<li>one</li>"));
    }

    #[test]
    fn code_fences_keep_their_language() {
        let output = html("```rust\nfn main() {}\n```\n");
        assert!(output.contains("language-rust"), "syntax highlighting needs the class: {output}");
    }

    #[test]
    fn gfm_tables_render() {
        let output = html("| a | b |\n| - | - |\n| 1 | 2 |\n");
        assert!(output.contains("<table>"));
        assert!(output.contains("<th>a</th>"));
    }

    #[test]
    fn gfm_can_be_switched_off() {
        let options = MarkdownOptions { gfm: false, ..MarkdownOptions::default() };
        let output = render("| a | b |\n| - | - |\n| 1 | 2 |\n", &options);
        assert!(!output.contains("<table>"));
    }

    #[test]
    fn headings_carry_ids_for_deep_linking() {
        assert!(html("# Getting Started\n").contains("id=\"getting-started\""));
    }

    #[test]
    fn an_inline_step_anchor_lands_inside_its_list_item() {
        // Contract case 2: the anchor has an `<li>` ancestor, so the runtime
        // stages that item rather than the whole list.
        let output = html("- one<span data-slidx-step=\"1\" hidden></span>\n- two\n");

        assert!(
            output.contains("<li>one<span data-slidx-step=\"1\" hidden></span></li>"),
            "anchor left its list item: {output}"
        );
    }

    #[test]
    fn a_block_step_anchor_lands_alone_in_its_own_paragraph() {
        // Contract case 1: the anchor's parent has no text of its own, so the
        // runtime stages the previous element sibling and drops the wrapper.
        let output = html("Some prose.\n\n<span data-slidx-step=\"2\" hidden></span>\n");

        assert!(
            output.contains("<p><span data-slidx-step=\"2\" hidden></span></p>"),
            "block anchor was not isolated: {output}"
        );
    }

    #[test]
    fn anchors_produced_by_the_core_compiler_survive_the_round_trip() {
        // The two halves are developed separately, so the seam between them is
        // worth asserting end to end rather than per side.
        let deck = slidx_core::parse_deck(
            "# Agenda\n\n- one <!-- step -->\n- two <!-- step -->\n",
            &DeckParseOptions::default(),
        );

        let output = html(&deck.slides[0].content);

        for action in &deck.slides[0].steps.actions {
            for target in action.targets() {
                let attribute = target.trim_start_matches('[').trim_end_matches(']');
                assert!(output.contains(attribute), "target {target} is not in the HTML: {output}");
            }
        }
    }

    #[test]
    fn every_anchor_id_appears_exactly_once() {
        let mut id = 1;
        let staged = markers::extract_step_markers(
            "- a <!-- step -->\n- b <!-- step -->\n- c <!-- step -->\n",
            &mut id,
        );
        let output = html(&staged.content);

        for n in 1..=3 {
            let attribute = format!("data-slidx-step=\"{n}\"");
            assert_eq!(output.matches(&attribute).count(), 1, "anchor {n} is not unique");
        }
    }

    #[test]
    fn an_empty_slide_renders_to_nothing_rather_than_failing() {
        assert!(html("").is_empty());
        assert!(html("   \n\n  ").trim().is_empty());
    }

    #[test]
    fn rendering_is_deterministic() {
        let source = "# One\n\n- a\n- b\n\n```sh\nls\n```\n";
        assert_eq!(html(source), html(source));
    }

    #[test]
    fn non_ascii_content_survives() {
        let output = html("# はじめに\n\n- 日本語のテキスト\n");
        assert!(output.contains("はじめに"));
        assert!(output.contains("日本語のテキスト"));
    }
}
