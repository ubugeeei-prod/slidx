//! Cutting a slide body into the blocks a translator works in.
//!
//! The unit is a block and not a line, because a hand-wrapped paragraph is one
//! sentence and handing a translator half of one is handing them nothing. It is
//! also not the whole slide, because a bullet list translated as a single string
//! comes back with its bullets rewritten.
//!
//! What never becomes a block:
//!
//! - **A fenced code block**, delimiters and info line included. The info line
//!   carries a `.share` key, which is a file name and a URL in the deck's own
//!   output; the body carries comments that are quoted in a recording.
//! - **A speaker-notes comment.** Notes are translated, but as their own
//!   segments — folding one into the body block around it would splice two
//!   overlapping ranges into the same bytes.
//!
//! Whether a block has any *words* in it is not decided here. A row of table
//! rules and a bare image are both blocks with nothing to translate, and both
//! only look that way once the markup has been masked out — which is
//! [`crate::protect`]'s job, not this module's.
//!
//! Markers stay out of the block: the `- ` of a bullet, the `#` of a heading and
//! the `> ` of a quote are structure, and a translator asked to keep them will
//! eventually not.

use slidx_core::scanner::{heading_span, list_item_indent, FenceTracker};
use slidx_core::ByteSpan;

/// One run of prose in a slide body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Block {
    /// Bytes it occupies in the body it was cut from, markers excluded.
    pub span: ByteSpan,
    /// True for the slide's first heading, which is where its id comes from.
    pub is_heading: bool,
    /// One-based line within the body.
    pub line: u32,
}

/// How a line joins what came before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Join {
    /// A paragraph line, which continues an open block.
    Continue,
    /// A bullet, a quote, or a table row: its own block from here.
    Open,
    /// A heading: one block, on its own, closed immediately.
    Alone,
}

/// Every translatable block in a slide body, in source order.
///
/// `notes` are the byte ranges of the slide's notes comments, which are cut out
/// rather than translated in place.
pub(crate) fn blocks(body: &str, notes: &[ByteSpan]) -> Vec<Block> {
    let mut found: Vec<Block> = Vec::new();
    let mut open: Option<Block> = None;
    let mut fences = FenceTracker::new();
    let mut seen_heading = false;

    for (number, (start, line)) in lines(body).enumerate() {
        let line_span = ByteSpan::new(start, start + line.len());

        // A fence delimiter reports `false` too, so the whole block including
        // both delimiters is skipped by this one test.
        if !fences.feed(line) || line.trim().is_empty() || overlaps(line_span, notes) {
            found.extend(open.take());
            continue;
        }

        let (prose, join) = classify(line);
        let span = ByteSpan::new(start + prose.start, start + prose.end);

        match (join, open.as_mut()) {
            (Join::Continue, Some(block)) => block.span = ByteSpan::new(block.span.start, span.end),
            (Join::Alone, _) => {
                found.extend(open.take());
                found.push(Block { span, is_heading: !seen_heading, line: number as u32 + 1 });
                seen_heading = true;
            }
            _ => {
                found.extend(open.take());
                open = Some(Block { span, is_heading: false, line: number as u32 + 1 });
            }
        }
    }

    found.extend(open);
    found
}

/// Where the prose on this line starts and ends, and how it joins.
fn classify(line: &str) -> (std::ops::Range<usize>, Join) {
    if let Some(span) = heading_span(line) {
        return (span, Join::Alone);
    }

    let end = line.trim_end().len();
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    // A table row is one block, pipes included. Per cell would give a translator
    // one word with no sentence around it, and a missing pipe is visible in the
    // rendered table — unlike a missing mark key.
    if trimmed.starts_with('|') {
        return (indent..end, Join::Open);
    }

    if let Some(marker) = trimmed.strip_prefix('>') {
        let start = indent + 1 + (marker.len() - marker.trim_start().len());
        return (start..end, Join::Open);
    }

    if list_item_indent(line).is_some() {
        let after = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
        let rest = &trimmed[after..];
        let start = indent + after + (rest.len() - rest.trim_start().len());

        return (start..end, Join::Open);
    }

    (indent..end, Join::Continue)
}

fn overlaps(span: ByteSpan, spans: &[ByteSpan]) -> bool {
    spans.iter().any(|other| span.start < other.end && other.start < span.end)
}

