//! Splitting a source file into per-slide segments.
//!
//! `---` is overloaded in Markdown: it opens frontmatter, closes frontmatter,
//! separates slides, and draws a horizontal rule. Getting the disambiguation
//! right is the whole job of this module, and it is deliberately the only
//! place that reasons about it.
//!
//! The rules, in the order they are applied:
//!
//! 1. A separator on line one opens **deck frontmatter**. Its position is
//!    unambiguous, so it is accepted even if the YAML inside is broken — the
//!    author gets a diagnostic rather than a slide full of raw YAML.
//! 2. A separator anywhere else **ends the current slide**.
//! 3. Immediately after such a separator, a non-blank line that is followed by
//!    another separator, and whose text parses as a YAML mapping, is
//!    **slide frontmatter**. All three conditions are required: the blank-line
//!    test keeps ordinary rules from being misread, and the mapping test keeps
//!    a slide body that happens to sit between two rules from being eaten.
//! 4. Separators inside fenced code blocks are just text.

use crate::scanner::{is_separator_of, FenceTracker};
use crate::span::ByteSpan;

/// A raw frontmatter block, before it is parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrontmatter {
    pub text: String,
    /// One-based line of the block's first key.
    pub line: u32,
    /// Bytes the YAML text occupies, between the delimiters. Setting a field
    /// splices inside this.
    pub span: ByteSpan,
    /// Bytes the whole block occupies, delimiters included. Deleting a slide
    /// splices this away with the body.
    pub block: ByteSpan,
}

/// One slide's worth of source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub frontmatter: Option<RawFrontmatter>,
    pub body: String,
    /// One-based line where the body starts.
    pub line: u32,
    /// True when the block's position made it unambiguous, so a YAML error
    /// should be reported rather than causing a fallback to body text.
    pub frontmatter_is_certain: bool,
    /// Bytes [`body`](Self::body) was read from.
    ///
    /// The two agree byte for byte on a file with Unix line endings. On a file
    /// with CRLF they differ by the carriage returns, and the span is the one
    /// to trust: an edit splices the file as the author saved it, so it must
    /// not silently convert their line endings.
    pub body_span: ByteSpan,
}

impl Segment {
    fn is_blank(&self) -> bool {
        self.body.trim().is_empty() && self.frontmatter.is_none()
    }
}

/// One source line with the bytes it occupies.
///
/// `text` matches what [`str::lines`] yields — the terminator is excluded and
/// a carriage return before it is stripped — while `start` and `end` stay in
/// the coordinates of the original file.
#[derive(Debug, Clone, Copy)]
struct Line<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

fn scan_lines(source: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0usize;

    for (index, byte) in source.bytes().enumerate() {
        if byte != b'\n' {
            continue;
        }

        let mut end = index;
        if end > start && source.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        lines.push(Line { text: &source[start..end], start, end });
        start = index + 1;
    }

    // A file ending in a newline has no final line, which is what `str::lines`
    // reports and what every offset downstream is counted against.
    if start < source.len() {
        lines.push(Line { text: &source[start..], start, end: source.len() });
    }

    lines
}

