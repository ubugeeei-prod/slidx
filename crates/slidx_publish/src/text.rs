//! Counting and cutting text the way a publishing platform does.
//!
//! Every cap in this crate is a count of *characters*, and a character is a
//! code point here — never a UTF-16 unit and never a byte. The distinction is
//! not academic: the platforms these payloads are built for validate in
//! JavaScript, where `String#length` counts UTF-16 units, so a deck titled with
//! one emoji costs two and is rejected for a limit it visibly did not reach.
//! Code points are the count a platform's own validator agrees with often
//! enough to be safe.
//!
//! The two slug functions look like duplicates and are not. A path segment on
//! someone else's platform has to survive their URL rules, which in practice
//! means ASCII; a file on the author's own disk does not, and a Japanese deck
//! deserves a Japanese filename rather than `slide-deck-2`.

/// Characters as a person counts them, not as UTF-16 stores them.
pub fn count_characters(text: &str) -> usize {
    text.chars().count()
}

/// The ellipsis is one code point, so it costs one character of the budget.
const ELLIPSIS: char = '…';

/// Clips `text` to at most `limit` characters, ellipsis included.
///
/// Cuts on a word boundary when there is one in the second half of the budget.
/// The restriction matters for scripts that do not space their words: a
/// Japanese sentence has no boundary to find, and honouring the first space in
/// a mostly-CJK string would throw away most of the budget to keep one Latin
/// word intact.
pub fn truncate(text: &str, limit: usize) -> String {
    if limit == 0 {
        return String::new();
    }

    let characters: Vec<char> = text.chars().collect();
    if characters.len() <= limit {
        return text.to_string();
    }
    if limit == 1 {
        return ELLIPSIS.to_string();
    }

    let budget = limit - 1;
    let candidate: String = characters[..budget].iter().collect();
    let broken = match last_break(&candidate) {
        // Halfway is a floor rather than a rounding: a break exactly at the
        // midpoint keeps too little of the sentence to be worth the words it
        // costs.
        Some(at) if at * 2 > budget => candidate.chars().take(at).collect::<String>(),
        _ => candidate,
    };

    format!("{}{ELLIPSIS}", broken.trim_end())
}

/// Where the last word boundary in `text` begins, in characters.
///
/// The boundary is the *start* of the whitespace run that precedes the final
/// word, so cutting there drops the run along with the half-word after it. A
/// run rather than a single space, because two spaces after a full stop are one
/// boundary and cutting between them would leave a stray space before the
/// ellipsis.
fn last_break(text: &str) -> Option<usize> {
    let characters: Vec<char> = text.chars().collect();
    let is_space = |at: &usize| characters[*at].is_whitespace();

    // Where the final word starts. Zero means the text has no whitespace at
    // all, which is the case the caller falls back to a length cut for.
    let word = (0..characters.len()).rev().find(is_space)? + 1;

    Some((0..word).rev().find(|at| !is_space(at)).map_or(0, |at| at + 1))
}

/// A slug for a URL on a platform that is not ours.
///
/// ASCII only. Returns an empty string when nothing survives — a title written
/// entirely in kana has no Latin slug, and inventing one from the slide index
/// would produce a URL that means nothing and changes silently when a slide
/// moves. Callers report the empty result and name `slug` as the fix.
pub fn ascii_slug(text: &str) -> String {
    slug_from(text, |character| character.is_ascii_alphanumeric())
}

/// A slug for a file on the author's own disk.
///
/// Keeps letters and digits from any script, case-folded by the Unicode rules
/// rather than the ASCII ones, matching `slug.rs` in `slidx_core` so a deck's
/// anchors and its blog draft are named alike.
pub fn file_slug(text: &str) -> String {
    slug_from(text, |character| character.is_alphanumeric())
}

