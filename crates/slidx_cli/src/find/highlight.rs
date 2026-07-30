//! Showing which characters a query matched.
//!
//! [`super::scoring`] already works out exactly where a needle landed in a
//! haystack, down to the byte, and gets it right for multi-byte text. Until this
//! module existed nothing read that: the picker showed which decks matched and
//! never which part of them did, which is the difference between a list you have
//! to re-read and a list you can scan.
//!
//! ## Two carriers, always both
//!
//! A matched run is bracketed **and** coloured. The brackets are not a fallback
//! for a monochrome terminal — they are there in every terminal, because:
//!
//! - colour is off in a pipe, under `NO_COLOR`, and on a `dumb` terminal, and a
//!   highlight that vanished there would be a highlight nobody could rely on;
//! - the layout has to be the same either way. Everything else slidx prints
//!   occupies the same columns coloured and plain, and a bracket that appeared
//!   only sometimes would move the column beside it only sometimes.
//!
//! ## Offsets are bytes and columns are cells
//!
//! The positions are byte offsets into the haystack, and the picker draws pieces
//! of that haystack — a shortened path, a title. So they are rebased onto the
//! piece being drawn before anything is marked, and the marking is done by
//! splitting on byte boundaries the scorer guaranteed. Width never enters into
//! it here, which is what keeps a Japanese title from shifting the rest of the
//! row: the brackets are ASCII and the text between them is untouched.

use std::ops::Range;

use crate::style::{Ink, Style};

/// The positions that fall inside `span`, measured from its start.
///
/// A haystack is a path, a title and an event joined together; the picker draws
/// them in different columns. A position outside the piece being drawn is
/// dropped rather than clamped — clamping would put a bracket around a character
/// that did not match.
pub fn rebased(positions: &[usize], span: Range<usize>) -> Vec<usize> {
    positions.iter().filter(|at| span.contains(at)).map(|at| at - span.start).collect()
}

/// Shifts positions to follow text whose front was replaced by something
/// shorter.
///
/// The picker draws `~/talks/x` for `/home/somebody/talks/x`. A match inside the
/// part that was replaced is not on screen at all, so it is dropped.
pub fn after_prefix(positions: &[usize], replaced: usize, with: usize) -> Vec<usize> {
    positions.iter().filter(|at| **at >= replaced).map(|at| at - replaced + with).collect()
}

/// `text` with the matched runs bracketed, and coloured where colour is on.
///
/// Runs rather than characters: `[vue]conf` reads, and `[v][u][e]conf` does not.
///
/// Takes **unpainted** text and the ink the rest of the row is wearing, and
/// paints each piece itself. Painting the row first and marking afterwards is
/// the obvious way round and is wrong twice: the positions would be offsets into
/// a string with escape bytes in front of them, and the highlight's own reset
/// would end the row's ink for everything after it.
pub fn marked(text: &str, positions: &[usize], base: Ink, style: &Style) -> String {
    if positions.is_empty() {
        return style.paint(base, text);
    }

    let mut out = String::with_capacity(text.len() + positions.len() * 2);
    let mut rest = 0;

    for run in runs(text, positions) {
        out.push_str(&style.paint(base, &text[rest..run.start]));
        out.push_str(&style.paint(Ink::Hit, format!("[{}]", &text[run.clone()])));
        rest = run.end;
    }

    out.push_str(&style.paint(base, &text[rest..]));
    out
}

