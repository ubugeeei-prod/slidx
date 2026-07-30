//! The shape a slidx report takes on a terminal.
//!
//! `doctor` and `lint` answer different questions and print the same way, and
//! that is deliberate. Somebody who has learned to read one has learned to read
//! the other: the status is always the first word, the thing it is about is
//! always next, what was observed is underneath it, and the line that says what
//! to do is always the one marked `->`.
//!
//! Keeping that in one module rather than one per command is what stops the two
//! drifting into two dialects a year from now.
//!
//! ## Two forms
//!
//! A [`block`] is a paragraph: status, subject, observation, next action. It is
//! for anything somebody has to act on.
//!
//! A [`line`] is one row of a column: status, subject, observation. It is for
//! everything that passed, where the only job is to say it was looked at.
//!
//! The difference in weight is the report's most useful signal, and it works
//! before a word has been read.

use crate::style::{self, Ink, Style};

/// Marks the line somebody acts on.
///
/// ASCII rather than an arrow glyph. This prints on whatever console the venue
/// laptop turns out to have, and a mojibake box in front of the one line that
/// says what to do is worse than a plain hyphen.
pub const REMEDY: &str = "->";

/// The status column, sized to the longest token any report prints.
pub const STATUS_WIDTH: usize = 7;

/// Where the text of a report starts: two spaces, the status column, two more.
pub const INDENT: usize = 2 + STATUS_WIDTH + 2;

/// Indent for a command that reports one value rather than a column of findings.
///
/// `preview` and `export` say where a file is; they do not wear the status
/// column, because that space is reserved for a word and leaving it empty reads
/// as something missing.
pub const VALUE_INDENT: usize = 2;

/// One finding that needs doing something about.
///
/// `detail` and `remedy` are wrapped; `subject` is not, because it is a short
/// locator and wrapping one would put half a slide number on its own line.
pub fn block(
    status: &str,
    ink: Ink,
    subject: &str,
    detail: &str,
    remedy: Option<&str>,
    style: &Style,
) -> String {
    let mut text = format!(
        "  {}  {}\n",
        style.pad(ink, status, STATUS_WIDTH),
        style.paint(Ink::Strong, subject)
    );

    text.push_str(&flowed(detail, INDENT, Ink::Strong, style));

    if let Some(remedy) = remedy {
        text.push_str(&hanging(remedy, ink, style));
    }

    text
}

/// One finding there is nothing to do about.
///
/// `width` is the subject column, so a list of them lines up. Sized by the
/// caller, which knows every subject it is about to print.
pub fn line(
    status: &str,
    ink: Ink,
    subject: &str,
    detail: &str,
    width: usize,
    style: &Style,
) -> String {
    let indent = INDENT + width + 2;

    format!(
        "  {}  {}  {}\n",
        style.pad(ink, status, STATUS_WIDTH),
        style.pad(Ink::Strong, subject, width),
        style::wrap(detail, style::WIDTH - indent).join(&format!("\n{}", " ".repeat(indent)))
    )
}

/// A paragraph, wrapped and indented.
pub fn flowed(text: &str, indent: usize, ink: Ink, style: &Style) -> String {
    style::wrap(text, style::WIDTH - indent)
        .iter()
        .map(|line| format!("{}{}\n", " ".repeat(indent), style.paint(ink, line)))
        .collect()
}

/// The next action, hanging under its own marker.
///
/// The marker column is left clear on every continuation line, so the eye can
/// drop straight down the page from one thing-to-do to the next without reading
/// what is in between. That is the whole reason for the hang.
fn hanging(remedy: &str, ink: Ink, style: &Style) -> String {
    let hang = INDENT + REMEDY.len() + 1;
    let wrapped = style::wrap(remedy, style::WIDTH - hang);

    format!(
        "{}{} {}\n",
        " ".repeat(INDENT),
        style.paint(ink, REMEDY),
        wrapped.join(&format!("\n{}", " ".repeat(hang)))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONG: &str = "clear a few gigabytes before you start, or point the recording at an \
                        external drive that has room on it";

    fn plain_block(remedy: Option<&str>) -> String {
        block("FAIL", Ink::Fail, "disk", "1.3 GiB free", remedy, &Style::plain())
    }

    #[test]
    fn a_block_leads_with_the_status_and_then_what_it_is_about() {
        // The two things somebody scanning a page is looking for, in the two
        // places their eye is already going.
        let first = plain_block(None).lines().next().expect("a line").to_string();

        assert_eq!(first.trim(), "FAIL     disk");
    }

    #[test]
    fn a_block_without_a_remedy_prints_no_marker_line() {
        assert!(!plain_block(None).contains(REMEDY));
    }

    #[test]
    fn a_remedy_that_wraps_hangs_clear_of_the_marker_column() {
        let text = plain_block(Some(LONG));
        let continuation = text.lines().find(|line| line.contains("external")).expect("a wrap");

        assert!(!continuation.contains(REMEDY), "{text}");
        assert_eq!(indent_of(continuation), INDENT + REMEDY.len() + 1);
    }

    #[test]
    fn nothing_a_block_prints_runs_past_the_fixed_width() {
        for line in plain_block(Some(LONG)).lines() {
            assert!(
                line.chars().count() <= style::WIDTH,
                "{} columns: {line}",
                line.chars().count()
            );
        }
    }

    #[test]
    fn a_line_keeps_its_subject_in_a_column_so_a_list_of_them_reads_down() {
        let text = format!(
            "{}{}",
            line("PASS", Ink::Pass, "power", "on mains", 14, &Style::plain()),
            line("PASS", Ink::Pass, "network", "online", 14, &Style::plain())
        );

        let starts: Vec<usize> = text
            .lines()
            .map(|row| row.find("on mains").or_else(|| row.find("online")).unwrap_or(0))
            .collect();

        assert_eq!(starts[0], starts[1]);
    }

    #[test]
    fn a_line_whose_detail_is_long_wraps_under_its_own_column() {
        let text = line("PASS", Ink::Pass, "fonts", LONG, 14, &Style::plain());
        let continuation = text.lines().nth(1).expect("a wrap");

        assert_eq!(indent_of(continuation), INDENT + 14 + 2);
        for row in text.lines() {
            assert!(row.chars().count() <= style::WIDTH, "{row}");
        }
    }

    #[test]
    fn the_marker_is_ascii_so_no_console_can_turn_it_into_a_box() {
        // The one line that says what to do renders on whatever terminal the
        // venue laptop turns out to have.
        assert!(REMEDY.is_ascii());
    }

    #[test]
    fn a_report_carries_no_escape_sequences_when_colour_is_off() {
        assert!(!plain_block(Some(LONG)).contains('\u{1b}'));
        assert!(
            !line("PASS", Ink::Pass, "power", "on mains", 8, &Style::plain()).contains('\u{1b}')
        );
    }

    #[test]
    fn colour_changes_no_line_count_and_no_column() {
        // Escape codes are zero-width on screen and several bytes in a string.
        // Counted as characters they shear the report one line at a time.
        assert_eq!(
            plain_block(Some(LONG)).lines().count(),
            block("FAIL", Ink::Fail, "disk", "1.3 GiB free", Some(LONG), &Style::colored())
                .lines()
                .count()
        );
    }

    fn indent_of(line: &str) -> usize {
        line.len() - line.trim_start().len()
    }
}
