//! Writing one key of a frontmatter block.
//!
//! Frontmatter is YAML, and YAML is where a round-tripping editor usually goes
//! wrong: load it into a map, dump it back, and the author's key order, their
//! comments, their quoting style and their block scalars are all gone. So this
//! never loads the block. It finds where one key's value is written and
//! replaces those bytes, which leaves every other key exactly as typed —
//! including the ones this version of slidx has never heard of.
//!
//! The deck's frontmatter is the first slide's. That is what the parser
//! already believes, so setting `title` on slide 0 is how a deck gets a title.

use serde_json::Value as JsonValue;

use slidx_core::scanner::is_separator_of;
use slidx_core::ByteSpan;

use crate::edit::EditBuilder;
use crate::op::{EditError, SlideRef};
use crate::source::DeckSource;

pub(crate) fn set_field(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    key: &str,
    value: &JsonValue,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    write_key(deck, index, key, &format!(" {}", scalar(value)), builder);

    Ok(())
}

/// Writes `key:` followed by `value`, which carries its own leading space or
/// newline because a scalar and a block list need different ones.
pub(crate) fn write_key(
    deck: &DeckSource<'_>,
    index: usize,
    key: &str,
    value: &str,
    builder: &mut EditBuilder<'_>,
) {
    let Some(block) = deck.at(index).frontmatter else {
        open_block(deck, index, &format!("{key}:{value}"), builder);
        return;
    };

    let text = block.slice(deck.source);
    match entry(text, key) {
        Some(found) => builder.replace(found.value.shifted(block.start), value),
        None if text.trim().is_empty() => builder.replace(block, format!("{key}:{value}")),
        None => builder.insert(block.end, format!("{}{key}:{value}", deck.newline())),
    }
}

/// Where a top-level key and its value are written inside a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Entry {
    /// The key line through the last line of its value.
    pub whole: ByteSpan,
    /// Everything after the colon, leading space included.
    pub value: ByteSpan,
}

/// Finds a top-level key in a raw frontmatter block.
///
/// Deliberately a line scan rather than a parse. A key's value runs to the
/// next key at column zero, which is true of scalars, block lists, block
/// scalars, and nested maps alike, and needs none of them to be understood.
pub(crate) fn entry(text: &str, key: &str) -> Option<Entry> {
    let mut found: Option<(usize, usize)> = None;

    for (start, line) in lines(text) {
        let Some(name) = top_level_key(line) else { continue };

        if found.is_some() {
            // The next key at column zero ends the one before it.
            return close(text, found?, start);
        }
        if name == key {
            found = Some((start, start + line.len()));
        }
    }

    close(text, found?, text.len())
}

fn close(text: &str, (start, key_end): (usize, usize), limit: usize) -> Option<Entry> {
    let end = start + text[start..limit].trim_end().len();
    let colon = start + text[start..key_end].find(':')? + 1;

    Some(Entry { whole: ByteSpan::new(start, end), value: ByteSpan::new(colon, end.max(colon)) })
}

/// The key a line declares, or `None` when the line is not a key at column
/// zero.
fn top_level_key(line: &str) -> Option<&str> {
    if line.starts_with([' ', '\t', '-', '#']) || line.trim().is_empty() {
        return None;
    }

    let (name, rest) = line.split_once(':')?;
    // `key:value` is a key; `https://example.com` is not, and neither is a
    // plain scalar that happens to contain a colon.
    (rest.is_empty() || rest.starts_with(' ') || !name.contains(' ')).then_some(name.trim())
}

pub(crate) fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut cursor = 0usize;

    std::iter::from_fn(move || {
        if cursor > text.len() {
            return None;
        }

        let start = cursor;
        let end = text[start..].find('\n').map_or(text.len(), |at| start + at);
        cursor = end + 1;

        Some((start, text[start..end].trim_end_matches('\r')))
    })
}

/// Gives a slide a frontmatter block it did not have.
///
/// The first slide's block opens the file, because that position is what makes
/// it the deck's. Any other slide's opens on the separator line that already
/// ends the slide above, which is the one place the parser looks for one.
fn open_block(deck: &DeckSource<'_>, index: usize, body: &str, builder: &mut EditBuilder<'_>) {
    let newline = deck.newline();
    let fresh =
        format!("{}{newline}{body}{newline}{}{}", deck.separator, deck.separator, deck.blank());

    if index == 0 {
        builder.insert(0, fresh);
        return;
    }

    let gap = deck.gap(index - 1);
    let Some(after) = separator_end(gap.slice(deck.source), deck.separator) else {
        // No separator to hang the block on, which means the slide above owns
        // it as its own delimiter. Write a fresh one.
        builder.insert(deck.at(index).content.start, fresh);
        return;
    };

    builder.insert(gap.start + after, format!("{body}{newline}{}{newline}", deck.separator));
}

