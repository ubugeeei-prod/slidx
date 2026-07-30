//! Frontmatter key order and indentation.
//!
//! Frontmatter is the one part of a deck file that is not Markdown. It is a
//! slidx dialect written in YAML, so slidx gets to say what it looks like — and
//! a deck read a year later is easier to scan when `title:` is where `title:`
//! always is.
//!
//! # The safety net
//!
//! Reordering lines in somebody's YAML is the riskiest thing in this crate, so
//! it is checked rather than argued: the rewritten block is parsed again and
//! compared to what the original parsed to. Anything that would change the
//! meaning of the frontmatter — or stop it parsing at all — is discarded and
//! the block is left exactly as it was found.
//!
//! That is also why a block that does not parse is never touched. A deck is
//! parsed on every keystroke, so half-typed YAML is an ordinary input, and a
//! formatter run against one must not turn it into differently-broken YAML.
//!
//! # What is left alone
//!
//! A block scalar's body — `description: |` and what follows it — is prose in
//! every sense that matters: its relative indentation is part of the value.
//! Comments travel with the key underneath them, because a comment separated
//! from what it documents is worse than an unordered file.

use slidx_core::ByteSpan;
use slidx_edit::EditBuilder;

/// One level of nesting, in spaces.
///
/// Two, which is what `StepAction::to_source` already writes when the editor
/// adds a step. A formatter that disagreed with it would fight the editor over
/// the same lines.
const INDENT: usize = 2;

/// Every frontmatter key slidx reads, in the order it writes them.
///
/// Grouped by how often an author touches one, so the keys that change while a
/// deck is being written sit nearest the slide they describe. A key slidx does
/// not know — a theme's option, a plugin's — keeps its position relative to the
/// other unknown keys and follows all of these; frontmatter is deliberately
/// open, and a formatter that dropped or alphabetised somebody's plugin
/// configuration would be losing data to tidiness.
pub const ORDER: &[&str] = &[
    // The talk itself: what it is, how it looks, how long it runs.
    "title",
    "description",
    "author",
    "lang",
    "theme",
    "aspect",
    "duration",
    "safeArea",
    // Which of two decks of the same talk this is. Written by `slidx i18n
    // apply` rather than by hand, and next to the metadata it qualifies.
    "translationOf",
    // Where it was given and where it is published. Written once, at proposal
    // time, and rarely opened again.
    "event",
    "date",
    "venue",
    "hashtag",
    "url",
    "repo",
    "slug",
    "tags",
    "recording",
    // What one slide does. `id` leads, because it is the slide's address and
    // the one key here that something outside the deck depends on.
    "id",
    "transition",
    "layout",
    "budget",
    "optional",
    "autoSteps",
    "steps",
    "demo",
];

/// Normalises one frontmatter block, given the span of its YAML.
pub fn format(source: &str, span: ByteSpan, builder: &mut EditBuilder) {
    let text = span.slice(source);

    let Some(before) = parse(text) else {
        return;
    };

    let rewritten = rewrite(text);

    // The net: a rewrite that changes what the block means, or stops it being
    // readable at all, is not applied. Cheaper than being certain, and it holds
    // for the input nobody thought of.
    if parse(&rewritten).as_ref() != Some(&before) {
        return;
    }

    builder.replace(span, rewritten);
}

/// The block as a YAML mapping, or `None` for anything else.
///
/// A non-mapping block is already reported by the parser, and a block that does
/// not parse is somebody mid-keystroke. Neither is something to reformat.
fn parse(text: &str) -> Option<serde_yaml::Mapping> {
    match serde_yaml::from_str::<serde_yaml::Value>(text) {
        Ok(serde_yaml::Value::Mapping(mapping)) if !mapping.is_empty() => Some(mapping),
        _ => None,
    }
}

fn rewrite(text: &str) -> String {
    // A carriage return belongs to the terminator, not to the line, or a
    // reordered block moves one onto the end of a different line.
    let crlf = text.contains("\r\n");
    let lines: Vec<&str> = text.split('\n').map(|line| line.trim_end_matches('\r')).collect();

    let literal = literal_lines(&lines);
    let ladder = ladder(&lines, &literal);

    let mut entries = entries(&lines);
    // Stable, so unknown keys keep the order the author put them in and only
    // the keys slidx documents are moved.
    entries.sort_by_key(|entry| rank(entry.key));

    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for entry in &entries {
        for &index in &entry.lines {
            out.push(indented(lines[index], literal[index], &ladder));
        }
    }

    out.join(if crlf { "\r\n" } else { "\n" })
}

/// Rank in [`ORDER`], or one past the end for a key slidx does not know.
fn rank(key: &str) -> usize {
    ORDER.iter().position(|known| *known == key || kebab_case(known) == key).unwrap_or(ORDER.len())
}

