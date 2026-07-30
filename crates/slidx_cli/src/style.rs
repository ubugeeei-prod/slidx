//! Colour, and when to leave it out.
//!
//! The doctor report is read on a laptop propped on a lectern, under stage
//! lighting, by somebody who is nervous. Colour is the fastest way to say
//! "this line, not the other six" — so it is used, and used sparingly enough
//! that the red means something.
//!
//! It is also the first thing to go wrong. Escape codes in a log file, in a CI
//! transcript, or piped into `grep` are noise at best and unreadable at worst,
//! so the decision is made once, up front, and every writer downstream takes a
//! [`Style`] rather than deciding for itself.
//!
//! ## The rule
//!
//! Colour is on only when **all** of these hold: the stream is a terminal,
//! `NO_COLOR` is unset or empty, and `TERM` is not `dumb`. `NO_COLOR` wins over
//! everything, including a `--color` flag, if one is ever added — the
//! convention is that setting it is the last word.
//!
//! Nothing in the layout depends on colour. Every status is spelled out as a
//! word as well, so the plain-text report says exactly what the coloured one
//! does. A report that only makes sense in colour is a report that stops making
//! sense the moment somebody pastes it into a chat window to ask for help.
//!
//! ## Columns are cells, not characters
//!
//! Everything that pads or wraps here measures with [`width::of`]. A column
//! measured in characters is a column that shears on the first Japanese title,
//! and those are the ordinary case here rather than the awkward one.

pub mod width;

use std::env;
use std::fmt::Display;
use std::io::IsTerminal;

/// How wide a line may get before it is wrapped.
///
/// Fixed rather than read from the terminal, and the fixed number is the reason
/// the reports are readable: a finding wrapped to a 200-column window is one
/// long line the eye cannot track back from, and a report whose shape changes
/// with the window is one nobody learns to scan. 80 is also what a paste into
/// an issue, a chat window, or a slide will survive.
pub const WIDTH: usize = 80;

/// What a piece of text is doing, rather than what colour it is.
///
/// Named for meaning so the palette can be changed in one place. `Ink::Red`
/// would put the decision at every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    /// Fix this before you start.
    Fail,
    /// Worth thirty seconds.
    Warn,
    /// Nobody could measure it.
    Unknown,
    /// Nothing to do.
    Pass,
    /// A heading, or a count worth finding again.
    Strong,
    /// Present, but not what anyone is looking for.
    Faint,
    /// The part of a line that answers what was typed — the characters a fuzzy
    /// query matched. Distinct from [`Ink::Strong`] because the row it appears
    /// on is often already strong, and a highlight that matched its background
    /// would highlight nothing.
    Hit,
}

impl Ink {
    /// SGR parameters, bold-first so the colour survives a washed-out screen.
    ///
    /// The original eight colours and the bold and faint attributes, and
    /// nothing else. That is not a limitation being tolerated — it is what
    /// makes one palette correct on a 24-bit terminal, on an 8-colour console,
    /// over ssh into a machine with a minimal `TERM`, and inside tmux. A
    /// 24-bit sequence would look better on the author's laptop and print as
    /// literal text on the venue's, and there is no reading of "better" that
    /// covers that trade.
    ///
    /// It also means the colours are the *user's* colours: every terminal
    /// theme maps these eight, so slidx's red is the red they chose.
    fn code(self) -> &'static str {
        match self {
            Self::Fail => "1;31",
            Self::Warn => "1;33",
            Self::Unknown => "1;35",
            Self::Pass => "32",
            Self::Strong => "1",
            Self::Faint => "2",
            Self::Hit => "1;36",
        }
    }
}

/// Whether this run may use colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    color: bool,
}

impl Style {
    /// No escape codes at all. What a pipe, a log, and every test gets.
    pub const fn plain() -> Self {
        Self { color: false }
    }

    pub const fn colored() -> Self {
        Self { color: true }
    }

    /// Reads the environment and the stream.
    ///
    /// Called once, in `main`. Everything downstream is handed the answer,
    /// because a second call could disagree with the first — stdout and stderr
    /// are not the same stream, and a report split between the two would be
    /// half coloured.
    pub fn detect() -> Self {
        Self {
            color: wants_color(
                env::var("NO_COLOR").ok().as_deref(),
                env::var("TERM").ok().as_deref(),
                std::io::stdout().is_terminal(),
            ),
        }
    }

    pub fn is_colored(self) -> bool {
        self.color
    }

