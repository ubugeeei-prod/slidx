//! # slidx fmt
//!
//! Normalises the parts of a deck that belong to slidx, and nothing else.
//!
//! ## What it will not do, and why that is the design
//!
//! A slidx deck is Markdown the author owns. `slidx_edit` exists because
//! parsing a deck, mutating a model and serialising it back regularises blank
//! lines, turns `*` bullets into `-`, and rewraps a hand-wrapped paragraph —
//! every one of them invisible on the canvas and enormous in the diff. A
//! *formatter* that did the same thing would produce exactly that diff on
//! purpose, on every file, to a document nobody asked anyone to reflow.
//!
//! So this is not a Markdown formatter and must never become one. It touches
//! five constructs, all of which slidx invented:
//!
//! | Construct | Normalised to |
//! |---|---|
//! | frontmatter | slidx's keys first, in one order; two spaces per level |
//! | slide separator | the separator's own spelling, at column zero |
//! | step marker | `<!-- step -->`, or `<!-- step: preset -->` |
//! | mark attributes | `#key`, classes as written, properties sorted |
//! | notes comment | `<!-- notes: … -->`, keyword spelled `notes:` |
//!
//! Prose, line wrapping, bullet markers, emphasis spelling, table alignment,
//! heading style, trailing whitespace on a prose line, and every byte inside a
//! fenced code block are left exactly as they were found. That is a property
//! rather than an intention: see `tests/format_properties.rs`, which states it
//! over generated decks including CJK, emoji, and zero-width fragments.
//!
//! ## An edit, not a rewrite
//!
//! [`plan`] returns a [`slidx_edit::Edit`] — the byte ranges that change and
//! what replaces them — for the same reason the editor's operations do. The
//! bytes it does not name are never read and never rewritten, so "leaves the
//! rest alone" is structural rather than something each rule has to remember.
//!
//! Idempotence is inherited from the same place: `EditBuilder` drops a
//! replacement that matches the text it would replace, so a file already in
//! canonical form plans an *empty* edit and `--check` has nothing to report.
//!
//! ```
//! use slidx_core::DeckParseOptions;
//!
//! let source = "---\ntheme: editorial\ntitle: Fast Decks\n---\n\n#   Hello\n\n- a\n- b\n";
//! let formatted = slidx_fmt::format(source, &DeckParseOptions::default());
//!
//! assert_eq!(
//!     formatted,
//!     "---\ntitle: Fast Decks\ntheme: editorial\n---\n\n#   Hello\n\n- a\n- b\n",
//! );
//! ```
//!
//! The keys are reordered. The three spaces after the `#` and the `-` bullets
//! are not slidx's to have an opinion about.

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod frontmatter;
pub mod inline;
pub mod marker;
pub mod notes;
pub mod separator;

use slidx_core::parser::split;
use slidx_core::scanner::FenceTracker;
use slidx_core::{ByteSpan, DeckParseOptions};
use slidx_edit::{Edit, EditBuilder};

/// Works out which bytes formatting changes, without changing them.
///
/// Empty when the source is already canonical, which is what `--check` reads.
pub fn plan(source: &str, options: &DeckParseOptions) -> Edit {
    let mut builder = EditBuilder::new(source);

    // Bytes no later rule may enter. Seeded with the fenced code blocks and the
    // frontmatter, and grown by each rule as it claims what it rewrote.
    //
    // The order the rules run in is the order the parser reads them: notes are
    // lifted out of a body before step markers are resolved, and markers become
    // anchors before marks are found. That matters on a half-typed file, where
    // an unclosed `]{` swallows the comment below it and two rules would
    // otherwise both rewrite the same bytes — the one the parser would have
    // believed has to win.
    let mut claimed = Vec::new();

    for segment in split(source, &options.separator) {
        if let Some(matter) = &segment.frontmatter {
            frontmatter::format(source, matter.span, &mut builder);
            claimed.push(matter.span);
        }

        let body = segment.body_span;
        let mut inside = fenced_spans(source, body);

        if is_settled(source, body) {
            notes::format(source, body, &mut inside, &mut builder);
            marker::format(source, body, &mut inside, &mut builder);
            inline::format(source, body, &mut inside, &mut builder);
        }

        claimed.extend(inside);
    }

    separator::format(source, &options.separator, &claimed, &mut builder);

    builder.build()
}

/// The source in canonical form.
pub fn format(source: &str, options: &DeckParseOptions) -> String {
    plan(source, options).apply(source)
}

/// True when every mark in this body closes its attributes where it opened them.
///
/// An `]{` with no `}` on its line reaches down the slide for one, and what it
/// swallows on the way is an accident of where the next `}` happens to be. The
/// parser reads it that way too — but that reading changes when the author types
/// one more character, and it would change again if this crate rewrote a comment
/// anywhere inside the range. Formatting a body in that state is not so much
/// wrong as meaningless, so it is not attempted: the slide is left exactly as it
/// was found, and formats on the next save.
///
/// A mark whose *text* spans lines is fine and stays formattable. Somebody
/// hand-wrapping a paragraph around a marked phrase has written something
/// ordinary, and only the attribute group is ever rewritten.
fn is_settled(source: &str, body: ByteSpan) -> bool {
    let text = &source[body.start..body.end];

    slidx_core::mark::find_marks(text)
        .iter()
        .all(|found| !text[found.attributes_start..found.end].contains('\n'))
}

