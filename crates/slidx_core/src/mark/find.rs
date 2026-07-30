//! Recognising a mark in a source string.
//!
//! Separate from the model because it answers a different question: the model
//! says what a mark *is*, this says how to spot one in what an author wrote —
//! including everything that is deliberately *not* a mark.
//!
//! Everything here is forgiving. A half-typed mark exists constantly while
//! someone is editing, and none of them may make the rest of the slide vanish:
//! anything that does not parse is left as literal text.

use super::Mark;

/// A mark found in a source string, with the byte range it occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundMark {
    pub mark: Mark,
    pub start: usize,
    pub end: usize,
    /// Byte offset of the `{` that opens the attribute group.
    ///
    /// `attributes_start..end` is the only part of a mark a formatter is
    /// allowed to rewrite. Everything between the brackets is the author's
    /// prose, and re-emitting it would re-escape brackets they typed
    /// themselves. A translation splits the same way and in the same place, for
    /// the opposite reason: the prose is the whole point and the attributes are
    /// addresses a `steps:` entry points at.
    pub attributes_start: usize,
}

impl FoundMark {
    /// The bytes `{…}` occupies, attribute group and braces included.
    pub fn attributes_span(&self) -> std::ops::Range<usize> {
        self.attributes_start..self.end
    }
}

/// Finds every mark in a string, in source order.
///
/// Links are skipped, escaped brackets are skipped, and an unterminated mark
/// is left as literal text — a half-typed mark in the editor must not make the
/// rest of the slide disappear.
pub fn find_marks(source: &str) -> Vec<FoundMark> {
    let bytes = source.as_bytes();
    let mut marks = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                index += 2;
                continue;
            }
            b'[' => {}
            _ => {
                index += 1;
                continue;
            }
        }

        let Some(text_end) = matching_bracket(bytes, index) else {
            index += 1;
            continue;
        };

        // `]{` is the whole distinction from a link. CommonMark gives `{` no
        // meaning here, so claiming it cannot break an existing document.
        if bytes.get(text_end + 1) != Some(&b'{') {
            index = text_end + 1;
            continue;
        }

        let Some(attributes_end) = source[text_end + 1..].find('}').map(|at| text_end + 1 + at)
        else {
            index = text_end + 1;
            continue;
        };

        let Some(mark) = build(&source[index + 1..text_end], &source[text_end + 2..attributes_end])
        else {
            index = text_end + 1;
            continue;
        };

        marks.push(FoundMark {
            mark,
            start: index,
            end: attributes_end + 1,
            attributes_start: text_end + 1,
        });
        index = attributes_end + 1;
    }

    marks
}

/// Index of the `]` closing the `[` at `open`, respecting nesting and escapes.
fn matching_bracket(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 1,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }

    None
}

/// Builds a mark from the two halves a `[text]{attributes}` match yields.
///
/// The attribute grammar is [`crate::attributes`]'s and is not repeated here.
/// One grammar with two parsers is one grammar with two answers, and the mark's
/// spelling is the one an author learns first.
fn build(text: &str, attributes: &str) -> Option<Mark> {
    let parsed = crate::attributes::parse(attributes)?;

    Some(Mark {
        text: unescape(text),
        key: parsed.key,
        classes: parsed.classes,
        properties: parsed.properties,
    })
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut escaped = false;

    for character in text.chars() {
        if escaped {
            out.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            out.push(character);
        }
    }

    out
}
