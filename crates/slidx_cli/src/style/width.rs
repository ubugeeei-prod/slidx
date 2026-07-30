//! How many columns a string takes up on a terminal.
//!
//! Every aligned column slidx prints — the report's status column, the help
//! text's flag column, the picker's path column, the box the TUI draws — is
//! padded to a width. Counting characters to get that width is wrong twice
//! over, and both are wrong in this repository specifically:
//!
//! **An escape sequence is several characters and no columns.** A coloured
//! report measured in characters wraps early and shears one row at a time.
//!
//! **A Japanese character is one character and two columns.** This
//! maintainer's decks are in Japanese, so `日本語のトーク` is seven characters
//! and fourteen cells — and a table that assumed otherwise is broken on the
//! first deck it is pointed at, not on some edge case.
//!
//! ## Why a table here rather than a crate
//!
//! `unicode-width` is small and correct and would be the obvious answer. It is
//! not here for the same reason the argument parser and the SHA-256 are not:
//! this binary is what `curl | sh` hands somebody, and the ranges that decide
//! the answer for real text are a table you can read in one screen.
//!
//! ## What it does not do, and why that is safe here
//!
//! A combining mark is counted as a column when it occupies none, and an emoji
//! joined by a zero-width joiner is counted once per emoji in it. Getting those
//! right needs the Unicode character database rather than a range table.
//!
//! Both are safe to be wrong about *in this direction*: over-counting pads a
//! column too little and leaves a line short, never past the edge. Nothing here
//! truncates by width, so a wrong answer is a ragged column and never a string
//! cut through the middle of a character.

/// Ranges that take two cells, as East Asian Wide or Fullwidth.
///
/// Kept as data because that is what it is. The list is the blocks real text
/// arrives in — the CJK ideographs, the kana, Hangul, the fullwidth forms, and
/// the emoji blocks a slide title reaches for — rather than every range the
/// standard defines.
const WIDE: &[(char, char)] = &[
    // Hangul Jamo, and the syllables themselves.
    ('\u{1100}', '\u{115f}'),
    ('\u{ac00}', '\u{d7a3}'),
    // CJK radicals, Kangxi, the ideographic space, punctuation, kana, bopomofo,
    // and the compatibility blocks after them. U+303F is a half-fill and is
    // narrow, which is why this stops at U+303E.
    ('\u{2e80}', '\u{303e}'),
    ('\u{3041}', '\u{33ff}'),
    ('\u{3400}', '\u{4dbf}'),
    ('\u{4e00}', '\u{9fff}'),
    // Yi, and Yi radicals.
    ('\u{a000}', '\u{a4cf}'),
    // Compatibility ideographs, and the vertical and fullwidth forms.
    ('\u{f900}', '\u{faff}'),
    ('\u{fe10}', '\u{fe19}'),
    ('\u{fe30}', '\u{fe6f}'),
    ('\u{ff00}', '\u{ff60}'),
    ('\u{ffe0}', '\u{ffe6}'),
    // Emoji, in the blocks a deck title actually uses.
    ('\u{1f300}', '\u{1f64f}'),
    ('\u{1f680}', '\u{1f6ff}'),
    ('\u{1f900}', '\u{1f9ff}'),
    // The supplementary ideographic planes.
    ('\u{20000}', '\u{3fffd}'),
];

/// Whether this character takes two cells.
pub fn is_wide(character: char) -> bool {
    WIDE.iter().any(|(first, last)| character >= *first && character <= *last)
}

/// How many columns `text` occupies, ignoring any escape sequences in it.
///
/// Takes painted or unpainted text, which is the point: a caller that had to
/// know which it was holding would eventually get it wrong, and the failure is
/// a report that lines up in one terminal and not the other.
pub fn of(text: &str) -> usize {
    let mut columns = 0;
    let mut in_escape = false;

    for character in text.chars() {
        match character {
            '\u{1b}' => in_escape = true,
            // An SGR sequence ends at `m`. Nothing slidx emits is any other
            // kind, and a sequence left unterminated would swallow the rest of
            // the line either way.
            'm' if in_escape => in_escape = false,
            _ if in_escape => {}
            _ if is_wide(character) => columns += 2,
            _ => columns += 1,
        }
    }

    columns
}

/// The longest prefix of `text` that fits in `columns`, and never a broken
/// character.
///
/// Used where something has to be cut rather than wrapped — a title beside a
/// position counter. Cutting by byte or by character would either panic on a
/// multi-byte boundary or overrun the column by one cell per wide character.
pub fn clip(text: &str, columns: usize) -> &str {
    let mut used = 0;

    for (at, character) in text.char_indices() {
        let next = used + if is_wide(character) { 2 } else { 1 };

        if next > columns {
            return &text[..at];
        }

        used = next;
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_one_column_per_character() {
        assert_eq!(of("slidx doctor"), 12);
        assert_eq!(of(""), 0);
    }

    #[test]
    fn a_japanese_title_is_two_columns_per_character() {
        // The reason this module exists. Seven characters, fourteen cells.
        assert_eq!(of("日本語のトーク"), 14);
    }

    #[test]
    fn a_mixed_title_adds_up_to_what_the_terminal_will_draw() {
        // What a slide title actually looks like.
        assert_eq!(of("Vue で作る"), 3 + 1 + 2 + 2 + 2);
    }

    #[test]
    fn full_width_punctuation_is_wide_and_its_ascii_twin_is_not() {
        // `（` and `(` look alike in a proportional font and are one cell apart
        // in a terminal, which is exactly how a column drifts by one.
        assert_eq!(of("（）"), 4);
        assert_eq!(of("()"), 2);
    }

    #[test]
    fn a_half_width_katakana_stays_one_column_because_that_is_the_point_of_it() {
        assert_eq!(of("ｱｲｳ"), 3);
    }

    #[test]
    fn an_escape_sequence_occupies_no_columns_at_all() {
        // A coloured report measured in characters wraps early and shears one
        // row at a time.
        assert_eq!(of("\u{1b}[1;31mfail\u{1b}[0m"), 4);
    }

    #[test]
    fn a_painted_japanese_string_measures_the_same_as_an_unpainted_one() {
        let plain = "日本語";
        let painted = format!("\u{1b}[1m{plain}\u{1b}[0m");

        assert_eq!(of(&painted), of(plain));
    }

    #[test]
    fn clipping_never_splits_a_character() {
        // An odd column count against a two-cell character is the case that
        // panics if the cut is made by byte.
        assert_eq!(clip("日本語", 4), "日本");
        assert_eq!(clip("日本語", 3), "日");
        assert_eq!(clip("日本語", 0), "");
    }

    #[test]
    fn clipping_something_that_already_fits_returns_all_of_it() {
        assert_eq!(clip("slidx", 20), "slidx");
        assert_eq!(clip("slidx", 5), "slidx");
    }

    #[test]
    fn a_clipped_string_never_measures_wider_than_the_column_it_was_given() {
        for columns in 0..12 {
            assert!(of(clip("日本語のトーク", columns)) <= columns);
        }
    }
}
