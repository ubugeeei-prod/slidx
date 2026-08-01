//! What language a deck is written in.
//!
//! `lang:` in frontmatter is the answer whenever an author gives one, and this
//! module exists entirely for when they do not. The alternative there is not
//! "unknown" — it is `en`, because `<html>` needs a tag and something has to be
//! written into it. A deck in Japanese served as `lang="en"` is a specific,
//! confident, wrong answer, and three things downstream believe it:
//!
//! - a screen reader picks an English voice and reads the kana as noise;
//! - the browser applies Latin line-breaking, so 禁則処理 does not happen;
//! - `slidx_theme`'s CJK setting — leading, tracking, 約物 — never matches.
//!
//! So the default is worth improving even though a guess is normally worse than
//! silence. The rule below is written to be defensible rather than clever, and
//! to hand back `None` the moment it stops being.
//!
//! # What each answer rests on
//!
//! **Kana proves Japanese.** Hiragana and katakana are used in no other
//! language. One is not enough to outvote a deck of English, but a deck whose
//! letters are mostly CJK and which contains kana is Japanese.
//!
//! **Hangul proves Korean**, for the same reason.
//!
//! **Han with neither is Chinese.** This is the one inference rather than an
//! observation, and it is sound in the direction that matters: Japanese prose
//! cannot avoid kana, because particles and okurigana are kana. A Japanese deck
//! long enough to measure will always contain some.
//!
//! # What it refuses to do
//!
//! It never overrides a declared `lang:`, it never distinguishes `zh-Hans` from
//! `zh-Hant` — script is not recoverable from a character set that both share —
//! and it never returns a Latin tag. Telling English from German is a different
//! problem needing a different kind of evidence, and `en` is already the
//! default, so a wrong guess there would replace a wrong answer with another
//! wrong answer for no gain.

/// The language a deck is written in, when the characters are enough to say.
///
/// `None` means the default stands. Deliberately not "unknown": every caller
/// already has an answer for that case and this is only ever an improvement on
/// it.
pub fn detect(text: &str) -> Option<&'static str> {
    let mut kana = false;
    let mut hangul = false;
    let mut cjk = 0usize;
    let mut letters = 0usize;

    for character in text.chars() {
        if !character.is_alphabetic() {
            continue;
        }

        letters += 1;

        if is_kana(character) {
            kana = true;
            cjk += 1;
        } else if is_hangul(character) {
            hangul = true;
            cjk += 1;
        } else if is_han(character) {
            cjk += 1;
        }
    }

    // A majority of the letters, not a presence test. An English talk quoting
    // one Japanese phrase is an English talk, and the cost of getting this
    // backwards is a deck read aloud in the wrong voice.
    if letters == 0 || cjk * 2 <= letters {
        return None;
    }

    match (kana, hangul) {
        (true, _) => Some("ja"),
        (false, true) => Some("ko"),
        (false, false) => Some("zh"),
    }
}

/// Hiragana and katakana, including the half-width forms.
///
/// The iteration marks and the prolonged sound mark are inside these blocks and
/// are as Japanese as the kana themselves.
fn is_kana(character: char) -> bool {
    matches!(character as u32, 0x3040..=0x30FF | 0xFF66..=0xFF9F)
}

fn is_hangul(character: char) -> bool {
    matches!(character as u32, 0x1100..=0x11FF | 0xAC00..=0xD7AF)
}

fn is_han(character: char) -> bool {
    matches!(character as u32,
        0x3400..=0x4DBF   // extension A
        | 0x4E00..=0x9FFF // unified
        | 0xF900..=0xFAFF // compatibility
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_prose_is_japanese() {
        assert_eq!(detect("スライドツールにおける日本語組版の現在地"), Some("ja"));
        assert_eq!(detect("行末の禁則処理が甘く、句読点が行頭に来てしまう。"), Some("ja"));
    }

    #[test]
    fn korean_prose_is_korean() {
        assert_eq!(detect("한국어로 작성된 발표 자료"), Some("ko"));
    }

    #[test]
    fn chinese_prose_is_chinese() {
        // The one inference on the list: Han with no kana and no hangul. It
        // holds because Japanese prose cannot be written without kana.
        assert_eq!(detect("这是一份中文的演示文稿"), Some("zh"));
    }

    #[test]
    fn a_deck_in_english_is_left_alone() {
        assert_eq!(detect("Making decks fast"), None);
        assert_eq!(detect(""), None);
        assert_eq!(detect("1234 !@#$ ---"), None);
    }

    #[test]
    fn one_quoted_phrase_does_not_change_the_language_of_a_talk() {
        // The failure that makes a presence test unusable. This deck is in
        // English and reading it aloud in Japanese would be worse than the
        // default it replaced.
        let english = "The Japanese word for a slide deck is スライド, which is \
                       a loanword. Everything else about this talk is in English \
                       and it should be read in English by anything that reads it.";

        assert_eq!(detect(english), None);
    }

    #[test]
    fn a_japanese_deck_quoting_english_is_still_japanese() {
        // The mirror case, and the common one: a Japanese talk about software
        // is full of Latin identifiers.
        let japanese = "このトークではRustとWebAssemblyを使い、\
                        Markdownからスライドを生成する仕組みについて話します。";

        assert_eq!(detect(japanese), Some("ja"));
    }

    #[test]
    fn kana_decides_between_japanese_and_chinese() {
        // Han alone reads as Chinese; the same text with one particle in kana
        // is Japanese. That single character is the whole distinction and the
        // test says so out loud.
        assert_eq!(detect("日本語組版"), Some("zh"));
        assert_eq!(detect("日本語の組版"), Some("ja"));
    }

    #[test]
    fn digits_and_punctuation_are_not_evidence_either_way() {
        // Only letters are counted, so a slide that is mostly numbers does not
        // dilute the letters that are there.
        assert_eq!(detect("2026年 第4四半期の結果"), Some("ja"));
    }

    #[test]
    fn the_boundary_is_a_majority_and_a_tie_is_not_one() {
        // Four CJK letters against four Latin ones is not evidence. Stated as a
        // test because `cjk * 2 <= letters` is easy to read as `<`.
        assert_eq!(detect("日本語の組版です abcdefgh"), None, "eight and eight is a tie");
        assert_eq!(detect("日本語の組版ですよ abcdefgh"), Some("ja"), "nine against eight is not");
    }
}
