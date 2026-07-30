//! The slide separator's spelling.
//!
//! `is_separator_of` tolerates up to three spaces of indentation and any
//! trailing whitespace, because editors add both. A separator that renders as a
//! slide break in one editor and a thematic rule in another is the failure this
//! exists to remove: the accepted spellings all mean the same thing, so the
//! file may as well say the one thing.
//!
//! Only the whitespace around the separator is normalised. `- - -` is a
//! Markdown thematic break and not a slidx separator, so slidx has no opinion
//! about it — but `  ---  ` is one, and the spaces carry nothing.

use slidx_core::scanner::is_separator_of;
use slidx_core::ByteSpan;
use slidx_edit::EditBuilder;

use crate::is_claimed;

/// Normalises every separator line outside a fence and outside frontmatter.
pub fn format(source: &str, separator: &str, claimed: &[ByteSpan], builder: &mut EditBuilder) {
    let mut cursor = 0usize;

    for line in source.split_inclusive('\n') {
        let text = line.trim_end_matches(['\n', '\r']);
        let span = ByteSpan::new(cursor, cursor + text.len());
        cursor += line.len();

        if is_separator_of(text, separator) && !is_claimed(claimed, span) {
            builder.replace(span, separator);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format_default(source: &str) -> String {
        let mut builder = EditBuilder::new(source);
        format(source, "---", &[], &mut builder);
        builder.build().apply(source)
    }

    #[test]
    fn an_indented_separator_moves_to_column_zero() {
        assert_eq!(format_default("# One\n\n  ---\n\n# Two\n"), "# One\n\n---\n\n# Two\n");
    }

    #[test]
    fn trailing_whitespace_on_a_separator_is_removed() {
        assert_eq!(format_default("# One\n\n---   \n\n# Two\n"), "# One\n\n---\n\n# Two\n");
    }

    #[test]
    fn a_longer_run_of_dashes_is_not_the_separator_and_is_left_alone() {
        // `----` is a thematic break, not a slide break: `split` does not break
        // on it, so normalising it here would invent a slide.
        assert_eq!(format_default("# One\n\n----\n\n# Two\n"), "# One\n\n----\n\n# Two\n");
    }

    #[test]
    fn a_markdown_thematic_break_is_not_rewritten_into_a_slide_break() {
        // `- - -` and `***` render as rules and mean nothing to slidx. Turning
        // one into a separator would split a slide the author did not split.
        for rule in ["- - -", "***", "___"] {
            let source = format!("# One\n\n{rule}\n\n# Two\n");
            assert_eq!(format_default(&source), source, "{rule}");
        }
    }

    #[test]
    fn a_frontmatter_delimiter_is_normalised_like_any_other_separator() {
        // It is the same predicate that recognises both, so the same spelling
        // has to come out.
        assert_eq!(
            format_default("  ---  \ntitle: T\n---\n\n# One\n"),
            "---\ntitle: T\n---\n\n# One\n"
        );
    }

    #[test]
    fn a_protected_span_is_left_alone() {
        let source = "```\n---\n```\n";
        let mut builder = EditBuilder::new(source);
        format(source, "---", &[ByteSpan::new(0, source.len())], &mut builder);

        assert!(builder.build().is_empty());
    }

    #[test]
    fn a_line_ending_is_never_part_of_the_span_that_is_replaced() {
        // Replacing the newline too would convert CRLF to LF on every
        // separator in a deck written on Windows.
        let source = "# One\r\n\r\n  ---  \r\n\r\n# Two\r\n";

        assert_eq!(format_default(source), "# One\r\n\r\n---\r\n\r\n# Two\r\n");
    }
}
