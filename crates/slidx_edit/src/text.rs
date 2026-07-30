//! Typing on the slide: replacing a run of a body's text where it is written.
//!
//! Everything else in this crate changes a *structure* — a slide's order, a
//! block's region, a step's timing. This changes prose, which is what an author
//! spends the day doing, and it is the one operation whose range comes from a
//! caret rather than from a click on a thing.
//!
//! The range is measured in the slide's **source body**, the same coordinates a
//! mark's is, because that is what the editor can slice for itself. Turning
//! "these characters on screen" into those bytes is the editor's job and is done
//! against the spans [`crate::SlideSpans`] hands it; deciding what the bytes
//! then become is this module's, and nowhere else.
//!
//! # Why a mark cannot be edited by accident
//!
//! `[120ms]{#latency}` renders as five characters and is seventeen bytes, and
//! the twelve the reader never sees are an address: a `steps:` entry animates
//! that key, and a theme colours that class. A replacement over the naive range
//! would take both away, and the author would find out at the next rehearsal.
//!
//! So two rules, and they are the whole design:
//!
//! **A range inside one mark's words splices inside them.** The brackets and the
//! group are not in the range and are never rewritten, so typing a digit into
//! `120ms` produces exactly the bytes typing it in a text editor would.
//!
//! **A range that crosses a mark's edge leaves the mark holding the words of it
//! that survived, and what was typed lands outside.** Selecting from the middle
//! of `[120ms]{#latency}` into the prose after it and typing gives
//! `[120]{#latency}…` — the key survives, and the new words are not silently
//! adopted into a mark the author was deleting half of. A mark left holding no
//! words at all goes with them, because `[]{#latency}` is not something a person
//! meant to write; that is the rule a mark's attributes already keep.
//!
//! # What is escaped and what is not
//!
//! Text written outside a mark is written as typed. An author who types `**` on
//! the canvas is typing Markdown into a Markdown file and gets emphasis, which
//! is the point of editing the file in place rather than a shortcoming of it.
//!
//! Inside a mark the brackets are escaped, and only they. An unbalanced `]`
//! there would end the mark early and lose the key — the failure this module
//! exists to prevent, arriving by another door.

use slidx_core::mark::{escape_text, FoundMark};
use slidx_core::{find_marks, ByteSpan};

use crate::edit::EditBuilder;
use crate::op::{EditError, SlideRef};
use crate::source::DeckSource;

pub(crate) fn set(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    range: ByteSpan,
    text: &str,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let source = body.slice(deck.source);

    if !addressable(source, range) {
        return Err(EditError::UnusableRange { range });
    }

    let (span, replacement) = narrowed(source, rewritten(source, range, text));
    builder.replace(span.shifted(body.start), replacement);

    Ok(())
}

/// True when a range names bytes of this body that could be spliced.
///
/// A range from an editor holding a copy of the file from a keystroke ago is
/// ordinary traffic, so this is a question rather than an assertion.
fn addressable(source: &str, range: ByteSpan) -> bool {
    range.start <= range.end
        && range.end <= source.len()
        && source.is_char_boundary(range.start)
        && source.is_char_boundary(range.end)
}

/// The bytes the operation affects, and what takes their place.
///
/// Wider than the range whenever a mark has to be rewritten, because a mark is
/// only correct as a whole: half of one is prose that happens to start with a
/// bracket.
fn rewritten(source: &str, range: ByteSpan, text: &str) -> (ByteSpan, String) {
    let marks = find_marks(source);

    if let Some(mark) = marks.iter().find(|mark| holds(mark, range)) {
        let words = words_of(mark);
        let survives = range.start > words.start || range.end < words.end || !text.is_empty();

        return if survives {
            (range, escape_text(text))
        } else {
            (ByteSpan::new(mark.start, mark.end), String::new())
        };
    }

    let touched: Vec<&FoundMark> = marks.iter().filter(|mark| reaches(mark, range)).collect();
    let Some((first, last)) = touched.first().zip(touched.last()) else {
        return (range, text.to_string());
    };

    let affected = ByteSpan::new(range.start.min(first.start), range.end.max(last.end));
    let mut rewrite = Rewrite { source, range, text, out: String::new(), written: false };
    let mut cursor = affected.start;

    for mark in &touched {
        rewrite.plain(ByteSpan::new(cursor, mark.start));
        rewrite.mark(mark);
        cursor = mark.end;
    }
    rewrite.plain(ByteSpan::new(cursor, affected.end));

    (affected, rewrite.out)
}

/// True when the range lies entirely within this mark's words.
fn holds(mark: &FoundMark, range: ByteSpan) -> bool {
    words_of(mark).contains(range)
}