/// Where the separator line's own newline ends inside a gap.
///
/// Measured from the newline itself rather than from the line's length, which
/// has already had a carriage return trimmed off it.
fn separator_end(text: &str, separator: &str) -> Option<usize> {
    let (start, _) = lines(text).find(|(_, line)| is_separator_of(line, separator))?;

    Some(text[start..].find('\n').map_or(text.len(), |at| start + at + 1))
}

/// Renders a JSON value as the YAML that means the same thing.
///
/// Quoting is decided by whether the plain form would read back as something
/// else. A title of `true` has to survive being a title, and a duration of
/// `20m` must not gain quotes it did not ask for.
fn scalar(value: &JsonValue) -> String {
    match value {
        JsonValue::String(text) => {
            if needs_quotes(text) {
                format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
            } else {
                text.clone()
            }
        }
        JsonValue::Null => "null".to_string(),
        // Flow style: JSON is valid YAML, so a list or a map arrives correct
        // and on one line, which keeps it to one line of diff.
        other => other.to_string(),
    }
}

fn needs_quotes(text: &str) -> bool {
    const RESERVED: [char; 14] =
        ['-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>'];
    const LOOK_TYPED: [&str; 8] = ["true", "false", "null", "yes", "no", "on", "off", "~"];

    text.is_empty()
        || text.trim() != text
        || text.starts_with(RESERVED)
        || text.starts_with(['\'', '"', '%', '@', '`'])
        || text.contains(": ")
        || text.contains(" #")
        || text.contains('\n')
        || LOOK_TYPED.contains(&text.to_ascii_lowercase().as_str())
        || text.parse::<f64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_and_its_value_are_found_by_scanning_rather_than_parsing() {
        let text = "title: Fast Decks\nduration: 20m";
        let found = entry(text, "duration").unwrap();

        assert_eq!(found.whole.slice(text), "duration: 20m");
        assert_eq!(found.value.slice(text), " 20m");
    }

    #[test]
    fn a_keys_value_runs_to_the_next_key_at_column_zero() {
        let text = "steps:\n  - reveal: \".a\"\n  - hide: \".b\"\ntitle: T";
        let found = entry(text, "steps").unwrap();

        assert_eq!(found.value.slice(text), "\n  - reveal: \".a\"\n  - hide: \".b\"");
    }

    #[test]
    fn the_last_key_in_a_block_ends_at_the_last_thing_it_says() {
        let text = "title: T\nsteps:\n  - reveal: \".a\"\n";
        let found = entry(text, "steps").unwrap();

        assert_eq!(found.whole.slice(text), "steps:\n  - reveal: \".a\"");
    }

    #[test]
    fn a_key_that_is_not_there_is_not_invented() {
        assert!(entry("title: T", "theme").is_none());
        assert!(entry("", "title").is_none());
    }

    #[test]
    fn a_list_item_that_looks_like_a_key_is_not_one() {
        let text = "steps:\n  - reveal: \".a\"";
        assert!(entry(text, "reveal").is_none());
    }

    #[test]
    fn a_value_with_a_colon_in_it_does_not_declare_a_key() {
        let text = "url: https://example.com/talks";
        assert_eq!(entry(text, "url").unwrap().value.slice(text), " https://example.com/talks");
        assert!(entry(text, "//example.com/talks").is_none());
    }

    #[test]
    fn values_that_would_read_back_as_another_type_are_quoted() {
        assert_eq!(scalar(&JsonValue::from("true")), "\"true\"");
        assert_eq!(scalar(&JsonValue::from("12")), "\"12\"");
        assert_eq!(scalar(&JsonValue::from("hello: there")), "\"hello: there\"");
        assert_eq!(scalar(&JsonValue::from("- dash")), "\"- dash\"");
        assert_eq!(scalar(&JsonValue::from("")), "\"\"");
    }

    #[test]
    fn values_that_read_back_as_themselves_are_left_bare() {
        assert_eq!(scalar(&JsonValue::from("Fast Decks")), "Fast Decks");
        assert_eq!(scalar(&JsonValue::from("20m")), "20m");
        assert_eq!(scalar(&JsonValue::from(true)), "true");
        assert_eq!(scalar(&JsonValue::from(90)), "90");
    }

    #[test]
    fn collections_are_written_in_flow_style_so_they_stay_on_one_line() {
        assert_eq!(scalar(&serde_json::json!(["a", "b"])), r#"["a","b"]"#);
    }
}
