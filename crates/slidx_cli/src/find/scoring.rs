//! Subsequence matching, scored the way a person expects.
//!
//! Typing `vcf` should find `vueconf-tokyo`, and typing `deck` should rank
//! `making-decks-fast` above `a-deck-somewhere-in-a-long-path`. Both fall out
//! of one idea: a match is a subsequence, and the score is how *tight* and how
//! *well-placed* that subsequence is.
//!
//! ## What earns points
//!
//! **Consecutive characters.** `deck` matching `deck` beats `deck` matching
//! `d-e-c-k` by a wide margin. This is the single strongest signal, because it
//! is what somebody typing a prefix of a word actually means.
//!
//! **Starting a word.** A character right after a separator — `-`, `_`, `/`,
//! `.`, a space — or at a camelCase boundary is worth more than one in the
//! middle. Typing `vcf` for `vue-conf-fukuoka` works because of this rule and
//! nothing else.
//!
//! **Being near the front.** A late match is usually a coincidence.
//!
//! ## What is deliberately not here
//!
//! No typo tolerance. `slidz` does not find `slidx`. A fuzzy finder that
//! matches things you did not type is one you cannot use to *exclude* things,
//! and narrowing is most of what people do with a picker.
//!
//! Matching is case-insensitive, but an exact-case hit scores higher, so
//! `VueConf` still beats `vueconf` when both are on screen.

/// Where a needle matched, and how well.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Higher is better. Only comparable between candidates for one needle.
    pub score: i32,
    /// Byte offsets in the haystack that matched, in order. For highlighting.
    pub positions: Vec<usize>,
}

const CONSECUTIVE: i32 = 15;
const WORD_START: i32 = 10;
const EXACT_CASE: i32 = 2;
/// Subtracted once per character skipped before the first match, capped so a
/// long path does not sink an otherwise perfect hit at the end of it.
const LEADING_PENALTY: i32 = 1;
const LEADING_CAP: i32 = 20;
/// Subtracted per character skipped inside the match. Gentler than the bonus
/// for staying together, so a tight match with one gap still beats a scattered
/// one with none.
const GAP_PENALTY: i32 = 2;

/// Scores `needle` against `haystack`, or `None` if it is not a subsequence.
///
/// An empty needle matches everything with a score of zero, which is what makes
/// an empty picker prompt list the whole index in its own order rather than an
/// arbitrary one.
pub fn score(needle: &str, haystack: &str) -> Option<Match> {
    if needle.is_empty() {
        return Some(Match { score: 0, positions: Vec::new() });
    }

    let candidate: Vec<char> = haystack.chars().collect();
    let mut offsets = Vec::with_capacity(candidate.len());
    let mut offset = 0;
    for character in &candidate {
        offsets.push(offset);
        offset += character.len_utf8();
    }

    let mut positions = Vec::new();
    let mut total = 0;
    let mut at = 0;
    let mut previous_index: Option<usize> = None;

    for wanted in needle.chars() {
        let found = candidate[at..]
            .iter()
            .position(|character| eq_ignoring_case(*character, wanted))
            .map(|relative| at + relative)?;

        total += placement(&candidate, found, previous_index);

        if candidate[found] == wanted {
            total += EXACT_CASE;
        }

        if previous_index.is_none() {
            total -= (found as i32 * LEADING_PENALTY).min(LEADING_CAP);
        }

        positions.push(offsets[found]);
        previous_index = Some(found);
        at = found + 1;
    }

    Some(Match { score: total, positions })
}

/// What one matched character is worth, given where it landed.
fn placement(candidate: &[char], found: usize, previous: Option<usize>) -> i32 {
    if previous == Some(found.wrapping_sub(1)) {
        return CONSECUTIVE;
    }

    let gap = previous.map(|last| (found - last - 1) as i32).unwrap_or(0);
    let base = if starts_a_word(candidate, found) { WORD_START } else { 0 };

    base - gap * GAP_PENALTY
}

/// True at the start of the string, after a separator, or at a camelCase edge.
fn starts_a_word(candidate: &[char], index: usize) -> bool {
    if index == 0 {
        return true;
    }

    let previous = candidate[index - 1];
    let here = candidate[index];

    if !previous.is_alphanumeric() {
        return true;
    }

    // `vueConf` — the C begins a word even though no separator does.
    previous.is_lowercase() && here.is_uppercase()
}