/// True when the range reaches into this mark from outside, or out of it.
///
/// A mark the range merely surrounds is not one of these: it disappears with the
/// words it marked, which is what selecting a whole phrase and retyping it
/// means, and needs no rewriting to say so.
fn reaches(mark: &FoundMark, range: ByteSpan) -> bool {
    let span = ByteSpan::new(mark.start, mark.end);

    range.start < span.end && span.start < range.end && !range.contains(span)
}

/// The bytes between a mark's brackets.
fn words_of(mark: &FoundMark) -> ByteSpan {
    // One byte for `[` and one for the `]` that the group follows, both ASCII.
    ByteSpan::new(mark.start + 1, mark.attributes_start - 1)
}

/// Rebuilds a span of the body that a range crosses a mark inside.
#[derive(Debug)]
struct Rewrite<'a> {
    source: &'a str,
    range: ByteSpan,
    text: &'a str,
    out: String,
    /// Whether the typed text has been placed. It goes after the first piece
    /// that reaches the start of the range, and exactly once.
    written: bool,
}

impl Rewrite<'_> {
    /// Source outside any mark, with the part the range covers taken out.
    fn plain(&mut self, span: ByteSpan) {
        self.keep(ByteSpan::new(span.start, span.end.min(self.range.start)));

        if self.range.start <= span.end {
            self.typed();
        }

        self.keep(ByteSpan::new(span.start.max(self.range.end), span.end));
    }

    /// One mark, holding whatever of its words the range left.
    fn mark(&mut self, mark: &FoundMark) {
        let words = words_of(mark);
        let cut = ByteSpan::new(
            self.range.start.clamp(words.start, words.end),
            self.range.end.clamp(words.start, words.end),
        );
        let (prefix, suffix) =
            (ByteSpan::new(words.start, cut.start), ByteSpan::new(cut.end, words.end));

        if !prefix.is_empty() || !suffix.is_empty() {
            // The brackets and the group come from the source rather than from a
            // re-render, so a mark that only lost a letter does not also lose the
            // spacing the author wrote inside its braces.
            self.keep(ByteSpan::new(mark.start, words.start));
            self.keep(prefix);
            self.keep(suffix);
            self.keep(ByteSpan::new(words.end, mark.end));
        }

        if self.range.start <= mark.end {
            self.typed();
        }
    }

    fn keep(&mut self, span: ByteSpan) {
        if !span.is_empty() {
            self.out.push_str(span.slice(self.source));
        }
    }

    fn typed(&mut self) {
        if self.written {
            return;
        }

        self.out.push_str(self.text);
        self.written = true;
    }
}

