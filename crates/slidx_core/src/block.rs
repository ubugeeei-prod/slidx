//! Blocks, and the attribute line that configures one.
//!
//! # Why this exists
//!
//! A [`Mark`](crate::Mark) names a range *inside* a block, which is what lets an
//! editor colour three words. Nothing named a *whole* block, so nothing an
//! author or an editor could say about one had anywhere to go in the file —
//! including the first thing a slide editor is asked for, which is "put this
//! over there".
//!
//! A block attribute is that name. An attribute group on a line of its own
//! attaches to the block below it:
//!
//! ```text
//! {.side}
//! ![The pipeline](./pipeline.svg)
//! ```
//!
//! # What is deliberately not decided here
//!
//! Which class names a *region* is not resolved here, and cannot be: a region
//! belongs to a layout, a layout belongs to the theme, and this crate has never
//! heard of a theme. `.side` and `.accent` are the same shape of thing until
//! something that knows the layout reads them.
//!
//! # What is not an attribute line
//!
//! A line that starts with `{` and does not parse as a group is ordinary
//! content, and nothing is said about it. An author writing a paragraph that
//! begins with a brace is not making a mistake, and a diagnostic there would be
//! a diagnostic on prose.
//!
//! # Why a step anchor never stands alone
//!
//! A marker on its own line stages the block above it — the runtime resolves it
//! to the anchor's previous element sibling. So a chunk that holds nothing but
//! anchors is folded into the block before it rather than counted as a block of
//! its own. That is what keeps the anchor in the same region when the block it
//! stages is moved to another one, and it is why the block list is the same
//! length whether it was taken from the source an author saved or from the
//! content the pipeline compiled.

use serde::{Deserialize, Serialize};

use crate::attributes::{self, Attributes};
use crate::markers::ANCHOR_ATTRIBUTE;
use crate::scanner::FenceTracker;
use crate::span::ByteSpan;

/// One top-level block, and whatever an attribute line said about it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    /// The block's own bytes, step anchors included and the attribute line not.
    pub span: ByteSpan,
    pub attributes: Attributes,
}

/// A block as it was found in a source string, with the line that attributed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundBlock {
    pub block: Block,
    /// The `{…}` line, trailing newline included, or nothing.
    ///
    /// This is the range an operation splices to rewrite a block's attributes,
    /// which is why it is kept separately from the block's own bytes.
    pub attribute_line: Option<ByteSpan>,
}

/// A body with its attribute lines lifted out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtractedBlocks {
    /// The content without the attribute lines, which is what gets rendered.
    pub content: String,
    /// Blocks in source order, spanning `content`.
    pub blocks: Vec<Block>,
}

/// Finds every top-level block, in source order.
///
/// Blank lines and fences decide where a block ends, so a blank line inside a
/// code block does not split one. An attribute line also ends the block above
/// it: an author writing
///
/// ```text
/// # Heading
/// {.side}
/// ![A diagram](./a.svg)
/// ```
///
/// gets a heading and a placed image, because a heading needs no blank line
/// after it and asking for one would be asking them to remember which
/// constructs do.
pub fn find_blocks(source: &str) -> Vec<FoundBlock> {
    let mut blocks: Vec<FoundBlock> = Vec::new();
    let mut carried: Option<ByteSpan> = None;

    for chunk in chunks(source) {
        // A chunk that is only anchors belongs to the block it stages, so the
        // two cannot be separated by a move.
        if is_anchor_only(chunk.slice(source)) {
            if let Some(last) = blocks.last_mut() {
                last.block.span.end = chunk.end;
                continue;
            }
        }

        let mut pending = carried.take();
        let mut start = chunk.start;

        for line in attribute_lines_of(source, chunk) {
            // Content above the line closes as its own block; the line then
            // attributes whatever comes after it.
            let content = ByteSpan::new(start, trimmed_end(source, start, line.start));
            if !content.is_empty() {
                blocks.push(found(source, content, pending));
                pending = None;
            }

            // Consecutive lines are one group, so the second never survives into
            // the content as text.
            pending = Some(match pending {
                Some(span) => ByteSpan::new(span.start, line.end),
                None => line,
            });
            start = line.end;
        }

        if start < chunk.end {
            blocks.push(found(source, ByteSpan::new(start, chunk.end), pending));
        } else {
            carried = pending;
        }
    }

    // Nothing followed it, so it attributes an empty block rather than being
    // written onto the slide as text — which is the state every deck passes
    // through while somebody types the block.
    if let Some(line) = carried {
        blocks.push(found(source, ByteSpan::empty(line.end), Some(line)));
    }

    blocks
}

