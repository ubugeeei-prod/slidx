//! Fence-aware line scanning.
//!
//! Every structural decision in a deck — where a slide ends, which `---` opens
//! frontmatter, which lines are list items — has to ignore anything inside a
//! fenced code block. Talks about code are full of `---` lines in diffs, shell
//! transcripts, and YAML samples, and a scanner that misses that splits a slide
//! in half. This module is the one place that knows the rule.

/// Tracks whether the scanner is currently inside a fenced code block.
///
/// Implements the CommonMark fence rules that matter in practice: a fence opens
/// with three or more backticks or tildes indented at most three spaces, and
/// closes with the same character at the same length or longer. A backtick
/// fence cannot be closed by tildes, which is what lets a Markdown slide show a
/// Markdown fence.
#[derive(Debug, Clone, Copy, Default)]
pub struct FenceTracker {
    open: Option<Fence>,
}

#[derive(Debug, Clone, Copy)]
struct Fence {
    marker: char,
    length: usize,
}

impl FenceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// True while the scanner sits inside a fenced block.
    pub fn is_inside(&self) -> bool {
        self.open.is_some()
    }

    /// Feeds one line and reports whether that line is *content*.
    ///
    /// Fence delimiters themselves report `false`, so callers that want to skip
    /// code can simply skip every line for which this returns `false` while
    /// [`is_inside`](Self::is_inside) was or became true.
    pub fn feed(&mut self, line: &str) -> bool {
        let Some(fence) = parse_fence(line) else {
            return !self.is_inside();
        };

        match self.open {
            // Inside a block: only a matching, long-enough fence closes it.
            Some(open) if fence.marker == open.marker && fence.length >= open.length => {
                self.open = None;
                false
            }
            Some(_) => false,
            None => {
                self.open = Some(fence);
                false
            }
        }
    }

    /// True when the line is ordinary prose the caller may interpret.
    ///
    /// Convenience for the common `for line in lines` loop.
    pub fn is_prose(&mut self, line: &str) -> bool {
        self.feed(line)
    }
}

fn parse_fence(line: &str) -> Option<Fence> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }

    let rest = &line[indent..];
    let marker = rest.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }

    let length = rest.chars().take_while(|&c| c == marker).count();
    if length < 3 {
        return None;
    }

    // An info string on a backtick fence may not itself contain backticks,
    // otherwise inline code spans such as ```` ``x`` ```` would open a block.
    if marker == '`' && rest[length..].contains('`') {
        return None;
    }

    Some(Fence { marker, length })
}

/// True when the line is a bare `---` thematic break.
///
/// This is the default deck separator and the frontmatter delimiter. Trailing
/// spaces are tolerated because editors add them; anything else on the line is
/// not a separator.
pub fn is_separator(line: &str) -> bool {
    is_separator_of(line, "---")
}

/// True when the line is exactly `separator`, allowing surrounding whitespace.
///
/// Decks that show Markdown source configure a different separator so their
/// examples survive; everything downstream goes through this one predicate.
pub fn is_separator_of(line: &str, separator: &str) -> bool {
    let trimmed = line.trim_end();
    let indent = trimmed.len() - trimmed.trim_start_matches(' ').len();
    indent <= 3 && trimmed.trim_start() == separator
}

/// The nesting depth of a Markdown list item, or `None` for other lines.
///
/// Returns the leading indentation in spaces, so callers can select top-level
/// items without treating nested bullets as separate stops.
pub fn list_item_indent(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    let rest = line[indent..].trim_end();

    let after_marker = if let Some(rest) = rest.strip_prefix(['-', '*', '+']) {
        rest
    } else {
        // Ordered items: one or more digits followed by `.` or `)`.
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let rest = &rest[digits..];
        rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?
    };

    // A marker must be followed by whitespace, otherwise `-x` and `1.5` would
    // read as list items.
    if after_marker.is_empty() || after_marker.starts_with([' ', '\t']) {
        Some(indent)
    } else {
        None
    }
}

/// The text of an ATX heading, or `None` for other lines.
pub fn heading_text(line: &str) -> Option<&str> {
    heading_span(line).map(|span| &line[span])
}