/// Every byte range inside `body` that a fenced code block occupies.
///
/// Shared by the three rules that read a slide body, because a talk about
/// slidx puts a step marker, a mark and a notes comment inside a fence on
/// purpose. Text in a fence is code, and code is a slide doing its job.
///
/// The delimiter lines count as part of the block: rewriting an info string
/// would be an opinion about somebody's code sample.
fn fenced_spans(source: &str, body: ByteSpan) -> Vec<ByteSpan> {
    let mut fences = FenceTracker::new();
    let mut spans = Vec::new();
    let mut cursor = body.start;

    for line in source[body.start..body.end].split_inclusive('\n') {
        let end = cursor + line.len();

        // `feed` reports content, and a delimiter is not content — so a false
        // answer covers the opening fence, the body, and the closing fence
        // alike.
        if !fences.feed(line.trim_end_matches(['\n', '\r'])) {
            spans.push(ByteSpan::new(cursor, end));
        }

        cursor = end;
    }

    spans
}

/// Records a construct, and says whether it is free to rewrite.
///
/// A construct is claimed whether or not anything is written to it, because the
/// claim is about which rule *owns* the bytes rather than about what happened to
/// them. A notes comment reaching into a fenced block is the case that forces
/// it: the note is not rewritten, because a fence is untouchable — but the
/// parser still lifts those bytes out as a note, so a mark sitting inside them
/// is not a mark and must not be normalised either.
fn claim(claimed: &mut Vec<ByteSpan>, span: ByteSpan) -> bool {
    let free = !is_claimed(claimed, span);
    claimed.push(span);
    free
}

/// True when `span` overlaps any of `claimed`.
fn is_claimed(claimed: &[ByteSpan], span: ByteSpan) -> bool {
    claimed.iter().any(|other| span.start < other.end && other.start < span.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> String {
        format(source, &DeckParseOptions::default())
    }

    #[test]
    fn a_deck_already_in_canonical_form_plans_no_edit_at_all() {
        // Not "an edit that produces the same bytes" — no edit. That is what
        // makes `--check` a question about the file rather than about a diff.
        let source = "---\ntitle: T\n---\n\n# One\n\n<!-- notes: say hello -->\n";

        assert!(plan(source, &DeckParseOptions::default()).is_empty());
    }

    #[test]
    fn formatting_is_idempotent() {
        let source =
            "---\ntheme: minimal\ntitle:  T\n---\n\n- a <!--step-->\n\n<!-- note:  x -->\n";
        let once = run(source);

        assert_eq!(run(&once), once);
    }

    #[test]
    fn every_construct_is_normalised_in_one_pass() {
        // The five rules have to compose: an author runs the formatter once.
        let source = "---\ntheme: minimal\ntitle: T\n---\n\n\
                      # One [x]{.a #k} <!--step: fly-in-->\n\n<!-- note: a prompt -->\n\
                      \n  ---  \n\n# Two\n";

        assert_eq!(
            run(source),
            "---\ntitle: T\ntheme: minimal\n---\n\n\
             # One [x]{#k .a} <!-- step: fly-in -->\n\n<!-- notes: a prompt -->\n\
             \n---\n\n# Two\n"
        );
    }

    #[test]
    fn nothing_inside_a_fenced_code_block_is_touched() {
        // A deck about slidx shows this markup on a slide. Rewriting it would
        // make the slide wrong about the thing it is teaching.
        let source = "# Talk\n\n```md\n---\ntheme: minimal\ntitle: T\n---\n\n\
                      - a <!--step-->\n\n[x]{.a #k}\n\n<!-- note: x -->\n```\n";

        assert_eq!(run(source), source);
    }

    #[test]
    fn a_deck_written_with_crlf_keeps_its_carriage_returns() {
        // A deck emailed between two machines arrives with whichever line
        // ending the sender's editor uses, and converting them silently is a
        // diff on every line of a file nobody edited.
        let source = "---\r\ntheme: minimal\r\ntitle: T\r\n---\r\n\r\n# One\r\n";
        let formatted = run(source);

        assert_eq!(formatted, "---\r\ntitle: T\r\ntheme: minimal\r\n---\r\n\r\n# One\r\n");
        assert!(!formatted.contains("\n\n"), "a carriage return was dropped");
    }

    #[test]
    fn a_custom_separator_is_the_one_that_gets_normalised() {
        // A deck that shows Markdown source configures its own separator, and
        // `---` inside it is content the author is quoting.
        let options = DeckParseOptions { separator: "===".to_string(), ..Default::default() };
        let source = "# One\n\n---\n\n  ===  \n\n# Two\n";

        assert_eq!(format(source, &options), "# One\n\n---\n\n===\n\n# Two\n");
    }

    #[test]
    fn a_slide_whose_mark_never_closed_its_attributes_is_left_alone() {
        // The `{` reached down the slide for a `}` and took the note with it.
        // What the parser makes of that changes with the next character typed,
        // so there is nothing here worth normalising — and rewriting the note
        // would change what the swallowing mark contains.
        let source = "[three]{#k\n\n<!--note: a prompt-->\n\n[x]{.a #j}\n";

        assert_eq!(run(source), source);
    }

    #[test]
    fn a_marked_phrase_wrapped_across_two_lines_is_still_formatted() {
        // Only the attribute group is ever rewritten, so a hand-wrapped
        // paragraph with a mark across the wrap is ordinary, not ambiguous.
        assert_eq!(
            run("[three important\nwords]{.a #k} and more\n"),
            "[three important\nwords]{#k .a} and more\n"
        );
    }

    #[test]
    fn a_separator_inside_a_frontmatter_value_is_not_a_separator() {
        // `split` never scans a frontmatter block for boundaries, so neither
        // may this — a rule that disagreed with the parser about where a slide
        // begins would move a line out of somebody's YAML.
        let source = "---\ntitle: T\ndescription: \"a --- dash\"\n---\n\n# One\n";

        assert_eq!(run(source), source);
    }
}
