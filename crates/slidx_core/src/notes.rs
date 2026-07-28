//! Speaker notes.
//!
//! Notes live next to the slide they belong to, in an HTML comment, so a deck
//! stays one reviewable file and notes survive `git diff`. They are stripped
//! from the public body before it ever reaches a renderer, which means a
//! forgotten "don't mention the outage" can never ship to the audience.

/// A slide body with its notes removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtractedNotes {
    pub content: String,
    pub notes: Vec<String>,
}

const OPEN: &str = "<!--";
const CLOSE: &str = "-->";

/// Prefixes that mark a comment as speaker notes.
const NOTE_PREFIXES: [&str; 4] = ["notes:", "note:", "notes", "note"];

/// Splits speaker notes out of a slide body.
///
/// Comments that are not notes are left untouched: authors use them for
/// step markers, editor hints, and ordinary Markdown asides.
pub fn extract_notes(content: &str) -> ExtractedNotes {
    let mut body = String::with_capacity(content.len());
    let mut notes = Vec::new();
    let mut rest = content;

    while let Some(open_at) = rest.find(OPEN) {
        let after_open = &rest[open_at + OPEN.len()..];
        let Some(close_at) = after_open.find(CLOSE) else {
            // An unterminated comment is the author's problem to fix, but the
            // remaining text still belongs on the slide.
            break;
        };

        let inner = &after_open[..close_at];
        match note_body(inner) {
            Some(note) => {
                body.push_str(&rest[..open_at]);
                notes.push(note);
            }
            None => {
                body.push_str(&rest[..open_at + OPEN.len() + close_at + CLOSE.len()]);
            }
        }

        rest = &after_open[close_at + CLOSE.len()..];
    }

    body.push_str(rest);

    ExtractedNotes { content: collapse_blank_runs(body.trim()), notes }
}

/// Returns the note text if the comment is a notes comment.
fn note_body(inner: &str) -> Option<String> {
    let trimmed = inner.trim();
    let lowered = trimmed.to_ascii_lowercase();

    for prefix in NOTE_PREFIXES {
        if !lowered.starts_with(prefix) {
            continue;
        }

        let remainder = &trimmed[prefix.len()..];
        // `notes:` and `notes` match, but `notesomething` must not.
        if prefix.ends_with(':')
            || remainder.is_empty()
            || remainder.starts_with(char::is_whitespace)
        {
            // Dedent before trimming: trimming first would strip the first
            // line's indentation and make the block look ragged.
            return Some(dedent(remainder).trim().to_string());
        }
    }

    None
}

/// Removes the common leading indentation from a multi-line note.
///
/// Notes are usually indented to sit under their comment marker; keeping that
/// indentation would make presenter view render them as code blocks.
fn dedent(text: &str) -> String {
    let indent = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    if indent == 0 {
        return text.to_string();
    }

    text.lines()
        .map(|line| if line.len() >= indent { &line[indent..] } else { line.trim_start() })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collapses the blank-line runs left behind by a removed comment.
fn collapse_blank_runs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0usize;

    for line in text.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }

        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }

    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slide_without_comments_is_unchanged() {
        let result = extract_notes("# One\n\nBody.");
        assert_eq!(result.content, "# One\n\nBody.");
        assert!(result.notes.is_empty());
    }

    #[test]
    fn a_notes_comment_is_lifted_out() {
        let result = extract_notes("# One\n\n<!-- notes: remember the demo -->\n");
        assert_eq!(result.content, "# One");
        assert_eq!(result.notes, vec!["remember the demo"]);
    }

    #[test]
    fn multiline_notes_are_dedented() {
        let result = extract_notes("# One\n\n<!-- notes:\n  first line\n  second line\n-->\n");
        assert_eq!(result.notes, vec!["first line\nsecond line"]);
    }

    #[test]
    fn several_note_blocks_are_collected_in_order() {
        let result = extract_notes("<!-- note: a -->\n# One\n<!-- notes: b -->");
        assert_eq!(result.notes, vec!["a", "b"]);
    }

    #[test]
    fn the_notes_keyword_is_case_insensitive() {
        assert_eq!(extract_notes("<!-- NOTES: shout -->").notes, vec!["shout"]);
    }

    #[test]
    fn a_bare_notes_keyword_without_a_colon_works() {
        assert_eq!(extract_notes("<!-- notes\nbody\n-->").notes, vec!["body"]);
    }

    #[test]
    fn comments_that_only_look_like_notes_are_left_alone() {
        let result = extract_notes("<!-- notesomething -->");
        assert!(result.notes.is_empty());
        assert_eq!(result.content, "<!-- notesomething -->");
    }

    #[test]
    fn unrelated_comments_stay_in_the_body() {
        let result = extract_notes("# One\n<!-- step -->\nrest");
        assert!(result.notes.is_empty());
        assert!(result.content.contains("<!-- step -->"));
    }

    #[test]
    fn an_unterminated_comment_does_not_eat_the_slide() {
        let result = extract_notes("# One\n\n<!-- notes: oops\n\nstill here");
        assert!(result.content.contains("# One"));
        assert!(result.content.contains("still here"));
    }

    #[test]
    fn removing_a_note_does_not_leave_a_gap() {
        let result = extract_notes("# One\n\n<!-- notes: hidden -->\n\nAfter.");
        assert_eq!(result.content, "# One\n\nAfter.");
    }

    #[test]
    fn an_empty_note_is_still_recorded() {
        // A stub note is a deliberate placeholder, not noise to discard.
        assert_eq!(extract_notes("<!-- notes: -->").notes, vec![""]);
    }
}
