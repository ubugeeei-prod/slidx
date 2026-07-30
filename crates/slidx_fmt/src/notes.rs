//! The shape of a notes comment.
//!
//! `notes:`, `note:`, `notes` and `note` all open a note, and a note is the one
//! construct in a deck whose contents must not reach the audience. Reading a
//! diff for a forgotten note is easier when every note in the file looks the
//! same, so the keyword is spelled `notes:` and the delimiters sit where the
//! README says they sit.
//!
//! The note's own lines are copied byte for byte, indentation included. They
//! are the speaker's prose: a formatter that rewrapped them would be
//! rewriting what somebody plans to say, and it would do it in the one place
//! nobody proofreads because it never appears on a slide.

use slidx_core::notes::{find_notes, keyword_len};
use slidx_core::ByteSpan;
use slidx_edit::EditBuilder;

use crate::claim;

const OPEN: &str = "<!--";
const CLOSE: &str = "-->";
const KEYWORD: &str = "notes:";

/// Normalises every notes comment in a slide body.
pub fn format(
    source: &str,
    body: ByteSpan,
    claimed: &mut Vec<ByteSpan>,
    builder: &mut EditBuilder,
) {
    let text = &source[body.start..body.end];

    for found in find_notes(text) {
        let span = ByteSpan::new(body.start + found.span.start, body.start + found.span.end);
        if !claim(claimed, span) {
            continue;
        }

        if let Some(canonical) = canonical(span.slice(source)) {
            builder.replace(span, canonical);
        }
    }
}

/// The canonical spelling of one whole `<!-- … -->` comment.
///
/// `None` when the comment is not a note after all, which `find_notes` has
/// already ruled out — kept as a total function so the caller has no branch
/// that depends on the two agreeing.
fn canonical(comment: &str) -> Option<String> {
    let inner = comment.strip_prefix(OPEN)?.strip_suffix(CLOSE)?;
    let after_open = inner.trim_start();
    let rest = &after_open[keyword_len(after_open)?..];

    // How many lines a note takes is the author's business, so the keyword's
    // own line keeps whatever was written on it and the rest keep theirs.
    let (head, tail, newline) = match rest.find('\n') {
        Some(at) => {
            let newline = if rest[..at].ends_with('\r') { "\r\n" } else { "\n" };
            (rest[..at].trim(), rest[at + 1..].trim_end(), newline)
        }
        None => (rest.trim(), "", "\n"),
    };

    let opening = match head.is_empty() {
        true => format!("{OPEN} {KEYWORD}"),
        false => format!("{OPEN} {KEYWORD} {head}"),
    };

    // A note with nothing under the keyword closes on the same line, so a
    // one-line note does not become three.
    Some(match tail.is_empty() {
        true => format!("{opening} {CLOSE}"),
        false => format!("{opening}{newline}{tail}{newline}{CLOSE}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format_all(source: &str) -> String {
        let mut builder = EditBuilder::new(source);
        format(source, ByteSpan::new(0, source.len()), &mut Vec::new(), &mut builder);
        builder.build().apply(source)
    }

    #[test]
    fn every_accepted_keyword_becomes_the_one_spelling() {
        for keyword in ["notes:", "note:", "notes", "note", "NOTES:"] {
            assert_eq!(
                format_all(&format!("# One\n\n<!-- {keyword} say hello -->\n")),
                "# One\n\n<!-- notes: say hello -->\n",
                "{keyword}"
            );
        }
    }

    #[test]
    fn a_one_line_note_is_spaced_once_on_each_side() {
        assert_eq!(format_all("<!--notes:say hello-->\n"), "<!-- notes: say hello -->\n");
        assert_eq!(format_all("<!--   notes:   say hello   -->\n"), "<!-- notes: say hello -->\n");
    }

    #[test]
    fn a_one_line_note_is_not_broken_onto_several() {
        // How many lines a note takes is the author's business. A formatter
        // that expanded every note would rewrite a file nobody edited.
        let source = "<!-- notes: say hello -->\n";
        assert_eq!(format_all(source), source);
    }

    #[test]
    fn a_multi_line_note_keeps_the_indentation_of_its_own_lines() {
        // The note is the speaker's prose. Its shape is what they will read off
        // a second screen while talking.
        let source = "<!--notes:\n  open with the outcome\n    then the agenda\n-->\n";

        assert_eq!(
            format_all(source),
            "<!-- notes:\n  open with the outcome\n    then the agenda\n-->\n"
        );
    }

    #[test]
    fn a_multi_line_note_gets_its_delimiters_onto_their_own_lines() {
        assert_eq!(
            format_all("<!-- note: first\nsecond -->\n"),
            "<!-- notes: first\nsecond\n-->\n"
        );
    }

    #[test]
    fn a_note_in_a_crlf_file_keeps_crlf_on_the_lines_the_formatter_writes() {
        let source = "<!--notes:\r\n  a prompt\r\n-->\r\n";

        assert_eq!(format_all(source), "<!-- notes:\r\n  a prompt\r\n-->\r\n");
    }

    #[test]
    fn a_comment_that_is_not_a_note_is_left_alone() {
        for comment in ["<!-- step -->", "<!-- notesomething -->", "<!-- TODO: x -->"] {
            let source = format!("# One\n\n{comment}\n");
            assert_eq!(format_all(&source), source, "{comment}");
        }
    }

    #[test]
    fn an_empty_note_is_still_a_note() {
        assert_eq!(format_all("<!--notes:-->\n"), "<!-- notes: -->\n");
    }

    #[test]
    fn a_note_inside_a_fence_is_left_alone() {
        let source = "```md\n<!--note: x-->\n```\n";
        let mut builder = EditBuilder::new(source);
        let whole = ByteSpan::new(0, source.len());
        format(source, whole, &mut vec![whole], &mut builder);

        assert!(builder.build().is_empty());
    }

    #[test]
    fn several_notes_on_one_slide_are_all_normalised() {
        assert_eq!(
            format_all("<!--note: a-->\n\n# One\n\n<!--notes: b-->\n"),
            "<!-- notes: a -->\n\n# One\n\n<!-- notes: b -->\n"
        );
    }
}
