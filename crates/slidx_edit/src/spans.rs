//! Where the things an editor points at are in the file.
//!
//! An operation names bytes, and the editor's other half — a rendered slide in
//! an iframe — knows only what a reader sees. Something has to carry the
//! addresses across, and it has to be this crate: a second answer about where a
//! block starts would let a drag move the block above the one that was picked
//! up, and a second answer about where a mark's words are would let a text edit
//! swallow the `#key` a step targets.
//!
//! So the editor is told, rather than working it out. Every span here comes from
//! the same functions the operations splice with, which is why the editor can
//! map a caret to a byte range without a Markdown parser of its own.
//!
//! # Why block and mark spans are measured in the body
//!
//! The editor holds one slide's body as a string — that is what it puts in front
//! of an author and what a mark's range is already measured in — so a body-local
//! span is one it can slice directly. A file-local one would make every reader
//! subtract the same number.

use serde::{Deserialize, Serialize};

use slidx_core::{find_blocks, find_marks, ByteSpan, DeckParseOptions};

use crate::source::DeckSource;

/// Where one slide's bytes are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideSpans {
    /// Everything the slide is, its own frontmatter block included.
    pub content: ByteSpan,
    /// The Markdown body alone, which is what a mark's range is measured in
    /// and what an editor puts in front of an author.
    pub body: ByteSpan,
    /// The slide's top-level blocks, in source order.
    ///
    /// The same order the renderer writes onto the page as `data-slidx-block`,
    /// so the number a canvas gesture sends back indexes straight into this.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<BlockSpans>,
}

/// Where one block is, and what inside it can be addressed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSpans {
    /// The block's own bytes, its attribute line excluded.
    pub span: ByteSpan,
    /// The marks inside it, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<MarkSpans>,
}

/// Where one mark is, and where the words inside it are.
///
/// The words are called out separately because they are the only part of a mark
/// a text edit may touch. Everything from `]` onwards is an address a `steps:`
/// entry or a theme class points at, and typing in a paragraph must not be able
/// to reach it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkSpans {
    /// The whole mark, from `[` to the closing `}`.
    pub span: ByteSpan,
    /// The words between the brackets, as written — escapes included.
    pub words: ByteSpan,
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
        .map(|slide| SlideSpans {
            content: slide.content,
            body: slide.body,
            blocks: blocks_of(slide.body.slice(source)),
        })
        .collect()
}

/// One body's blocks, with the marks in each, all measured in that body.
fn blocks_of(body: &str) -> Vec<BlockSpans> {
    find_blocks(body)
        .into_iter()
        .map(|found| {
            let span = found.block.span;
            let marks = find_marks(span.slice(body))
                .into_iter()
                .map(|mark| MarkSpans {
                    span: ByteSpan::new(mark.start, mark.end).shifted(span.start),
                    // `[` is one byte and so is the `]` before the group, so the
                    // words are what the two brackets leave.
                    words: ByteSpan::new(mark.start + 1, mark.attributes_start - 1)
                        .shifted(span.start),
                })
                .collect();

            BlockSpans { span, marks }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(source: &str) -> Vec<SlideSpans> {
        slide_spans(source, &DeckParseOptions::default())
    }

    fn content(source: &str) -> Vec<&str> {
        spans(source).iter().map(|slide| slide.content.slice(source)).collect()
    }

    fn bodies(source: &str) -> Vec<&str> {
        spans(source).iter().map(|slide| slide.body.slice(source)).collect()
    }

    /// Every block of the first slide, sliced out of its body.
    fn blocks(source: &str) -> Vec<String> {
        let located = spans(source);
        let body = located[0].body.slice(source);

        located[0].blocks.iter().map(|block| block.span.slice(body).to_string()).collect()
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

    #[test]
    fn a_blocks_span_is_measured_in_the_body_the_editor_holds() {
        // The editor slices the body out of the deck source and then slices a
        // block out of that, so a file-local number would land on the wrong
        // slide entirely.
        let source = "# One\n\n---\n\n# Two\n\nSecond.\n";
        assert_eq!(bodies(source)[1], "# Two\n\nSecond.");

        let second = &spans(source)[1];
        let body = second.body.slice(source);
        let sliced: Vec<&str> = second.blocks.iter().map(|block| block.span.slice(body)).collect();

        assert_eq!(sliced, ["# Two", "Second."]);
    }

    #[test]
    fn a_blocks_attribute_line_is_not_part_of_the_block() {
        // It is not text a reader sees, so a caret can never be in it — and a
        // span that swallowed it would let a text edit delete a placement.
        assert_eq!(blocks("{.side}\n![D](./a.svg)\n"), ["![D](./a.svg)"]);
    }

    #[test]
    fn a_marks_words_are_named_apart_from_the_group_that_addresses_them() {
        let source = "Latency dropped to [120ms]{#latency}.\n";
        let block = &spans(source)[0].blocks[0];
        let mark = block.marks[0];

        assert_eq!(mark.span.slice(source), "[120ms]{#latency}");
        assert_eq!(mark.words.slice(source), "120ms");
    }

    #[test]
    fn a_marks_span_is_measured_in_the_body_rather_than_in_its_block() {
        // The block it is in starts partway down, and a mark span counted from
        // the block would name the wrong bytes of the body.
        let source = "# One\n\nA [b]{.accent} c.\n";
        let mark = spans(source)[0].blocks[1].marks[0];

        assert_eq!(mark.span.slice(source), "[b]{.accent}");
    }

    #[test]
    fn a_block_with_no_marks_carries_none_across_the_boundary() {
        // The editor asks whether a block has marks constantly. An empty list
        // per block would be a field on every block of every slide.
        let block = &spans("# One\n")[0].blocks[0];
        assert!(block.marks.is_empty());

        let json = serde_json::to_value(block).unwrap();
        assert_eq!(json.get("marks"), None);
    }
}