fn slug_from(text: &str, keep: impl Fn(char) -> bool) -> String {
    let mut slug = String::new();

    for character in text.chars() {
        if keep(character) {
            slug.extend(character.to_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }

    trim_hyphens(&slug)
}

/// Shortens a slug we derived, on a hyphen boundary.
///
/// Only ever applied to a slug this crate invented. A slug the author wrote is
/// theirs, and a URL half of which we chose is worse than a reported cap.
pub fn fit_slug(slug: &str, limit: usize) -> String {
    if count_characters(slug) <= limit {
        return slug.to_string();
    }

    // Always on a hyphen when there is one: a slug is read as words, and half a
    // word in a URL looks like a bug rather than a shortening.
    let clipped: String = slug.chars().take(limit).collect();

    match clipped.rfind('-') {
        Some(at) if at > 0 => trim_hyphens(&clipped[..at]),
        _ => trim_hyphens(&clipped),
    }
}

fn trim_hyphens(slug: &str) -> String {
    slug.trim_matches('-').to_string()
}

/// A tag as a platform stores one: no `#`, no spaces, case-folded.
///
/// Case folding is what makes deduplication work. `Rust` and `rust` are one tag
/// everywhere they are actually stored, so treating them as two would publish a
/// list with a visible duplicate in it.
pub fn normalize_tag(tag: &str) -> String {
    let trimmed = tag.trim().trim_start_matches('#');
    let mut normalized = String::new();
    let mut in_space = false;

    for character in trimmed.chars() {
        if character.is_whitespace() {
            in_space = true;
            continue;
        }
        if in_space {
            normalized.push('-');
            in_space = false;
        }
        normalized.extend(character.to_lowercase());
    }

    normalized
}

/// Keeps the first spelling of each value, dropping empties.
pub fn unique_tags(tags: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();

    for tag in tags {
        let normalized = normalize_tag(tag);
        if normalized.is_empty() || kept.contains(&normalized) {
            continue;
        }
        kept.push(normalized);
    }

    kept
}

/// Collapses runs of blank lines so composed Markdown diffs cleanly.
pub fn tidy_block(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(normalized.len());
    let mut newlines = 0usize;

    for character in normalized.chars() {
        if character == '\n' {
            newlines += 1;
            // One blank line survives, whatever the author left. Two is a
            // paragraph break; three is a diff nobody meant to write.
            if newlines <= 2 {
                out.push('\n');
            }
            continue;
        }
        newlines = 0;
        out.push(character);
    }

    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_emoji_counts_as_the_one_character_a_person_sees() {
        // UTF-16 says two, which is the number that would reject a title that
        // fits.
        assert_eq!(count_characters("🎤"), 1);
        assert_eq!("🎤".encode_utf16().count(), 2);
    }

    #[test]
    fn a_cjk_character_counts_once_rather_than_by_its_bytes() {
        assert_eq!(count_characters("日本語"), 3);
        assert_eq!("日本語".len(), 9);
    }

    #[test]
    fn text_that_already_fits_is_left_exactly_as_it_was() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("exactly-10", 10), "exactly-10");
    }

    #[test]
    fn the_ellipsis_is_counted_against_the_budget_rather_than_added_to_it() {
        // An ellipsis that pushed the result one character over would defeat
        // the entire point of truncating.
        let cut = truncate("The quick brown fox jumps over the lazy dog", 20);

        assert!(count_characters(&cut) <= 20, "{cut}");
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn a_cut_lands_on_a_word_boundary_rather_than_mid_word() {
        assert_eq!(truncate("The quick brown fox", 12), "The quick…");
    }

    #[test]
    fn a_script_with_no_word_boundaries_is_cut_by_length() {
        // Japanese has no spaces. Insisting on a boundary here would return
        // almost nothing.
        assert_eq!(truncate("これは日本語の説明文です", 6), "これは日本…");
    }

    #[test]
    fn a_single_character_is_never_stranded_in_front_of_the_ellipsis() {
        assert_eq!(truncate("anything", 1), "…");
        assert_eq!(truncate("anything", 0), "");
    }

    #[test]
    fn a_boundary_in_the_first_half_of_the_budget_is_not_worth_taking() {
        // Keeping one short word and throwing away eight characters of the
        // sentence reads as a bug rather than as a shortening.
        assert_eq!(truncate("a bcdefghijklmnop", 10), "a bcdefgh…");
    }

    #[test]
    fn a_slug_for_someone_elses_url_is_lowercase_ascii_joined_by_hyphens() {
        assert_eq!(ascii_slug("Zero-JavaScript Slides"), "zero-javascript-slides");
        assert_eq!(ascii_slug("Rust: fast, and — safe?"), "rust-fast-and-safe");
        assert_eq!(ascii_slug("  ...Slides!  "), "slides");
    }

    #[test]
    fn a_title_with_no_latin_characters_yields_no_url_slug_at_all() {
        // Reported by the caller as a missing `slug`, rather than invented. An
        // address that means nothing is worse than one the author chooses.
        assert_eq!(ascii_slug("日本語のスライド"), "");
    }

    #[test]
    fn a_slug_for_the_authors_own_disk_keeps_japanese() {
        assert_eq!(file_slug("日本語のスライド"), "日本語のスライド");
        assert_eq!(file_slug("Zero-JavaScript Slides"), "zero-javascript-slides");
        assert_eq!(file_slug("Rust 2026 の話"), "rust-2026-の話");
    }

    #[test]
    fn a_derived_slug_is_cut_on_a_hyphen_so_no_word_is_left_in_half() {
        assert_eq!(fit_slug("zero-javascript-slides", 30), "zero-javascript-slides");
        assert_eq!(fit_slug("zero-javascript-slides", 20), "zero-javascript");
        assert_eq!(fit_slug("internationalization", 10), "internatio");
    }

    #[test]
    fn a_tag_loses_its_hash_folds_case_and_hyphenates_its_spaces() {
        assert_eq!(normalize_tag("#Slidx Conf"), "slidx-conf");
    }

    #[test]
    fn two_spellings_of_one_tag_are_published_once() {
        // They are one tag wherever they are actually stored, so publishing
        // both would show a visible duplicate.
        assert_eq!(unique_tags(&owned(["Rust", "rust"])), ["rust"]);
    }

    #[test]
    fn tags_keep_the_order_the_author_wrote_them_in() {
        assert_eq!(unique_tags(&owned(["slides", "rust", "wasm"])), ["slides", "rust", "wasm"]);
    }

    #[test]
    fn a_tag_that_normalises_to_nothing_is_dropped_rather_than_published_blank() {
        assert_eq!(unique_tags(&owned(["rust", "  ", "#"])), ["rust"]);
    }

    #[test]
    fn runs_of_blank_lines_collapse_so_composed_markdown_diffs_cleanly() {
        assert_eq!(tidy_block("one\n\n\n\ntwo"), "one\n\ntwo");
        assert_eq!(tidy_block("one\r\ntwo"), "one\ntwo");
        assert_eq!(tidy_block("\n\n  one  \n\n"), "one");
    }

    fn owned<const N: usize>(tags: [&str; N]) -> Vec<String> {
        tags.iter().map(|tag| (*tag).to_string()).collect()
    }
}