/// Body lines, with the byte offset each one starts at.
///
/// `text` excludes the terminator and a carriage return before it, the same as
/// [`str::lines`], while the offset stays in the coordinates of the body.
fn lines(body: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut cursor = 0usize;

    std::iter::from_fn(move || {
        if cursor > body.len() {
            return None;
        }

        let start = cursor;
        let end = body[start..].find('\n').map_or(body.len(), |at| start + at);
        cursor = end + 1;

        Some((start, body[start..end].trim_end_matches('\r')))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(body: &str) -> Vec<&str> {
        blocks(body, &[]).iter().map(|block| block.span.slice(body)).collect()
    }

    #[test]
    fn a_heading_is_one_block_without_its_hashes() {
        assert_eq!(texts("## What actually goes wrong"), ["What actually goes wrong"]);
    }

    #[test]
    fn a_hand_wrapped_paragraph_is_one_block_rather_than_two() {
        // Half a sentence is not translatable. This is the reason the unit is a
        // block and not a line.
        assert_eq!(
            texts("Advancing, going back, and printing all\nindex into the same vector."),
            ["Advancing, going back, and printing all\nindex into the same vector."]
        );
    }

    #[test]
    fn every_bullet_is_its_own_block() {
        assert_eq!(texts("- one\n- two\n- three"), ["one", "two", "three"]);
    }

    #[test]
    fn a_bullet_continued_on_the_next_line_stays_one_block() {
        assert_eq!(texts("- the venue Wi-Fi is down\n  and the fonts were remote"), [
            "the venue Wi-Fi is down\n  and the fonts were remote"
        ]);
    }

    #[test]
    fn a_nested_bullet_is_its_own_block_without_its_marker() {
        assert_eq!(texts("- one\n  - nested"), ["one", "nested"]);
    }

    #[test]
    fn an_ordered_item_loses_its_number_because_the_number_is_structure() {
        assert_eq!(texts("1. first\n2. second"), ["first", "second"]);
    }

    #[test]
    fn a_blank_line_ends_a_block() {
        assert_eq!(texts("first paragraph\n\nsecond paragraph"), [
            "first paragraph",
            "second paragraph"
        ]);
    }

    #[test]
    fn a_fenced_code_block_is_not_translatable_at_all() {
        // Including the info line: it carries the `.share` key, which is a file
        // name and a URL in the deck's own output.
        let body = "Before.\n\n```rust {#retry .share title=\"How we back off\"}\nlet x = 1;\n```\n\nAfter.";

        assert_eq!(texts(body), ["Before.", "After."]);
    }

    #[test]
    fn a_fence_showing_markdown_does_not_leak_its_headings() {
        let body = "# Real\n\n````md\n# Not a heading\n- not a bullet\n````";

        assert_eq!(texts(body), ["Real"]);
    }

    #[test]
    fn a_table_row_is_one_block_including_its_pipes() {
        // Per cell would hand a translator one word with no sentence around it.
        // The row of rules is a block too, and is dropped later for having no
        // words in it once its markup is masked.
        let body = "| Rule | Catches |\n| ---- | ------- |\n| `a`  | b       |";

        assert_eq!(texts(body), ["| Rule | Catches |", "| ---- | ------- |", "| `a`  | b       |"]);
    }

    #[test]
    fn a_quote_loses_its_marker() {
        assert_eq!(texts("> the projector washed it out"), ["the projector washed it out"]);
    }

    #[test]
    fn a_notes_comment_is_not_part_of_the_body_block_around_it() {
        // Two splices into overlapping bytes otherwise, since notes are their
        // own segments.
        let body = "# One\n\n<!-- notes:\nremember the demo\n-->\n\nAfter.";
        let note = ByteSpan::new(body.find("<!--").unwrap(), body.find("-->").unwrap() + 3);

        let texts: Vec<&str> =
            blocks(body, &[note]).iter().map(|block| block.span.slice(body)).collect();

        assert_eq!(texts, ["One", "After."]);
    }

    #[test]
    fn the_first_heading_is_the_one_the_slide_id_comes_from() {
        let found = blocks("# Title\n\nBody.\n\n## Later heading", &[]);

        assert!(found[0].is_heading);
        assert!(!found[1].is_heading);
        assert!(!found[2].is_heading, "a second heading does not name the slide");
    }

    #[test]
    fn an_empty_body_has_nothing_to_translate() {
        assert!(blocks("", &[]).is_empty());
        assert!(blocks("\n\n", &[]).is_empty());
    }

    #[test]
    fn a_block_reports_the_line_it_starts_on() {
        let found = blocks("# One\n\nBody.", &[]);

        assert_eq!(found[0].line, 1);
        assert_eq!(found[1].line, 3);
    }
}
