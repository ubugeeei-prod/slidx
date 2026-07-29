//! Where a run of not-code ends.
//!
//! Every function here answers one question — how far does this comment, this
//! string, this number reach — and every one of them is where the scanner is
//! most likely to be wrong, because each is a place the language stops meaning
//! what it usually means.
//!
//! They are separated from the driver for that reason: the driver decides
//! *which* question to ask, and this decides the answer. The two change for
//! different reasons, and reading either one should not mean reading both.
//!
//! The shared rule is that an *unclosed* construct ends at the end of the
//! fragment rather than being refused. A slide shows part of a file, and part
//! of a file routinely stops in the middle of a comment. The one exception is a
//! quoted string, which the driver stops at instead — see [`super`].

use crate::scan::{is_word_char, next_char};
use crate::syntax::{Comment, Quote, Syntax};

/// End of the comment opening at `at`, if one does.
pub(super) fn comment_end(source: &str, at: usize, syntax: &Syntax) -> Option<usize> {
    syntax.comments.iter().find_map(|comment| match *comment {
        Comment::Line(open) => line_comment_end(source, at, open, false),
        Comment::Word(open) => line_comment_end(source, at, open, true),
        Comment::Flat(open, close) => block_comment_end(source, at, open, close, false),
        Comment::Nested(open, close) => block_comment_end(source, at, open, close, true),
    })
}

fn line_comment_end(source: &str, at: usize, open: &str, word_start: bool) -> Option<usize> {
    if !source[at..].starts_with(open) {
        return None;
    }

    // Shell's `#`: mid-word it is a URL fragment, a colour, or a Git revision,
    // and greying out the rest of a command line is a slide nobody can read.
    if word_start && at != 0 && !source[..at].ends_with(char::is_whitespace) {
        return None;
    }

    Some(source[at..].find('\n').map_or(source.len(), |offset| at + offset))
}

fn block_comment_end(
    source: &str,
    at: usize,
    open: &str,
    close: &str,
    nested: bool,
) -> Option<usize> {
    if !source[at..].starts_with(open) {
        return None;
    }

    let mut depth = 1usize;
    let mut cursor = at + open.len();

    while cursor < source.len() {
        if source[cursor..].starts_with(close) {
            cursor += close.len();
            depth -= 1;
            if depth == 0 {
                return Some(cursor);
            }
            continue;
        }

        if nested && source[cursor..].starts_with(open) {
            cursor += open.len();
            depth += 1;
            continue;
        }

        cursor += next_char(source, cursor).len_utf8();
    }

    Some(source.len())
}

/// End of a Rust raw string or a Python triple-quoted string opening at `at`.
pub(super) fn long_string_end(source: &str, at: usize, syntax: &Syntax) -> Option<usize> {
    if syntax.raw_strings {
        if let Some(end) = raw_string_end(source, at) {
            return Some(end);
        }
    }

    if syntax.triple_quotes {
        for fence in ["\"\"\"", "'''"] {
            if source[at..].starts_with(fence) {
                let body = at + fence.len();
                let end = source[body..]
                    .find(fence)
                    .map_or(source.len(), |offset| body + offset + fence.len());

                return Some(end);
            }
        }
    }

    None
}

/// Rust's `r"…"` and `r#"…"#`, where a backslash is an ordinary character.
fn raw_string_end(source: &str, at: usize) -> Option<usize> {
    let rest = &source[at..];
    let opener = rest.strip_prefix("br").or_else(|| rest.strip_prefix('r'))?;

    let hashes = opener.chars().take_while(|&c| c == '#').count();
    let body = opener[hashes..].strip_prefix('"')?;

    let terminator = format!("\"{}", "#".repeat(hashes));
    let start = source.len() - body.len();

    Some(body.find(&terminator).map_or(source.len(), |offset| start + offset + terminator.len()))
}

pub(super) fn quote_at(source: &str, at: usize, syntax: &Syntax) -> Option<Quote> {
    syntax.quotes.iter().copied().find(|quote| source[at..].starts_with(quote.delimiter()))
}

pub(super) fn quoted_end(source: &str, at: usize, quote: Quote) -> Option<usize> {
    let delimiter = quote.delimiter();
    let mut cursor = at + delimiter.len_utf8();

    while cursor < source.len() {
        let character = next_char(source, cursor);

        if quote.escapes() && character == '\\' {
            cursor += 1;
            cursor += if cursor < source.len() { next_char(source, cursor).len_utf8() } else { 0 };
            continue;
        }

        cursor += character.len_utf8();
        if character == delimiter {
            return Some(cursor);
        }
    }

    None
}

/// A JSON key is a string a colon follows.
pub(super) fn colon_follows(source: &str, at: usize) -> bool {
    source[at..].trim_start().starts_with(':')
}

/// End of the character literal at `at`, or `None` when this is a lifetime.
///
/// The lookahead is the whole decision. It covers one character, one escape,
/// and `\u{…}`; anything longer is a name rather than a literal, which is the
/// only reading that is right more often than it is wrong.
pub(super) fn char_literal_end(source: &str, at: usize) -> Option<usize> {
    let body = &source[at + 1..];

    let closing = if let Some(escaped) = body.strip_prefix('\\') {
        match escaped.strip_prefix("u{") {
            Some(unicode) => at + 4 + unicode.find('}')? + 1,
            None => at + 2 + escaped.chars().next()?.len_utf8(),
        }
    } else {
        at + 1 + body.chars().next()?.len_utf8()
    };

    source.get(closing..)?.starts_with('\'').then_some(closing + 1)
}

/// End of the number literal at `at`, or `None` when one does not start there.
///
/// A digit that follows a word character belongs to that word: `utf8` is one
/// identifier, not a name and a number.
pub(super) fn number_end(source: &str, at: usize) -> Option<usize> {
    if !source[at..].starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    if at > 0 && source[..at].ends_with(is_word_char) {
        return None;
    }

    let mut cursor = at;
    let mut previous = '\0';

    for (offset, character) in source[at..].char_indices() {
        cursor = at + offset;

        // A `.` continues a number only when a digit follows it, so Rust's
        // `1..10` stays a range and JavaScript keeps `1.toFixed`'s method.
        if character == '.' {
            if !source[cursor + 1..].starts_with(|c: char| c.is_ascii_digit()) {
                return Some(cursor);
            }
        } else if matches!(character, '+' | '-') {
            // A sign belongs to a number only inside an exponent.
            if !matches!(previous, 'e' | 'E' | 'p' | 'P') {
                return Some(cursor);
            }
        } else if !is_word_char(character) {
            return Some(cursor);
        }

        previous = character;
        cursor = at + offset + character.len_utf8();
    }

    Some(cursor)
}