    /// Wraps `text`, or returns it untouched when colour is off.
    pub fn paint(self, ink: Ink, text: impl Display) -> String {
        if !self.color {
            return text.to_string();
        }

        format!("\u{1b}[{}m{text}\u{1b}[0m", ink.code())
    }

    /// Pads to `columns` **after** painting, counting only what is drawn.
    ///
    /// The escape codes are zero-width on screen and several bytes in a string,
    /// so `format!("{:<8}", painted)` lines a column up in a plain terminal and
    /// ragged in a coloured one. Padding here is the only way the two runs
    /// produce the same layout.
    ///
    /// Cells rather than characters, so a Japanese subject in the column does
    /// not push everything after it half a column left.
    pub fn pad(self, ink: Ink, text: &str, columns: usize) -> String {
        let padding = columns.saturating_sub(width::of(text));
        format!("{}{}", self.paint(ink, text), " ".repeat(padding))
    }
}

/// Wraps `text` into lines of at most `columns` cells.
///
/// A word longer than the column is left whole and allowed to overhang. The
/// only things that get that long are paths, URLs and font names — exactly the
/// strings somebody is about to copy — and a path broken across two lines
/// cannot be copied at all.
///
/// Measured in cells, so a Japanese sentence wraps where the terminal will draw
/// the edge rather than at twice that.
pub fn wrap(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();

    for word in text.split_whitespace() {
        let would_be =
            if line.is_empty() { width::of(word) } else { width::of(&line) + 1 + width::of(word) };

        if !line.is_empty() && would_be > columns {
            lines.push(std::mem::take(&mut line));
        }

        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }

    if !line.is_empty() {
        lines.push(line);
    }

    lines
}