/// The same change, over only the bytes that differ.
///
/// A paragraph retyped end to end would otherwise splice the paragraph, and the
/// splice is what an undo step and a merge are measured in: a wide one takes a
/// co-author's word back with it. Trimming is safe whatever it finds, because
/// the bytes it drops are identical on both sides by construction.
fn narrowed(source: &str, (span, text): (ByteSpan, String)) -> (ByteSpan, String) {
    let was = span.slice(source);

    let mut start = 0;
    while start < was.len().min(text.len()) && was.as_bytes()[start] == text.as_bytes()[start] {
        start += 1;
    }
    while start > 0 && !(was.is_char_boundary(start) && text.is_char_boundary(start)) {
        start -= 1;
    }

    let mut end = 0;
    while end < was.len().min(text.len()) - start
        && was.as_bytes()[was.len() - 1 - end] == text.as_bytes()[text.len() - 1 - end]
    {
        end += 1;
    }
    while end > 0
        && !(was.is_char_boundary(was.len() - end) && text.is_char_boundary(text.len() - end))
    {
        end -= 1;
    }

    (ByteSpan::new(span.start + start, span.end - end), text[start..text.len() - end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::EditOp;
    use crate::{apply, plan};
    use slidx_core::DeckParseOptions;

    fn typed(source: &str, start: usize, end: usize, text: &str) -> String {
        let op = EditOp::SetText {
            slide: 0.into(),
            range: ByteSpan::new(start, end),
            text: text.to_string(),
        };

        apply(source, &DeckParseOptions::default(), &op).expect("the fixture has what it names")
    }

    /// Where `needle` starts in `source`, so a test says what it means.
    fn at(source: &str, needle: &str) -> usize {
        source.find(needle).unwrap_or_else(|| panic!("{needle:?} is not in the fixture"))
    }

    #[test]
    fn a_word_retyped_in_a_paragraph_replaces_only_that_word() {
        let source = "The result was faster.\n";
        let start = at(source, "faster");

        assert_eq!(typed(source, start, start + 6, "quicker"), "The result was quicker.\n");
    }

    #[test]
    fn typing_inside_a_mark_leaves_its_key_where_it_was() {
        // The failure this prevents is silent: the slide still reads correctly
        // and the `steps:` entry that animated `#latency` now targets nothing.
        let source = "Latency dropped to [120ms]{#latency}.\n";
        let start = at(source, "120");

        assert_eq!(
            typed(source, start, start + 3, "130"),
            "Latency dropped to [130ms]{#latency}.\n"
        );
    }

    #[test]
    fn typing_inside_a_mark_splices_the_words_and_nothing_else() {
        // Not "produces the same file" — the same *splice*. A replacement over
        // the whole mark would be a merge that takes a co-author's class away.
        let source = "A [120ms]{#latency .accent} b.\n";
        let start = at(source, "120");
        let op = EditOp::SetText {
            slide: 0.into(),
            range: ByteSpan::new(start, start + 3),
            text: "9".into(),
        };
        let edit = plan(source, &DeckParseOptions::default(), &op).unwrap();

        assert_eq!(edit.splices().len(), 1);
        assert_eq!(edit.splices()[0].span, ByteSpan::new(start, start + 3));
    }

    #[test]
    fn a_mark_whose_words_are_all_deleted_goes_with_them() {
        // `[]{#latency}` is not something a person meant to write, which is the
        // rule a mark's attributes already keep.
        let source = "A [120ms]{#latency} b.\n";
        let start = at(source, "120ms");

        assert_eq!(typed(source, start, start + 5, ""), "A  b.\n");
    }

    #[test]
    fn a_selection_across_a_marks_edge_leaves_it_holding_the_words_that_survived() {
        // Explainable rather than arbitrary: the mark kept `120`, and what was
        // typed is not part of it.
        let source = "Latency dropped to [120ms]{#latency} today.\n";
        let start = at(source, "ms]");
        let end = at(source, "today");

        assert_eq!(typed(source, start, end, "X "), "Latency dropped to [120]{#latency}X today.\n");
    }

    #[test]
    fn a_selection_reaching_into_a_mark_from_outside_leaves_the_end_of_its_words() {
        let source = "Down to [120ms]{#latency}.\n";
        let start = at(source, "to [");

        assert_eq!(typed(source, start, start + 6, "at "), "Down at [0ms]{#latency}.\n");
    }

    #[test]
    fn a_selection_spanning_two_marks_leaves_the_start_of_one_and_the_end_of_the_other() {
        let source = "[alpha]{#a} and [omega]{#b}.\n";
        let start = at(source, "pha]");
        let end = at(source, "ega]");

        assert_eq!(typed(source, start, end, " to "), "[al]{#a} to [ega]{#b}.\n");
    }

    #[test]
    fn a_selection_that_covers_a_whole_mark_takes_it_with_the_words_it_marked() {
        // Selecting a phrase and retyping it is not a request to keep a class
        // on words that are no longer there.
        let source = "The [old]{.accent} way.\n";
        let start = at(source, "[old]");
        let end = at(source, " way");

        assert_eq!(typed(source, start, end, "new"), "The new way.\n");
    }

    #[test]
    fn a_mark_the_selection_swallowed_goes_while_the_two_at_its_edges_survive() {
        let source = "Say [alpha]{#a} then [beta]{#b} then [gamma]{#c}.\n";
        let start = at(source, "pha]");
        let end = at(source, "mma]");

        assert_eq!(typed(source, start, end, " to "), "Say [al]{#a} to [mma]{#c}.\n");
    }

    #[test]
    fn a_bracket_typed_inside_a_mark_is_escaped_rather_than_ending_it() {
        // An unbalanced bracket there would close the mark early and lose the
        // key, which is the failure this module exists to prevent.
        let source = "A [word]{#k} b.\n";
        let start = at(source, "word");

        assert_eq!(typed(source, start, start + 4, "a] b"), "A [a\\] b]{#k} b.\n");
    }

    #[test]
    fn markdown_typed_outside_a_mark_is_written_as_typed() {
        // An author typing two asterisks into a Markdown file means emphasis.
        // Escaping them would make in-place editing a worse text editor than
        // the textarea it replaces.
        let source = "A word b.\n";
        let start = at(source, "word");

        assert_eq!(typed(source, start, start + 4, "**word**"), "A **word** b.\n");
    }

    #[test]
    fn text_inserted_at_a_caret_replaces_nothing() {
        let source = "Ready.\n";
        let start = at(source, ".");

        assert_eq!(typed(source, start, start, " now"), "Ready now.\n");
    }

    #[test]
    fn a_paragraph_retyped_end_to_end_splices_only_what_changed() {
        // The editor sends the run it was given rather than a diff, so the
        // narrowing has to happen here — and it is what keeps one author's
        // sentence from overwriting another's in the same paragraph.
        let source = "One two three four.\n";
        let op = EditOp::SetText {
            slide: 0.into(),
            range: ByteSpan::new(0, 19),
            text: "One two THREE four.".into(),
        };
        let edit = plan(source, &DeckParseOptions::default(), &op).unwrap();

        assert_eq!(edit.splices().len(), 1);
        assert_eq!(edit.splices()[0].span.slice(source), "three");
        assert_eq!(edit.splices()[0].text, "THREE");
    }

    #[test]
    fn text_that_says_what_the_source_already_says_is_not_an_edit_at_all() {
        let source = "One two three.\n";
        let op = EditOp::SetText {
            slide: 0.into(),
            range: ByteSpan::new(0, 14),
            text: "One two three.".into(),
        };

        assert!(plan(source, &DeckParseOptions::default(), &op).unwrap().is_empty());
    }

    #[test]
    fn the_heading_marker_is_outside_the_range_so_it_survives_being_retitled() {
        // The editor maps the words a reader sees, and `#   ` is not among
        // them. Three spaces after the hash are the author's.
        let source = "#   One\n\nBody.\n";
        let start = at(source, "One");

        assert_eq!(typed(source, start, start + 3, "Two"), "#   Two\n\nBody.\n");
    }

    #[test]
    fn a_range_that_would_cut_a_character_in_half_is_refused_rather_than_applied() {
        // CJK and emoji are ordinary content, and a byte range from a stale copy
        // of the file lands mid-character sooner or later.
        let source = "速い\n";
        let op = EditOp::SetText { slide: 0.into(), range: ByteSpan::new(0, 2), text: "x".into() };

        assert_eq!(
            plan(source, &DeckParseOptions::default(), &op),
            Err(EditError::UnusableRange { range: ByteSpan::new(0, 2) })
        );
    }

    #[test]
    fn a_range_past_the_end_of_the_body_is_refused_rather_than_applied() {
        let source = "# One\n\n---\n\n# Two\n";
        let op = EditOp::SetText { slide: 0.into(), range: ByteSpan::new(0, 99), text: "x".into() };

        assert!(plan(source, &DeckParseOptions::default(), &op).is_err());
    }

    #[test]
    fn typing_on_one_slide_leaves_every_other_slide_alone() {
        // The range is measured in one slide's body, and two slides of a deck
        // are allowed to say the same words.
        let source = "# One\n\nSame.\n\n---\n\n# Two\n\nSame.\n";
        let second = "# Two\n\nSame.";
        let start = second.find("Same").unwrap();
        let op = EditOp::SetText {
            slide: 1.into(),
            range: ByteSpan::new(start, start + 4),
            text: "Other".into(),
        };

        assert_eq!(
            apply(source, &DeckParseOptions::default(), &op).unwrap(),
            "# One\n\nSame.\n\n---\n\n# Two\n\nOther.\n"
        );
    }

    #[test]
    fn a_text_edit_and_the_same_change_typed_by_hand_produce_the_same_bytes() {
        // The claim the editor exists to keep, as a property rather than as a
        // sentence: the canvas and the file are two views of one document.
        let source = "Latency dropped to [120ms]{#latency} today.\n";
        let start = at(source, "120");
        let by_hand = "Latency dropped to [38ms]{#latency} today.\n";

        assert_eq!(typed(source, start, start + 5, "38ms"), by_hand);
    }

    #[test]
    fn no_range_at_all_can_leave_a_mark_carrying_different_attributes() {
        // A range built from a copy of the file a keystroke old can name any
        // bytes, the group's included. Whatever it names, the mark keeps the
        // whole group or goes — never half of one, and never another one.
        let source = "A [word]{#k .accent} b.\n";
        let body = "A [word]{#k .accent} b.";

        for start in 0..=body.len() {
            for end in start..=body.len() {
                let op = EditOp::SetText {
                    slide: 0.into(),
                    range: ByteSpan::new(start, end),
                    text: "X".into(),
                };
                let result =
                    apply(source, &DeckParseOptions::default(), &op).expect("a usable range");

                for found in slidx_core::find_marks(&result) {
                    assert_eq!(found.mark.key.as_deref(), Some("k"), "{start}..{end}: {result:?}");
                    assert_eq!(found.mark.classes, ["accent"], "{start}..{end}: {result:?}");
                }
            }
        }
    }
}
