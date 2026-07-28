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

        marks.push(FoundMark { mark, start: index, end: attributes_end + 1 });
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

fn build(text: &str, attributes: &str) -> Option<Mark> {
    let mut mark = Mark::new(unescape(text));
    let mut saw_attribute = false;

    for token in tokenize(attributes) {
        saw_attribute = true;
        if let Some(key) = token.strip_prefix('#') {
            if key.is_empty() {
                return None;
            }
            mark.key = Some(key.to_string());
        } else if let Some(class) = token.strip_prefix('.') {
            if class.is_empty() {
                return None;
            }
            mark.classes.push(class.to_string());
        } else if let Some((name, value)) = token.split_once('=') {
            if name.is_empty() {
                return None;
            }
            mark.properties.insert(name.to_string(), unquote(value));
        } else {
            // A bare word is shorthand for a class, so `[x]{accent}` works.
            mark.classes.push(token);
        }
    }

    saw_attribute.then_some(mark)
}

/// Splits an attribute list, keeping quoted values whole.
fn tokenize(attributes: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;

    for character in attributes.chars() {
        match character {
            _ if escaped => {
                current.push(character);
                escaped = false;
            }
            '\\' => escaped = true,
            '"' => {
                quoted = !quoted;
                current.push(character);
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    match trimmed.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        Some(inner) => inner.replace("\\\"", "\"").replace("\\\\", "\\"),
        None => trimmed.to_string(),
    }
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