fn kebab_case(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 2);
    for character in key.chars() {
        if character.is_ascii_uppercase() {
            out.push('-');
            out.push(character.to_ascii_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}

/// One top-level key and every line that belongs to it.
#[derive(Debug)]
struct Entry<'a> {
    key: &'a str,
    lines: Vec<usize>,
}

/// Splits the block into top-level entries, in source order.
///
/// A run of comments and blank lines attaches to the key *below* it, which is
/// where a YAML comment documents from. Anything trailing the last key has no
/// owner and is emitted last, under a key of `""` that no rank matches.
fn entries<'a>(lines: &[&'a str]) -> Vec<Entry<'a>> {
    let mut entries: Vec<Entry<'a>> = Vec::new();
    let mut pending: Vec<usize> = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let bare = line.trim();

        if bare.is_empty() || bare.starts_with('#') {
            pending.push(index);
            continue;
        }

        if indent_of(line) == 0 {
            let mut own = std::mem::take(&mut pending);
            own.push(index);
            entries.push(Entry { key: key_of(bare), lines: own });
            continue;
        }

        match entries.last_mut() {
            Some(entry) => {
                entry.lines.append(&mut pending);
                entry.lines.push(index);
            }
            // Indented before any key: not a mapping, so `parse` already
            // refused it. Kept for totality rather than reachability.
            None => pending.push(index),
        }
    }

    if !pending.is_empty() {
        entries.push(Entry { key: "", lines: pending });
    }

    entries
}

/// The key a top-level line declares, without its quotes.
fn key_of(bare: &str) -> &str {
    let name = bare.split(':').next().unwrap_or(bare).trim();

    name.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')).unwrap_or(name)
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// Which lines are the body of a block scalar, and therefore untouchable.
///
/// A `|` or `>` value keeps the relative indentation of what follows as part of
/// the value, so re-indenting it edits the text a slide displays.
fn literal_lines(lines: &[&str]) -> Vec<bool> {
    let mut literal = vec![false; lines.len()];
    let mut open: Option<usize> = None;

    for (index, line) in lines.iter().enumerate() {
        let bare = line.trim();

        if let Some(indent) = open {
            if bare.is_empty() || indent_of(line) > indent {
                literal[index] = true;
                continue;
            }
            open = None;
        }

        if opens_block_scalar(bare) {
            open = Some(indent_of(line));
        }
    }

    literal
}

/// True when a line's value is `|` or `>`, with any chomping or indentation
/// indicator after it.
fn opens_block_scalar(bare: &str) -> bool {
    let Some((_, value)) = bare.rsplit_once(':') else {
        return false;
    };

    let value = value.trim();
    let Some(rest) = value.strip_prefix(['|', '>']) else {
        return false;
    };

    rest.chars().all(|character| matches!(character, '+' | '-' | '0'..='9'))
}

/// Maps each indentation depth the author used onto a multiple of [`INDENT`].
///
/// Derived from the block rather than assumed, because a level is only as deep
/// as the levels above it. The mapping is order-preserving and injective, so
/// every parent still encloses every child.
fn ladder(lines: &[&str], literal: &[bool]) -> Vec<usize> {
    let mut depths: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(index, line)| !literal[*index] && !line.trim().is_empty())
        .map(|(_, line)| indent_of(line))
        .collect();

    depths.sort_unstable();
    depths.dedup();
    depths
}

