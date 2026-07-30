//! The thing somebody probably meant.
//!
//! `slidx lnit` is a typo, not a question about what slidx has. Answering it
//! with the whole command list makes a person do the search themselves; naming
//! `lint` ends it. The same holds for a flag: `--strcit` has exactly one
//! plausible reading.
//!
//! ## Why edit distance rather than the fuzzy scorer
//!
//! [`crate::find::scoring`] is a *subsequence* matcher and deliberately has no
//! typo tolerance — `lnit` is not a subsequence of `lint`, and a picker that
//! matched what you did not type is one you cannot narrow with. That is right
//! for a picker and useless here, because a typo is precisely a character in the
//! wrong place. Two different questions, two different answers.
//!
//! ## When it says nothing
//!
//! A guess has to be better than silence to be worth making. `slidx frobnicate`
//! is not a misspelling of anything, and suggesting `preview` for it would send
//! somebody to read a page that cannot help them. So the distance is bounded,
//! and bounded relative to what was typed: two edits is a typo in a nine-letter
//! word and a different word entirely in a three-letter one.

/// The closest candidate to `typed`, if one is close enough to be worth saying.
pub fn to<'a>(typed: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let typed = typed.to_lowercase();

    candidates
        .filter_map(|candidate| {
            let distance = between(&typed, &candidate.to_lowercase());

            worth_saying(&typed, candidate, distance).then_some((candidate, distance))
        })
        // The shortest candidate breaks a tie, because a shorter word reached in
        // the same number of edits is the closer relative of the two.
        .min_by_key(|(candidate, distance)| (*distance, candidate.len()))
        .map(|(candidate, _)| candidate)
}

/// Whether a guess at this distance helps more than it misleads.
fn worth_saying(typed: &str, candidate: &str, distance: usize) -> bool {
    // A prefix is somebody typing the start of a word — `slidx comp` — and no
    // distance threshold should refuse it. It is the strongest signal there is
    // and it is not a spelling mistake at all.
    if !typed.is_empty() && candidate.to_lowercase().starts_with(typed) {
        return true;
    }

    let length = typed.chars().count().max(candidate.chars().count());

    match distance {
        // One edit is unambiguous at any length. `tul` is `tui`.
        0 | 1 => true,
        // Two edits is a transposition — `lnit` for `lint`, the most common typo
        // there is — and has to be caught. In a three-letter word it is two
        // thirds of the word, and there is no way to tell which word was meant.
        2 => length >= 4,
        _ => false,
    }
}

/// Levenshtein distance: insertions, deletions and substitutions.
///
/// One row of the matrix rather than all of it, because nothing here needs the
/// path — only the number. Both words are command or flag names, so this runs
/// over a handful of characters and its cost never comes up.
///
/// A transposition costs two rather than one, which the threshold above allows
/// for: `lnit` for `lint` is the most common typo there is and has to be caught.
fn between(a: &str, b: &str) -> usize {
    let first: Vec<char> = a.chars().collect();
    let second: Vec<char> = b.chars().collect();

    if first.is_empty() {
        return second.len();
    }

    let mut previous: Vec<usize> = (0..=second.len()).collect();
    let mut current = vec![0; second.len() + 1];

    for (row, left) in first.iter().enumerate() {
        current[0] = row + 1;

        for (column, right) in second.iter().enumerate() {
            let substitute = previous[column] + usize::from(left != right);
            let insert = current[column] + 1;
            let delete = previous[column + 1] + 1;

            current[column + 1] = substitute.min(insert).min(delete);
        }

        std::mem::swap(&mut previous, &mut current);
    }

    previous[second.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command;

    fn nearest_command(typed: &str) -> Option<&'static str> {
        to(typed, command::names().into_iter())
    }

    #[test]
    fn a_transposition_is_the_typo_everybody_makes_and_is_caught() {
        assert_eq!(nearest_command("lnit"), Some("lint"));
        assert_eq!(nearest_command("doctro"), Some("doctor"));
    }

    #[test]
    fn a_missing_or_doubled_letter_is_caught() {
        assert_eq!(nearest_command("prevew"), Some("preview"));
        assert_eq!(nearest_command("publissh"), Some("publish"));
    }

    #[test]
    fn the_start_of_a_word_is_taken_as_that_word() {
        // `slidx comp` is not a misspelling and no distance threshold should
        // refuse it.
        assert_eq!(nearest_command("comp"), Some("completions"));
        assert_eq!(nearest_command("ver"), Some("version"));
    }

    #[test]
    fn something_that_is_not_a_misspelling_of_anything_gets_no_guess() {
        // Suggesting `preview` here would send somebody to read a page that
        // cannot help them, which is worse than the command list.
        assert_eq!(nearest_command("frobnicate"), None);
        assert_eq!(nearest_command("xyzzy"), None);
    }

    #[test]
    fn a_short_word_is_held_to_a_stricter_standard_than_a_long_one() {
        // Two edits in `publish` is a slip. Two edits in `tui` is two thirds of
        // the word, and there is no way to tell which word was meant.
        assert_eq!(to("xai", ["tui"].into_iter()), None);
        assert_eq!(to("cat", ["tui"].into_iter()), None);
        assert_eq!(to("publsih", ["publish"].into_iter()), Some("publish"));
    }

    #[test]
    fn one_edit_is_always_close_enough_however_short_the_word() {
        // `tul` for `tui` is unambiguous even though a third of it is wrong.
        assert_eq!(nearest_command("tul"), Some("tui"));
    }

    #[test]
    fn case_is_not_a_difference_because_nobody_meant_it_to_be() {
        assert_eq!(nearest_command("Lint"), Some("lint"));
        assert_eq!(nearest_command("DOCTOR"), Some("doctor"));
    }

    #[test]
    fn the_closest_of_several_candidates_wins() {
        assert_eq!(to("lst", ["list", "lint", "last"].into_iter()), Some("list"));
    }

    #[test]
    fn an_exact_match_is_its_own_nearest() {
        // Reached when a flag is offered to the command that does not take it:
        // the name is spelled correctly and belongs somewhere else.
        assert_eq!(nearest_command("lint"), Some("lint"));
    }

    #[test]
    fn nothing_typed_at_all_suggests_nothing() {
        assert_eq!(nearest_command(""), None);
    }

    #[test]
    fn a_flag_is_the_same_problem_and_gets_the_same_answer() {
        let lint = command::find("lint").expect("lint exists");
        let flags = lint.all_flags();

        assert_eq!(to("strcit", flags.iter().map(|flag| flag.long)), Some("strict"));
        assert_eq!(to("thmee", flags.iter().map(|flag| flag.long)), Some("theme"));
    }

    #[test]
    fn distance_counts_the_edits_it_says_it_does() {
        assert_eq!(between("lint", "lint"), 0);
        assert_eq!(between("lnit", "lint"), 2);
        assert_eq!(between("lin", "lint"), 1);
        assert_eq!(between("", "lint"), 4);
        assert_eq!(between("lint", ""), 4);
    }

    #[test]
    fn distance_counts_characters_rather_than_bytes() {
        // Nothing in the table is spelled in Japanese today. A distance measured
        // in bytes would report 3 for one wrong character, and the threshold
        // would then refuse every guess.
        assert_eq!(between("日本", "日語"), 1);
    }
}