fn found(source: &str, span: ByteSpan, attribute_line: Option<ByteSpan>) -> FoundBlock {
    let attributes = attribute_line.map(|line| merged_attributes(source, line)).unwrap_or_default();

    FoundBlock { block: Block { span, attributes }, attribute_line }
}

/// Every attribute group in a run of attribute lines, merged.
fn merged_attributes(source: &str, line: ByteSpan) -> Attributes {
    let mut merged = Attributes::default();

    for written in line.slice(source).lines().filter_map(attributes_of) {
        if written.key.is_some() {
            merged.key = written.key;
        }
        merged.classes.extend(written.classes);
        merged.properties.extend(written.properties);
    }

    merged
}

/// Removes every attribute line, and reports the blocks they belonged to.
///
/// Run once, at the end of the parse: the spans are into the returned content,
/// which is the string a renderer slices.
pub fn extract_blocks(source: &str) -> ExtractedBlocks {
    let found = find_blocks(source);
    let mut content = String::with_capacity(source.len());
    let mut blocks = Vec::with_capacity(found.len());
    let mut copied = 0usize;

    for entry in &found {
        if let Some(line) = entry.attribute_line {
            content.push_str(&source[copied..line.start]);
            copied = line.end;
        }

        content.push_str(&source[copied..entry.block.span.start]);

        let start = content.len();
        content.push_str(entry.block.span.slice(source));
        copied = entry.block.span.end;

        blocks.push(Block {
            span: ByteSpan::new(start, content.len()),
            attributes: entry.block.attributes.clone(),
        });
    }

    content.push_str(&source[copied..]);

    ExtractedBlocks { content, blocks }
}

/// The blank-line separated chunks of a source, fences respected.
///
/// A chunk excludes the blank lines around it, so its span is the bytes a
/// renderer would call the block.
fn chunks(source: &str) -> Vec<ByteSpan> {
    let mut spans = Vec::new();
    let mut fences = FenceTracker::new();
    let mut open: Option<(usize, usize)> = None;
    let mut at = 0usize;

    for line in source.split_inclusive('\n') {
        let without_newline = line.trim_end_matches('\n');
        let prose = fences.feed(without_newline);
        let blank = prose && without_newline.trim().is_empty();
        let end = at + without_newline.trim_end().len();

        if blank {
            if let Some((start, last)) = open.take() {
                spans.push(ByteSpan::new(start, last));
            }
        } else {
            let start = open.map_or(at, |(start, _)| start);
            open = Some((start, end));
        }

        at += line.len();
    }

    if let Some((start, last)) = open {
        spans.push(ByteSpan::new(start, last));
    }

    spans
}

/// The attribute lines inside one chunk, each span including its newline.
///
/// Fence-aware, because `{.side}` inside a code block is code. A chunk always
/// begins outside a fence, so tracking from its start is enough.
fn attribute_lines_of(source: &str, chunk: ByteSpan) -> Vec<ByteSpan> {
    let mut spans = Vec::new();
    let mut fences = FenceTracker::new();
    let mut at = chunk.start;

    for line in chunk.slice(source).split_inclusive('\n') {
        let prose = fences.feed(line.trim_end_matches('\n'));
        let mut end = at + line.len();

        // The chunk stops at its last character, but the newline after an
        // attribute line belongs to the line: removing one removes the break.
        if end == chunk.end && source.as_bytes().get(end) == Some(&b'\n') {
            end += 1;
        }

        if prose && attributes_of(line).is_some() {
            spans.push(ByteSpan::new(at, end));
        }

        at += line.len();
    }

    spans
}

/// Where content ends, ignoring the whitespace before the next line.
fn trimmed_end(source: &str, start: usize, end: usize) -> usize {
    start + source[start..end].trim_end().len()
}

