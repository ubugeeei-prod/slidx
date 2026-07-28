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

/// A raw frontmatter block, before it is parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrontmatter {
    pub text: String,
    /// One-based line of the block's first key.
    pub line: u32,
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
}

impl Segment {
    fn is_blank(&self) -> bool {
        self.body.trim().is_empty() && self.frontmatter.is_none()
    }
}

/// Splits a deck source into segments.
///
/// Always returns at least one segment, so callers never special-case an empty
/// file.
pub fn split(source: &str, separator: &str) -> Vec<Segment> {
    let lines: Vec<&str> = source.lines().collect();
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    let mut pending = None;
    let mut certain = false;

    // Rule 1: deck frontmatter.
    if lines.first().is_some_and(|line| is_separator_of(line, separator)) {
        if let Some(close) = find_separator(&lines, 1, separator) {
            pending = Some(RawFrontmatter { text: lines[1..close].join("\n"), line: 2 });
            certain = true;
            cursor = close + 1;
        }
    }

    let mut body: Vec<&str> = Vec::new();
    let mut body_line = cursor as u32 + 1;
    let mut fences = FenceTracker::new();

    while cursor < lines.len() {
        let line = lines[cursor];

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
        });
        body.clear();
        certain = false;

        // Rule 3: an immediately following YAML mapping is slide frontmatter.
        cursor += 1;
        if let Some(close) = detect_frontmatter(&lines, cursor, separator) {
            pending = Some(RawFrontmatter {
                text: lines[cursor..close].join("\n"),
                line: cursor as u32 + 1,
            });
            cursor = close + 1;
        }

        body_line = cursor as u32 + 1;
    }

    segments.push(Segment {
        frontmatter: pending,
        body: body.join("\n"),
        line: body_line,
        frontmatter_is_certain: certain,
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
        }]
    } else {
        kept
    }
}

fn find_separator(lines: &[&str], from: usize, separator: &str) -> Option<usize> {
    lines[from..]
        .iter()
        .position(|line| is_separator_of(line, separator))
        .map(|offset| from + offset)
}

/// Returns the index of the closing separator when `start` opens frontmatter.
fn detect_frontmatter(lines: &[&str], start: usize, separator: &str) -> Option<usize> {
    let first = lines.get(start)?;
    if first.trim().is_empty() || is_separator_of(first, separator) {
        return None;
    }

    let close = find_separator(lines, start + 1, separator)?;
    let block = lines[start..close].join("\n");

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
    fn an_unterminated_deck_frontmatter_is_treated_as_body() {
        // Better to show the author their YAML than to swallow the whole file.
        let segments = split_default("---\ntitle: T\n\n# One");
        assert_eq!(segments.len(), 1);
        assert!(segments[0].frontmatter.is_none());
    }
}
