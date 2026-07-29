//! Speaker notes.
//!
//! A slide's notes are one thing to the speaker and any number of comments in
//! the file, so setting them rewrites the first comment in place and removes
//! the others. Rewriting in place matters: the prose between two notes is not
//! notes, and an operation that replaced the whole region would take it away.
//!
//! A removed comment takes the line it sat on, and one of the blank lines
//! around it when it had one on each side. Otherwise every note a speaker
//! deletes leaves a widening gap behind, which is a diff about nothing.

use slidx_core::{find_notes, ByteSpan, FoundNote};

use crate::edit::EditBuilder;
use crate::op::{EditError, SlideRef};
use crate::source::DeckSource;

pub(crate) fn set(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    notes: &str,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let found = find_notes(body.slice(deck.source));
    let notes = notes.trim();

    let kept = match (found.first(), notes.is_empty()) {
        (Some(first), false) => {
            builder.replace(first.text_span.shifted(body.start), notes);
            1
        }
        (None, false) => {
            builder.insert(body.end, format!("{}<!-- notes: {notes} -->", deck.blank()));
            0
        }
        (_, true) => 0,
    };

    for note in found.iter().skip(kept) {
        builder.delete(around(deck.source, note, body.start));
    }

    Ok(())
}

/// The comment, plus the line it sat on when it had the line to itself.
fn around(source: &str, note: &FoundNote, offset: usize) -> ByteSpan {
    let span = note.span.shifted(offset);
    let before = &source[..span.start];
    let after = &source[span.end..];

    // A comment sharing its line with something else takes only itself.
    let line_end = span.end + (after.len() - after.trim_start_matches([' ', '\t']).len());
    let terminator = newline_len(&source[line_end..]);
    let alone = (before.is_empty() || before.ends_with('\n'))
        && (terminator > 0 || source[line_end..].is_empty());

    if !alone {
        return span;
    }

    let mut end = line_end + terminator;

    // Blank above and blank below: one of them was the comment's, and keeping
    // both would leave a hole that grows every time a note is deleted.
    if ends_with_blank_line(before) {
        end += newline_len(&source[end..]);
    }

    ByteSpan::new(span.start, end)
}

/// Length of the line ending at the start of `text`, or zero for anything else.
fn newline_len(text: &str) -> usize {
    match text.as_bytes() {
        [b'\r', b'\n', ..] => 2,
        [b'\n', ..] => 1,
        _ => 0,
    }
}

fn ends_with_blank_line(text: &str) -> bool {
    text.strip_suffix('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .is_some_and(|line| line.ends_with('\n'))
}