/// The group on a line that is nothing but one, or nothing.
fn attributes_of(line: &str) -> Option<Attributes> {
    let trimmed = line.trim();
    let inside = trimmed.strip_prefix('{')?.strip_suffix('}')?;

    attributes::parse(inside)
}

/// True when a chunk holds step anchors and nothing an audience sees.
fn is_anchor_only(text: &str) -> bool {
    let mut rest = text.trim();
    if rest.is_empty() {
        return false;
    }

    while let Some(open) = rest.find("<span ") {
        if !rest[..open].trim().is_empty() {
            return false;
        }
        let Some(close) = rest[open..].find("</span>") else { return false };
        if !rest[open..open + close].contains(ANCHOR_ATTRIBUTE) {
            return false;
        }
        rest = rest[open + close + "</span>".len()..].trim();
    }

    rest.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(source: &str) -> Vec<&str> {
        find_blocks(source).iter().map(|found| found.block.span.slice(source)).collect()
    }

    fn classes(source: &str) -> Vec<Vec<String>> {
        find_blocks(source).iter().map(|found| found.block.attributes.classes.clone()).collect()
    }

    #[test]
    fn a_blank_line_separates_two_blocks() {
        assert_eq!(blocks("# One\n\nSome prose.\n"), ["# One", "Some prose."]);
    }

    #[test]
    fn a_blank_line_inside_a_fence_does_not_split_a_block() {
        // The failure this prevents is a code block torn in half and rendered as
        // two, which loses the fence and colours nothing.
        let source = "```rust\nlet a = 1;\n\nlet b = 2;\n```\n";
        assert_eq!(blocks(source).len(), 1);
    }

    #[test]
    fn a_list_is_one_block() {
        assert_eq!(blocks("- one\n- two\n- three\n").len(), 1);
    }

    #[test]
    fn an_attribute_line_attaches_to_the_block_under_it() {
        let source = "{.side}\n![Diagram](./a.svg)\n";

        assert_eq!(blocks(source), ["![Diagram](./a.svg)"]);
        assert_eq!(classes(source), [vec!["side".to_string()]]);
    }

    #[test]
    fn an_attribute_line_separated_by_a_blank_line_still_attaches_below() {
        // Authors leave a blank line after anything that looks like a heading,
        // and the attribute would otherwise silently attach to nothing.
        let source = "{.side}\n\n![Diagram](./a.svg)\n";

        assert_eq!(blocks(source), ["![Diagram](./a.svg)"]);
        assert_eq!(classes(source), [vec!["side".to_string()]]);
    }

    #[test]
    fn an_attribute_line_ends_the_block_above_it_without_needing_a_blank_line() {
        // A heading needs no blank line after it, and an author should not have
        // to remember which constructs do. Without this the line would sit
        // inside the heading's paragraph and render as text.
        let source = "# One\n{.side}\n![D](./a.svg)\n";

        assert_eq!(blocks(source), ["# One", "![D](./a.svg)"]);
        assert_eq!(classes(source), [Vec::<String>::new(), vec!["side".to_string()]]);
    }

    #[test]
    fn two_attribute_lines_in_a_row_are_one_group() {
        // Whichever the author left behind, neither may survive into the content
        // as text.
        let extracted = extract_blocks("{.side}\n{#hero}\n# One\n");

        assert_eq!(extracted.content, "# One\n");
        assert_eq!(extracted.blocks[0].attributes.classes, vec!["side".to_string()]);
        assert_eq!(extracted.blocks[0].attributes.key.as_deref(), Some("hero"));
    }

    #[test]
    fn a_group_inside_a_fence_is_code_rather_than_an_attribute() {
        // A deck about slidx shows one on a slide, and deleting the line would
        // corrupt the code block it was printed in.
        let extracted = extract_blocks("```md\n{.side}\n# One\n```\n");

        assert_eq!(extracted.content, "```md\n{.side}\n# One\n```\n");
        assert_eq!(extracted.blocks.len(), 1);
        assert!(extracted.blocks[0].attributes.is_empty());
    }

    #[test]
    fn an_attribute_line_carries_a_key_and_properties_too() {
        let found = find_blocks("{#hero .side title=\"A diagram\"}\n![D](./a.svg)\n");
        let attributes = &found[0].block.attributes;

        assert_eq!(attributes.key.as_deref(), Some("hero"));
        assert_eq!(attributes.properties.get("title").map(String::as_str), Some("A diagram"));
    }

    #[test]
    fn a_paragraph_that_merely_begins_with_a_brace_is_ordinary_content() {
        // Nothing is said about it. An author writing prose that opens with a
        // brace is not making a mistake, and a diagnostic here would be one on
        // their prose.
        let source = "{not an attribute group\n";

        assert_eq!(blocks(source), ["{not an attribute group"]);
        assert!(classes(source)[0].is_empty());
    }

    #[test]
    fn an_empty_group_is_not_an_attribute_line() {
        assert_eq!(blocks("{}\nSome prose.\n"), ["{}\nSome prose."]);
    }

    #[test]
    fn a_block_with_no_attribute_line_carries_no_attributes() {
        let found = find_blocks("# One\n");

        assert!(found[0].block.attributes.is_empty());
        assert!(found[0].attribute_line.is_none());
    }

    #[test]
    fn the_attribute_line_span_is_the_bytes_an_operation_would_splice() {
        let source = "{.side}\n# One\n";
        let line = find_blocks(source)[0].attribute_line.unwrap();

        assert_eq!(line.slice(source), "{.side}\n");
    }

    #[test]
    fn a_group_with_nothing_under_it_yet_attributes_an_empty_block() {
        // Every deck passes through this state while somebody types the block.
        // Rendering the line as prose would flash `{.side}` onto the canvas.
        let found = find_blocks("# One\n\n{.side}\n");

        assert_eq!(found.len(), 2);
        assert!(found[1].block.span.is_empty());
        assert_eq!(found[1].block.attributes.classes, vec!["side".to_string()]);
    }

    #[test]
    fn a_chunk_of_nothing_but_anchors_belongs_to_the_block_it_stages() {
        // The runtime stages the anchor's previous element sibling, so the two
        // have to stay in the same region when one of them is moved.
        let source = "# One\n\n<span data-slidx-step=\"1\" hidden></span>\n\n# Two\n";
        let found = find_blocks(source);

        assert_eq!(found.len(), 2);
        assert!(found[0].block.span.slice(source).contains("data-slidx-step"));
    }

    #[test]
    fn an_anchor_with_no_block_above_it_is_a_block_of_its_own() {
        let source = "<span data-slidx-step=\"1\" hidden></span>\n\n# One\n";
        assert_eq!(find_blocks(source).len(), 2);
    }

    #[test]
    fn a_span_that_is_not_an_anchor_is_content() {
        let source = "<span class=\"x\">text</span>\n";
        assert_eq!(blocks(source), ["<span class=\"x\">text</span>"]);
    }

    #[test]
    fn extraction_removes_the_attribute_line_and_leaves_the_block() {
        let extracted = extract_blocks("# One\n\n{.side}\n![D](./a.svg)\n");

        assert_eq!(extracted.content, "# One\n\n![D](./a.svg)\n");
        assert_eq!(extracted.blocks.len(), 2);
        assert_eq!(extracted.blocks[1].attributes.classes, vec!["side".to_string()]);
    }

    #[test]
    fn extracted_spans_name_the_content_they_came_back_with() {
        let extracted = extract_blocks("{.a}\n# One\n\n{.b}\nProse.\n");
        let sliced: Vec<&str> =
            extracted.blocks.iter().map(|block| block.span.slice(&extracted.content)).collect();

        assert_eq!(sliced, ["# One", "Prose."]);
    }

    #[test]
    fn a_body_with_no_attribute_lines_comes_back_byte_identical() {
        // Most slides. Extraction that rewrote them would rewrite every slide in
        // the deck the first time anything was edited.
        for source in
            ["# One\n\n- a\n- b\n", "```rust\nfn main() {}\n```\n", "Prose.\n\n\n\nMore.\n", ""]
        {
            assert_eq!(extract_blocks(source).content, source, "rewrote {source:?}");
        }
    }

    #[test]
    fn finding_blocks_in_half_typed_input_never_panics() {
        for source in
            ["{", "{}", "{.a}", "{.a}\n", "```\n{.a}\n", "{#}\n# One\n", "\n\n\n", "{.a}\n\n\n"]
        {
            let _ = extract_blocks(source);
            let _ = find_blocks(source);
        }
    }
}