/// Byte range of an ATX heading's text within its line, or `None`.
///
/// The editor retitles a slide by replacing exactly this range. Everything
/// outside it — the heading level, the spacing, a closing run of hashes — is
/// the author's formatting, and rewriting it would turn a one-word change into
/// a diff about style.
pub fn heading_span(line: &str) -> Option<std::ops::Range<usize>> {
    let indent = line.len() - line.trim_start().len();
    let trimmed = &line[indent..];
    let level = trimmed.chars().take_while(|&c| c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }

    let rest = &trimmed[level..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }

    // A closing run of hashes is decoration, and the space before it is part of
    // that decoration rather than part of the title.
    let start = indent + level + (rest.len() - rest.trim_start().len());
    let text = rest.trim().trim_end_matches('#').trim_end();

    Some(start..start + text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prose_lines(source: &str) -> Vec<&str> {
        let mut tracker = FenceTracker::new();
        source.lines().filter(|line| tracker.feed(line)).collect()
    }

    #[test]
    fn plain_lines_are_prose() {
        assert_eq!(prose_lines("a\nb"), vec!["a", "b"]);
    }

    #[test]
    fn fenced_content_and_delimiters_are_not_prose() {
        assert_eq!(prose_lines("a\n```\ncode\n```\nb"), vec!["a", "b"]);
    }

    #[test]
    fn tildes_do_not_close_a_backtick_fence() {
        assert_eq!(prose_lines("```\n~~~\nstill code\n```\nafter"), vec!["after"]);
    }

    #[test]
    fn a_longer_fence_can_contain_a_shorter_one() {
        assert_eq!(prose_lines("````\n```\nnested\n```\n````\nafter"), vec!["after"]);
    }

    #[test]
    fn a_shorter_fence_does_not_close_a_longer_one() {
        let mut tracker = FenceTracker::new();
        tracker.feed("````");
        tracker.feed("```");
        assert!(tracker.is_inside());
    }

    #[test]
    fn deeply_indented_backticks_are_not_a_fence() {
        assert_eq!(prose_lines("    ```\nstill prose"), vec!["    ```", "still prose"]);
    }

    #[test]
    fn an_info_string_is_allowed() {
        assert_eq!(prose_lines("```rust,ignore\ncode\n```\nafter"), vec!["after"]);
    }

    #[test]
    fn an_inline_code_span_does_not_open_a_fence() {
        assert_eq!(prose_lines("``` `x` ```\nafter"), vec!["``` `x` ```", "after"]);
    }

    #[test]
    fn separators_tolerate_trailing_whitespace() {
        assert!(is_separator("---"));
        assert!(is_separator("---   "));
        assert!(is_separator("  ---"));
    }

    #[test]
    fn longer_rules_and_decorated_lines_are_not_separators() {
        assert!(!is_separator("----"));
        assert!(!is_separator("--- title"));
        assert!(!is_separator("***"));
        assert!(!is_separator("      ---"));
    }

    #[test]
    fn list_items_report_their_indentation() {
        assert_eq!(list_item_indent("- one"), Some(0));
        assert_eq!(list_item_indent("  - nested"), Some(2));
        assert_eq!(list_item_indent("1. one"), Some(0));
        assert_eq!(list_item_indent("12) twelve"), Some(0));
    }

    #[test]
    fn non_list_lines_report_none() {
        assert_eq!(list_item_indent("text"), None);
        assert_eq!(list_item_indent("-no-space"), None);
        assert_eq!(list_item_indent("1.5 is a number"), None);
        assert_eq!(list_item_indent(""), None);
    }

    #[test]
    fn headings_report_their_text() {
        assert_eq!(heading_text("# One"), Some("One"));
        assert_eq!(heading_text("### Deep  "), Some("Deep"));
        assert_eq!(heading_text("## Closed ##"), Some("Closed"));
    }

    #[test]
    fn non_headings_report_none() {
        assert_eq!(heading_text("#no-space"), None);
        assert_eq!(heading_text("####### too deep"), None);
        assert_eq!(heading_text("text"), None);
    }

    #[test]
    fn a_heading_span_names_only_the_words() {
        // Retitling a slide must not restyle it: the level, the padding, and a
        // closing run of hashes all belong to the author, not to the edit.
        for line in ["# One", "###   Deep  ", "## Closed ##", "  # Indented"] {
            let span = heading_span(line).unwrap();
            assert_eq!(&line[span], heading_text(line).unwrap());
        }
    }

    #[test]
    fn an_empty_heading_spans_the_insertion_point_after_the_hashes() {
        let span = heading_span("## ").unwrap();
        assert_eq!(span, 3..3);
    }
}