/// The span the joined text of `lines[from..to]` was read from.
fn joined_span(lines: &[Line<'_>], from: usize, to: usize, source_len: usize) -> ByteSpan {
    match (lines.get(from), to.checked_sub(1).and_then(|last| lines.get(last))) {
        (Some(first), Some(last)) if to > from => ByteSpan::new(first.start, last.end),
        _ => ByteSpan::empty(line_start(lines, from, source_len)),
    }
}

/// Where line `index` begins, or the end of the file when it does not exist.
fn line_start(lines: &[Line<'_>], index: usize, source_len: usize) -> usize {
    lines.get(index).map_or(source_len, |line| line.start)
}

/// The one-based line a body starting at `index` reports, kept inside the file.
///
/// A body can start past the last line: a deck that is nothing but frontmatter
/// has one slide whose body begins where the file ends, and so does one ending
/// in a separator. The arithmetic then names a line that is not there, and
/// every reader of it is an editor being told where to put a caret — a jump to
/// line 4 of a three-line file lands nowhere, and a diagnostic reported there
/// is a diagnostic pointing at nothing.
///
/// So it is clamped to the last line, which is where an author would type this
/// slide's first word. One-based, so an empty file still answers 1 rather than
/// 0: there is a line 1 to put a caret on, even with nothing on it.
fn line_of_body(lines: &[Line<'_>], index: usize) -> u32 {
    index.min(lines.len().saturating_sub(1)) as u32 + 1
}

/// Splits a deck source into segments.
///
/// Always returns at least one segment, so callers never special-case an empty
/// file.
pub fn split(source: &str, separator: &str) -> Vec<Segment> {
    let lines = scan_lines(source);
    let end = source.len();
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    let mut pending = None;
    let mut certain = false;

    // Rule 1: deck frontmatter.
    if lines.first().is_some_and(|line| is_separator_of(line.text, separator)) {
        if let Some(close) = find_separator(&lines, 1, separator) {
            pending = Some(raw_frontmatter(&lines, 0, 1, close, end));
            certain = true;
            cursor = close + 1;
        }
    }

    let mut body_from = cursor;
    let mut body: Vec<&str> = Vec::new();
    let mut body_line = line_of_body(&lines, cursor);
    let mut fences = FenceTracker::new();

    while cursor < lines.len() {
        let line = lines[cursor].text;

        if !fences.feed(line) || !is_separator_of(line, separator) {
            body.push(line);
            cursor += 1;
            continue;
        }

        // Rule 2: the separator ends this slide.
        segments.push(Segment {
            frontmatter: pending.take(),
            body: body.join("\n"),
            line: body_line,
            frontmatter_is_certain: certain,
            body_span: joined_span(&lines, body_from, cursor, end),
        });
        body.clear();
        certain = false;

        // Rule 3: an immediately following YAML mapping is slide frontmatter.
        // The separator that ended the slide is also this block's opening
        // delimiter, which is why the block starts one line back.
        let opened_at = cursor;
        cursor += 1;
        if let Some(close) = detect_frontmatter(&lines, cursor, separator) {
            pending = Some(raw_frontmatter(&lines, opened_at, cursor, close, end));
            cursor = close + 1;
        }

        body_from = cursor;
        body_line = line_of_body(&lines, cursor);
    }

    segments.push(Segment {
        frontmatter: pending,
        body: body.join("\n"),
        line: body_line,
        frontmatter_is_certain: certain,
        body_span: joined_span(&lines, body_from, lines.len(), end),
    });

    // A trailing separator, or a file of nothing but separators, leaves blank
    // segments behind. Keep one so callers always have a slide to render.
    let kept: Vec<Segment> = segments.into_iter().filter(|segment| !segment.is_blank()).collect();
    if kept.is_empty() {
        vec![Segment {
            frontmatter: None,
            body: String::new(),
            line: 1,
            frontmatter_is_certain: false,
            body_span: ByteSpan::empty(0),
        }]
    } else {
        kept
    }
}

/// Builds the block whose opening delimiter is `open`, text is `text..close`,
/// and closing delimiter is `close`.
fn raw_frontmatter(
    lines: &[Line<'_>],
    open: usize,
    text: usize,
    close: usize,
    source_len: usize,
) -> RawFrontmatter {
    RawFrontmatter {
        text: lines[text..close].iter().map(|line| line.text).collect::<Vec<_>>().join("\n"),
        line: text as u32 + 1,
        span: joined_span(lines, text, close, source_len),
        block: ByteSpan::new(
            line_start(lines, open, source_len),
            lines.get(close).map_or(source_len, |line| line.end),
        ),
    }
}

fn find_separator(lines: &[Line<'_>], from: usize, separator: &str) -> Option<usize> {
    lines[from..]
        .iter()
        .position(|line| is_separator_of(line.text, separator))
        .map(|offset| from + offset)
}

/// Returns the index of the closing separator when `start` opens frontmatter.
fn detect_frontmatter(lines: &[Line<'_>], start: usize, separator: &str) -> Option<usize> {
    let first = lines.get(start)?.text;
    if first.trim().is_empty() || is_separator_of(first, separator) {
        return None;
    }

    let close = find_separator(lines, start + 1, separator)?;
    let block = lines[start..close].iter().map(|line| line.text).collect::<Vec<_>>().join("\n");

    // Only a mapping with at least one key counts. A slide body sitting between
    // two rules parses as a string, a comment, or nothing; requiring a real key
    // is what stops `# Two` — a YAML comment — from being read as frontmatter.
    match serde_yaml::from_str::<serde_yaml::Value>(&block) {
        Ok(serde_yaml::Value::Mapping(mapping)) if !mapping.is_empty() => Some(close),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split_default(source: &str) -> Vec<Segment> {
        split(source, "---")
    }

    #[test]
    fn a_plain_document_is_one_segment() {
        let segments = split_default("# One\n\nBody.");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].body, "# One\n\nBody.");
        assert!(segments[0].frontmatter.is_none());
    }

    #[test]
    fn an_empty_document_still_yields_a_segment() {
        assert_eq!(split_default("").len(), 1);
        assert_eq!(split_default("\n\n").len(), 1);
    }

    #[test]
    fn deck_frontmatter_attaches_to_the_first_segment() {
        let segments = split_default("---\ntitle: T\n---\n\n# One");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].frontmatter.as_ref().unwrap().text, "title: T");
        assert_eq!(segments[0].frontmatter.as_ref().unwrap().line, 2);
        assert!(segments[0].frontmatter_is_certain);
        assert_eq!(segments[0].body.trim(), "# One");
    }

    #[test]
    fn broken_deck_frontmatter_is_still_treated_as_frontmatter() {
        let segments = split_default("---\ntitle: [oops\n---\n\n# One");
        assert!(segments[0].frontmatter.is_some(), "position alone settles it");
        assert!(segments[0].frontmatter_is_certain);
    }

    #[test]
    fn separators_split_slides() {
        let segments = split_default("# One\n\n---\n\n# Two\n\n---\n\n# Three");
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[2].body.trim(), "# Three");
    }

    #[test]
    fn slide_frontmatter_must_follow_the_separator_immediately() {
        let segments = split_default("# One\n\n---\nlayout: split\n---\n\n# Two");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[1].frontmatter.as_ref().unwrap().text, "layout: split");
        assert!(!segments[1].frontmatter_is_certain, "mid-document blocks are inferred");
    }

    #[test]
    fn a_blank_line_after_the_separator_means_there_is_no_frontmatter() {
        let segments = split_default("# One\n\n---\n\nlayout: not frontmatter\n\n---\n\n# Two");
        assert_eq!(segments.len(), 3);
        assert!(segments[1].frontmatter.is_none());
    }

    #[test]
    fn a_body_between_two_rules_is_not_mistaken_for_frontmatter() {
        let segments = split_default("# One\n\n---\n# Two\n---\n\n# Three");
        assert_eq!(segments.len(), 3);
        assert!(segments[1].frontmatter.is_none(), "`# Two` is a heading, not a mapping");
        assert_eq!(segments[1].body.trim(), "# Two");
    }

    #[test]
    fn separators_inside_fences_are_content() {
        let segments = split_default("# One\n\n```sh\n---\n```\n\n---\n\n# Two");
        assert_eq!(segments.len(), 2);
        assert!(segments[0].body.contains("```sh\n---\n```"));
    }

    #[test]
    fn a_trailing_separator_does_not_create_an_empty_slide() {
        let segments = split_default("# One\n\n---\n");
        assert_eq!(segments.len(), 1);
    }

    #[test]
    fn a_custom_separator_is_honoured() {
        let segments = split("# One\n\n===\n\n# Two", "===");
        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn a_custom_separator_leaves_markdown_rules_alone() {
        let segments = split("# One\n\n---\n\n# Still One\n\n===\n\n# Two", "===");
        assert_eq!(segments.len(), 2);
        assert!(segments[0].body.contains("---"));
    }

    #[test]
    fn body_line_numbers_point_at_the_source() {
        let segments = split_default("---\ntitle: T\n---\n# One\n\n---\n\n# Two");
        assert_eq!(segments[0].line, 4, "body starts after the closing delimiter");
        assert_eq!(segments[1].line, 7);
    }

    #[test]
    fn a_body_line_never_names_a_line_the_file_does_not_have() {
        // Everything that reads this is an editor being told where to put a
        // caret, and a jump to line 4 of a three-line file lands nowhere.
        for source in [
            // Nothing but deck frontmatter: the body begins where the file ends.
            "---\ntitle: T\n---\n",
            // The same, with no trailing newline.
            "---\ntitle: T\n---",
            // A trailing separator, whose blank segment is dropped — leaving
            // the slide before it, whose own line must still be inside.
            "# One\n\n---\n",
            // Nothing at all.
            "",
        ] {
            let deck = crate::parse_deck(source, &crate::DeckParseOptions::default());
            let last = source.lines().count().max(1) as u32;

            for slide in &deck.slides {
                assert!(
                    slide.source_line >= 1 && slide.source_line <= last,
                    "{source:?}: slide {} says line {}, file has {last}",
                    slide.index,
                    slide.source_line
                );
            }
        }
    }

    #[test]
    fn a_body_span_names_the_bytes_the_body_was_read_from() {
        // The editor changes a deck by splicing the file, so every segment has
        // to be able to say which bytes it came from — not just which line.
        let source = "---\ntitle: T\n---\n\n# One\n\n---\n\n# Two\n";

        for segment in split_default(source) {
            assert_eq!(segment.body_span.slice(source), segment.body);
        }
    }

    #[test]
    fn a_frontmatter_block_names_its_text_and_its_delimiters_separately() {
        // Setting a field splices inside the text; deleting a slide splices
        // the whole block away, delimiters included.
        let source = "---\ntitle: T\n---\n\n# One";
        let segments = split_default(source);
        let matter = segments[0].frontmatter.as_ref().unwrap();

        assert_eq!(matter.span.slice(source), "title: T");
        assert_eq!(matter.block.slice(source), "---\ntitle: T\n---");
    }

    #[test]
    fn a_slide_frontmatter_block_starts_at_the_separator_that_opens_it() {
        let source = "# One\n\n---\nlayout: split\n---\n\n# Two";
        let segments = split_default(source);
        let matter = segments[1].frontmatter.as_ref().unwrap();

        assert_eq!(matter.span.slice(source), "layout: split");
        assert_eq!(matter.block.slice(source), "---\nlayout: split\n---");
        assert_eq!(segments[1].body_span.slice(source), "\n# Two");
    }

    #[test]
    fn an_empty_frontmatter_block_names_an_insertion_point_between_its_delimiters() {
        let source = "---\n---\n\n# One";
        let segments = split_default(source);
        let matter = segments[0].frontmatter.as_ref().unwrap();

        assert_eq!(matter.span.slice(source), "");
        assert_eq!(matter.block.slice(source), "---\n---");
    }

    #[test]
    fn a_body_span_is_empty_where_a_slide_has_no_body() {
        let source = "---\nlayout: cover\n---\n";
        let segments = split_default(source);

        assert!(segments[0].body_span.is_empty());
        assert!(segments[0].body_span.start >= segments[0].frontmatter.as_ref().unwrap().block.end);
    }

    #[test]
    fn an_unterminated_deck_frontmatter_is_treated_as_body() {
        // Better to show the author their YAML than to swallow the whole file.
        let segments = split_default("---\ntitle: T\n\n# One");
        assert_eq!(segments.len(), 1);
        assert!(segments[0].frontmatter.is_none());
    }
}