fn eq_ignoring_case(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

/// Ranks candidates, best first, dropping the ones that do not match at all.
///
/// A stable sort, so equally-scored candidates keep the order they came in —
/// which for the index is most-recently-seen, and is the right tiebreak: two
/// decks that match a query equally well are told apart by which one you
/// touched last.
pub fn rank<'a, T>(
    needle: &str,
    candidates: &'a [T],
    text: impl Fn(&T) -> String,
) -> Vec<(&'a T, Match)> {
    let mut hits: Vec<(&T, Match)> = candidates
        .iter()
        .filter_map(|candidate| score(needle, &text(candidate)).map(|found| (candidate, found)))
        .collect();

    hits.sort_by(|(_, a), (_, b)| b.score.cmp(&a.score));
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scored(needle: &str, haystack: &str) -> i32 {
        score(needle, haystack).unwrap_or_else(|| panic!("{needle} should match {haystack}")).score
    }

    fn beats(needle: &str, winner: &str, loser: &str) {
        assert!(
            scored(needle, winner) > scored(needle, loser),
            "{needle}: {winner} ({}) should beat {loser} ({})",
            scored(needle, winner),
            scored(needle, loser)
        );
    }

    #[test]
    fn a_needle_that_is_not_a_subsequence_does_not_match_at_all() {
        // No typo tolerance on purpose: a finder that matches what you did not
        // type is one you cannot narrow with.
        assert!(score("slidz", "slidx").is_none());
        assert!(score("xyz", "vueconf").is_none());
    }

    #[test]
    fn characters_have_to_appear_in_order() {
        assert!(score("cba", "abc").is_none());
        assert!(score("abc", "abc").is_some());
    }

    #[test]
    fn an_empty_needle_matches_everything_so_an_empty_prompt_lists_the_index() {
        let found = score("", "anything").expect("an empty needle matches");

        assert_eq!(found.score, 0);
        assert!(found.positions.is_empty());
    }

    #[test]
    fn initials_find_a_hyphenated_name() {
        // The reason the word-start bonus exists. `vcf` has to find this.
        assert!(score("vcf", "vue-conf-fukuoka").is_some());
        beats("vcf", "vue-conf-fukuoka", "vacufo");
    }

    #[test]
    fn initials_find_a_camel_case_name() {
        beats("vc", "vueConf", "voiceactor");
    }

    #[test]
    fn a_run_of_characters_beats_the_same_characters_scattered() {
        // The strongest signal there is: it is what somebody typing a prefix
        // of a word actually means.
        beats("deck", "deck", "d-e-c-k");
        beats("deck", "making-decks-fast", "dark-eccentric-knot");
    }

    #[test]
    fn a_match_at_the_front_beats_the_same_match_buried_in_a_path() {
        beats("talk", "talks/one", "/home/somebody/code/x/talk");
    }

    #[test]
    fn a_long_path_does_not_sink_an_otherwise_perfect_match() {
        // The leading penalty is capped for exactly this: everybody's projects
        // are twelve directories deep and that says nothing about relevance.
        let deep = "/home/somebody/code/github.com/someone/talks/vueconf";

        assert!(scored("vueconf", deep) > scored("vc", "/vc"), "a whole word should still win");
    }

    #[test]
    fn matching_ignores_case_but_the_exact_case_still_wins() {
        assert!(score("vueconf", "VueConf").is_some());
        beats("VueConf", "VueConf", "vueconf");
    }

    #[test]
    fn the_positions_point_at_what_matched_for_highlighting() {
        let found = score("dc", "deck").expect("a match");

        assert_eq!(found.positions, [0, 2]);
    }

    #[test]
    fn positions_are_byte_offsets_so_they_survive_multi_byte_text() {
        // A deck titled in Japanese is not unusual, and a highlight computed in
        // characters would slice a string mid-codepoint.
        let found = score("x", "日本語のx").expect("a match");

        assert_eq!(found.positions, [12]);
        assert!("日本語のx".is_char_boundary(found.positions[0]));
    }

    #[test]
    fn ranking_puts_the_best_match_first_and_drops_the_non_matches() {
        let candidates = ["vue-conf-fukuoka", "rustfest", "vueconf-tokyo"];
        let ranked = rank("vueconf", &candidates, |name| name.to_string());

        assert_eq!(ranked.len(), 2);
        assert_eq!(*ranked[0].0, "vueconf-tokyo");
    }

    #[test]
    fn equally_good_matches_keep_the_order_they_came_in() {
        // For the index that order is most-recently-seen, which is the right
        // tiebreak: two equally good matches are told apart by which one you
        // touched last.
        let candidates = ["talks/a", "talks/b"];
        let ranked = rank("talks", &candidates, |name| name.to_string());

        assert_eq!(*ranked[0].0, "talks/a");
        assert_eq!(*ranked[1].0, "talks/b");
    }

    #[test]
    fn ranking_an_empty_needle_keeps_every_candidate_in_its_own_order() {
        let candidates = ["c", "a", "b"];
        let ranked = rank("", &candidates, |name| name.to_string());

        assert_eq!(ranked.iter().map(|(name, _)| **name).collect::<Vec<_>>(), ["c", "a", "b"]);
    }
}