/// The matched positions gathered into adjacent stretches of `text`.
///
/// Adjacency is by character, not by byte: two positions three bytes apart are
/// consecutive when the character between them is three bytes long, which is the
/// ordinary case for a Japanese title rather than an unusual one.
fn runs(text: &str, positions: &[usize]) -> Vec<Range<usize>> {
    let mut sorted: Vec<usize> = positions.iter().copied().filter(|at| *at < text.len()).collect();
    sorted.sort_unstable();
    sorted.dedup();

    let mut runs: Vec<Range<usize>> = Vec::new();

    for at in sorted {
        // A position the scorer produced is always on a character boundary. One
        // that is not came from somewhere else, and slicing on it would panic —
        // so it is skipped rather than trusted.
        if !text.is_char_boundary(at) {
            continue;
        }

        let end = at + text[at..].chars().next().map(char::len_utf8).unwrap_or(0);

        match runs.last_mut() {
            Some(last) if last.end == at => last.end = end,
            _ => runs.push(at..end),
        }
    }

    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::find::scoring;

    fn mark(needle: &str, haystack: &str) -> String {
        let found = scoring::score(needle, haystack).expect("a match");

        marked(haystack, &found.positions, Ink::Strong, &Style::plain())
    }

    #[test]
    fn what_matched_is_shown_and_the_rest_is_left_alone() {
        assert_eq!(mark("vue", "vueconf"), "[vue]conf");
    }

    #[test]
    fn adjacent_matches_are_one_run_rather_than_one_bracket_each() {
        // `[v][u][e]conf` is technically the same information and unreadable.
        assert_eq!(marked("vueconf", &[0, 1, 2, 3], Ink::Strong, &Style::plain()), "[vuec]onf");
    }

    #[test]
    fn matches_with_a_gap_between_them_get_a_bracket_each() {
        // And they mark where the scorer actually landed, not where somebody
        // reading the query would guess: `vcf` takes the `f` of `conf`, because
        // the scorer takes the first of each character it can.
        assert_eq!(mark("vcf", "vue-conf-fukuoka"), "[v]ue-[c]on[f]-fukuoka");
    }

    #[test]
    fn a_match_in_japanese_text_brackets_whole_characters() {
        // The scorer's positions are byte offsets. Marking by byte would split
        // `語` down the middle and the terminal would draw a replacement box
        // where the highlight should be.
        assert_eq!(mark("語", "日本語のトーク"), "日本[語]のトーク");
    }

    #[test]
    fn a_highlight_inside_japanese_text_does_not_change_the_text_around_it() {
        // The row beside it is padded from the rendered width, so the only thing
        // that may change is the two brackets — never the characters.
        let marked = mark("トク", "日本語のトーク");

        assert!(marked.contains("日本語の"), "{marked}");
        assert_eq!(marked.replace(['[', ']'], ""), "日本語のトーク");
    }

    #[test]
    fn nothing_is_marked_when_nothing_matched() {
        // An empty query lists the whole index, and every row of it would
        // otherwise be wearing empty brackets.
        assert_eq!(marked("vueconf", &[], Ink::Strong, &Style::plain()), "vueconf");
    }

    #[test]
    fn the_brackets_are_there_without_colour_because_a_pipe_has_no_colour() {
        // The rule: colour is never the only carrier. In a pipe, under
        // NO_COLOR, and on a dumb terminal there is nothing but the brackets.
        let plain = marked("vueconf", &[0, 1, 2], Ink::Strong, &Style::plain());

        assert_eq!(plain, "[vue]conf");
        assert!(!plain.contains('\u{1b}'));
    }

    #[test]
    fn colour_is_added_on_top_and_changes_no_column() {
        let plain = marked("vueconf", &[0, 1, 2], Ink::Strong, &Style::plain());
        let colored = marked("vueconf", &[0, 1, 2], Ink::Strong, &Style::colored());

        assert!(colored.contains('\u{1b}'));
        assert_eq!(crate::style::width::of(&colored), crate::style::width::of(&plain));
    }

    #[test]
    fn a_position_past_the_end_of_the_text_is_dropped_rather_than_panicking() {
        // Reachable if a caller rebases against the wrong span. A panic here is
        // a panic inside a raw-mode terminal, which is the worst place for one.
        assert_eq!(marked("abc", &[0, 99], Ink::Strong, &Style::plain()), "[a]bc");
    }

    #[test]
    fn a_position_that_is_not_a_character_boundary_is_skipped() {
        assert_eq!(marked("日本", &[1], Ink::Strong, &Style::plain()), "日本");
    }

    #[test]
    fn only_the_positions_inside_the_piece_being_drawn_are_kept() {
        // A haystack is a path, a title and an event joined up. The title column
        // must not bracket a character the path matched.
        assert_eq!(rebased(&[2, 7, 11], 5..10), [2]);
        assert_eq!(rebased(&[0, 1], 5..10), Vec::<usize>::new());
    }

    #[test]
    fn positions_follow_a_path_whose_home_directory_was_shortened() {
        // `/home/somebody/talks/x` is drawn as `~/talks/x`, so a match at 15
        // is at 2 once the prefix has been replaced by one character.
        assert_eq!(after_prefix(&[15, 16], 14, 1), [2, 3]);
    }

    #[test]
    fn a_match_inside_the_part_that_was_replaced_is_not_shown_at_all() {
        // It is not on screen. Clamping it to the start would bracket the tilde,
        // which claims a match that is not there.
        assert_eq!(after_prefix(&[3], 14, 1), Vec::<usize>::new());
    }
}