/// The colour decision, as a function of the three inputs it depends on.
///
/// Separated from [`Style::detect`] so every branch is reachable from a test
/// without setting process-wide environment variables, which two tests running
/// in parallel would fight over.
pub fn wants_color(no_color: Option<&str>, term: Option<&str>, is_terminal: bool) -> bool {
    // https://no-color.org: any non-empty value disables colour, whatever it
    // says. Checked first so it overrides everything else.
    if no_color.is_some_and(|value| !value.is_empty()) {
        return false;
    }

    // `dumb` is the terminal that advertises it cannot do this. Emacs' shell
    // and a handful of CI runners set it.
    if term == Some("dumb") {
        return false;
    }

    is_terminal
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESCAPE: char = '\u{1b}';

    #[test]
    fn a_plain_style_emits_the_text_and_nothing_around_it() {
        let painted = Style::plain().paint(Ink::Fail, "12% and not charging");

        assert_eq!(painted, "12% and not charging");
        assert!(!painted.contains(ESCAPE));
    }

    #[test]
    fn a_coloured_style_wraps_the_text_and_closes_the_sequence() {
        // An unclosed sequence bleeds into whatever the shell prints next,
        // which is how a tool leaves somebody's prompt red.
        let painted = Style::colored().paint(Ink::Fail, "flat");

        assert!(painted.starts_with("\u{1b}[1;31m"), "{painted:?}");
        assert!(painted.ends_with("\u{1b}[0m"), "{painted:?}");
        assert!(painted.contains("flat"));
    }

    #[test]
    fn every_ink_has_its_own_sequence_so_two_statuses_never_look_alike() {
        let inks =
            [Ink::Fail, Ink::Warn, Ink::Unknown, Ink::Pass, Ink::Strong, Ink::Faint, Ink::Hit];
        let mut codes: Vec<&str> = inks.iter().map(|ink| ink.code()).collect();
        let total = codes.len();
        codes.sort_unstable();
        codes.dedup();

        assert_eq!(codes.len(), total, "two inks render identically");
    }

    #[test]
    fn every_colour_is_one_an_eight_colour_terminal_has() {
        // The same palette has to be right on a 24-bit terminal and on the
        // console a venue laptop boots into. A 256-colour or 24-bit sequence —
        // `38;5;n` or `38;2;r;g;b` — is drawn as literal text where it is not
        // understood, which puts `[38;2;255;0;0m` in front of the one line
        // that says what to do.
        for ink in
            [Ink::Fail, Ink::Warn, Ink::Unknown, Ink::Pass, Ink::Strong, Ink::Faint, Ink::Hit]
        {
            let code = ink.code();

            assert!(!code.contains("38;"), "{code} is an extended colour");
            assert!(!code.contains("48;"), "{code} is an extended colour");

            for parameter in code.split(';').filter_map(|part| part.parse::<u8>().ok()) {
                // 0-9 are the attributes, 30-37 the foreground colours, 40-47
                // the background ones. Everything else needs a terminal that
                // may not be there.
                assert!(
                    parameter < 10 || (30..=47).contains(&parameter),
                    "{code} uses SGR {parameter}, which the original eight do not have"
                );
            }
        }
    }

    #[test]
    fn no_color_turns_colour_off_on_a_real_terminal() {
        // The whole point of the convention: it wins even where colour would
        // otherwise be correct.
        assert!(!wants_color(Some("1"), Some("xterm-256color"), true));
        assert!(!wants_color(Some("anything at all"), Some("xterm-256color"), true));
    }

    #[test]
    fn an_empty_no_color_is_not_a_request_for_plain_text() {
        // Exported-but-empty is how a variable looks when a shell script unset
        // it badly. Treating that as "no colour" would surprise people.
        assert!(wants_color(Some(""), Some("xterm-256color"), true));
    }

    #[test]
    fn a_pipe_gets_plain_text_even_with_a_capable_terminal_behind_it() {
        // `slidx doctor > report.txt` and `slidx lint | grep` are both this.
        assert!(!wants_color(None, Some("xterm-256color"), false));
    }

    #[test]
    fn a_terminal_that_says_it_is_dumb_is_believed() {
        assert!(!wants_color(None, Some("dumb"), true));
    }

    #[test]
    fn a_terminal_with_no_term_variable_still_gets_colour() {
        // Windows consoles set no TERM and handle sequences fine. Withholding
        // colour there would punish the platform for a Unix convention.
        assert!(wants_color(None, None, true));
    }

    #[test]
    fn a_padded_column_is_the_same_width_coloured_and_plain() {
        // The bug this exists to prevent: escape codes counted as characters,
        // so a coloured report is ragged where a plain one lines up.
        let plain = Style::plain().pad(Ink::Fail, "fail", 8);
        let colored = Style::colored().pad(Ink::Fail, "fail", 8);

        assert_eq!(plain, "fail    ");
        assert_eq!(width::of(&colored), width::of(&plain));
    }

    #[test]
    fn a_japanese_subject_is_padded_to_the_cells_it_will_occupy() {
        // Two characters, four cells. Padded by character count the next column
        // would start two cells early, and every row with a Japanese subject in
        // it would be out of line with every row without one.
        assert_eq!(Style::plain().pad(Ink::Strong, "日本", 8), "日本    ");
        assert_eq!(width::of(&Style::plain().pad(Ink::Strong, "日本", 8)), 8);
        assert_eq!(
            width::of(&Style::plain().pad(Ink::Strong, "ab", 8)),
            width::of(&Style::plain().pad(Ink::Strong, "日本", 8))
        );
    }

    #[test]
    fn wrapping_a_japanese_sentence_breaks_where_the_terminal_draws_the_edge() {
        // Measured in characters it would wrap at twice the column, which puts
        // half of every line past the right-hand side of the window.
        let lines = wrap("日本語の トークを する", 10);

        assert!(lines.iter().all(|line| width::of(line) <= 10), "{lines:?}");
    }

    #[test]
    fn text_longer_than_its_column_is_never_truncated() {
        // A remedy cut off at a column boundary is a remedy nobody can follow.
        assert_eq!(Style::plain().pad(Ink::Pass, "unknown", 4), "unknown");
    }

    #[test]
    fn wrapping_breaks_on_spaces_and_never_mid_word() {
        let lines = wrap("the machine is on Asia/Tokyo and the deck does not say", 20);

        assert!(lines.iter().all(|line| line.chars().count() <= 20), "{lines:?}");
        assert_eq!(lines.join(" "), "the machine is on Asia/Tokyo and the deck does not say");
    }

    #[test]
    fn a_word_longer_than_the_column_is_left_whole_and_overhangs() {
        // Paths, URLs and font names are what get this long, and they are
        // exactly the strings somebody is about to copy. One broken across two
        // lines cannot be copied at all.
        let lines = wrap("free on /System/Volumes/Data/Users/somebody/talks", 12);

        assert!(
            lines.contains(&"/System/Volumes/Data/Users/somebody/talks".to_string()),
            "{lines:?}"
        );
    }

    #[test]
    fn wrapping_nothing_produces_no_lines_rather_than_one_empty_one() {
        // A finding with no help text must not print a blank indented line
        // under it, which reads as something missing.
        assert!(wrap("", 40).is_empty());
        assert!(wrap("   ", 40).is_empty());
    }

    #[test]
    fn a_text_that_already_fits_comes_back_as_one_line() {
        assert_eq!(wrap("on mains power", 40), ["on mains power"]);
    }
}