/// One line at its canonical indentation.
fn indented(line: &str, literal: bool, ladder: &[usize]) -> String {
    let indent = indent_of(line);

    if literal || indent == 0 || line.trim().is_empty() {
        return line.to_string();
    }

    let level = ladder.iter().position(|depth| *depth == indent).unwrap_or(0);

    format!("{}{}", " ".repeat(level * INDENT), &line[indent..])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Formats a whole file's first frontmatter block.
    fn format_block(source: &str) -> String {
        let segments = slidx_core::parser::split(source, "---");
        let span = segments[0].frontmatter.as_ref().expect("a block").span;

        let mut builder = EditBuilder::new(source);
        format(source, span, &mut builder);
        builder.build().apply(source)
    }

    #[test]
    fn slidx_keys_are_written_in_the_documented_order() {
        assert_eq!(
            format_block("---\ntheme: minimal\nduration: 20m\ntitle: T\n---\n\n# One\n"),
            "---\ntitle: T\ntheme: minimal\nduration: 20m\n---\n\n# One\n"
        );
    }

    #[test]
    fn a_key_slidx_does_not_know_keeps_its_place_after_the_ones_it_does() {
        // Frontmatter is open on purpose: a theme reads keys this crate has
        // never heard of, and losing or alphabetising them would lose data.
        assert_eq!(
            format_block(
                "---\ngridOverlay: true\ntheme: minimal\nzOrder: 3\ntitle: T\n---\n\n# One\n"
            ),
            "---\ntitle: T\ntheme: minimal\ngridOverlay: true\nzOrder: 3\n---\n\n# One\n"
        );
    }

    #[test]
    fn a_key_written_in_kebab_case_sorts_where_its_camel_case_spelling_does() {
        // The parser accepts either spelling, so ranking only one of them would
        // move half the decks in the world to the bottom of the block.
        assert_eq!(
            format_block("---\nauto-steps: list\ntitle: T\n---\n\n- a\n"),
            "---\ntitle: T\nauto-steps: list\n---\n\n- a\n"
        );
    }

    #[test]
    fn a_comment_travels_with_the_key_it_documents() {
        assert_eq!(
            format_block("---\n# why editorial\ntheme: editorial\ntitle: T\n---\n\n# One\n"),
            "---\ntitle: T\n# why editorial\ntheme: editorial\n---\n\n# One\n"
        );
    }

    #[test]
    fn nesting_becomes_two_spaces_a_level() {
        assert_eq!(
            format_block("---\nsteps:\n    - reveal: \".a\"\n      after: 100\n---\n\n# One\n"),
            "---\nsteps:\n  - reveal: \".a\"\n    after: 100\n---\n\n# One\n"
        );
    }

    #[test]
    fn a_nested_mapping_keeps_its_shape_when_it_is_re_indented() {
        assert_eq!(
            format_block("---\ndemo:\n      live: https://app.example.com\n      fallback: ./c.mp4\n---\n\n# One\n"),
            "---\ndemo:\n  live: https://app.example.com\n  fallback: ./c.mp4\n---\n\n# One\n"
        );
    }

    #[test]
    fn a_block_scalar_keeps_the_indentation_that_is_part_of_its_value() {
        // Re-indenting this edits the text a slide displays, which is the one
        // thing this crate promises not to do. Everything around it is still
        // normalised.
        let source =
            "---\ntitle: T\ndescription: |\n    first\n      second\nsteps:\n     - reveal: \".a\"\n---\n\n# One\n";

        assert_eq!(
            format_block(source),
            "---\ntitle: T\ndescription: |\n    first\n      second\nsteps:\n  - reveal: \".a\"\n---\n\n# One\n"
        );
    }

    #[test]
    fn a_block_whose_order_would_change_a_literal_value_is_left_as_written() {
        // A `|` value that ends the block has no newline after it, and one that
        // does not, does — so moving `title:` above it would quietly add a
        // newline to the description. The safety net catches that rather than
        // this rule having to know about it.
        let source = "---\ndescription: |\n    first\ntitle: T\n---\n\n# One\n";

        assert_eq!(format_block(source), source);
    }

    #[test]
    fn a_block_that_does_not_parse_is_left_completely_alone() {
        // Half-typed YAML is an ordinary input: the parser sees one on every
        // keystroke. Turning it into differently-broken YAML helps nobody.
        let source = "---\ntitle: [unclosed\ntheme: minimal\n---\n\n# One\n";

        assert_eq!(format_block(source), source);
    }

    #[test]
    fn a_block_whose_keys_repeat_is_left_alone_rather_than_resolved() {
        // Which of two `title:` keys wins is YAML's business, and reordering
        // them could change the answer.
        let source = "---\ntitle: first\ntheme: minimal\ntitle: second\n---\n\n# One\n";

        assert_eq!(format_block(source), source);
    }

    #[test]
    fn an_empty_block_is_not_a_mapping_and_is_left_alone() {
        let source = "---\n---\n\n# One\n";
        assert_eq!(format_block(source), source);
    }

    #[test]
    fn a_value_is_never_requoted_or_restyled() {
        // Quoting is the author's, and `serde_yaml`'s idea of it is not.
        let source = "---\ntitle: \"T\"\naspect: '16:9'\n---\n\n# One\n";
        assert_eq!(format_block(source), source);
    }

    #[test]
    fn formatting_a_block_is_idempotent() {
        let source = "---\nsteps:\n   - reveal: \".a\"\ntheme: minimal\ntitle: T\n---\n\n# One\n";
        let once = format_block(source);

        assert_eq!(format_block(&once), once);
    }

    #[test]
    fn every_key_in_the_canonical_order_is_named_once() {
        let mut names = ORDER.to_vec();
        let total = names.len();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), total, "a key is ranked twice");
    }

    #[test]
    fn the_canonical_order_ranks_the_keys_a_deck_is_identified_by_first() {
        assert_eq!(rank("title"), 0);
        assert!(rank("title") < rank("theme"));
        assert!(rank("theme") < rank("steps"));
        assert_eq!(rank("gridOverlay"), ORDER.len(), "an unknown key sorts last");
    }
}
