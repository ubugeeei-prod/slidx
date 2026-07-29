//! What the picker looks like, as a pure function of what it is showing.
//!
//! Every decision about the frame — which rows are visible, which one is
//! selected, what a row says, how a long path is shortened — is made here,
//! where it can be asserted on. [`super::picker`] is left with the part that
//! genuinely has to touch a terminal: put it in raw mode, read a key, draw a
//! string, put it back.
//!
//! That split is the point. A picker whose layout can only be checked by
//! looking at one is a picker whose layout is never checked.
//!
//! ## The shape
//!
//! ```text
//!   vue                                                    2 of 7
//!
//! > Making decks fast              ~/talks/vueconf
//!     VueConf, 2026-03-14
//!   Deck review                    ~/work/decks/arch
//! ```
//!
//! The event and date appear under the selected row and nowhere else. They are
//! what tells two decks called `slides` apart, and they are only worth the line
//! for the one somebody is looking at.

use std::path::Path;

use crate::index::Entry;
use crate::style::{self, Ink, Style};

/// Marks the row that Enter would choose.
///
/// ASCII, like everything else slidx draws: this runs over SSH, in tmux, and in
/// whatever terminal the machine has, and a marker that renders as a box is
/// worse than one that renders as a chevron.
const CURSOR: &str = ">";

/// How many rows to show at once.
///
/// Not the terminal's height. A picker that fills the screen erases whatever
/// somebody was reading before they opened it, and eight is more candidates
/// than anyone scans before they type another character.
pub const VISIBLE: usize = 8;

/// Everything the frame depends on.
#[derive(Debug, Clone)]
pub struct Screen<'a> {
    pub query: &'a str,
    pub matches: &'a [&'a Entry],
    pub selected: usize,
    /// The user's home directory, for shortening paths to `~`.
    pub home: Option<&'a Path>,
}

impl Screen<'_> {
    /// The whole frame, ready to write.
    pub fn draw(&self, style: &Style) -> String {
        let mut text = format!("  {}\n\n", self.header(style));

        if self.matches.is_empty() {
            text.push_str(&format!(
                "  {}\n",
                style.paint(Ink::Faint, "no deck matches. backspace to widen, esc to give up.")
            ));
            return text;
        }

        for (offset, entry) in self.window().iter().enumerate() {
            let index = self.first_visible() + offset;
            text.push_str(&self.row(entry, index == self.selected, style));
        }

        text
    }

    /// The query, and where in the list the cursor is.
    fn header(&self, style: &Style) -> String {
        let position = if self.matches.is_empty() {
            "none".to_string()
        } else {
            format!("{} of {}", self.selected + 1, self.matches.len())
        };

        let query = if self.query.is_empty() {
            style.paint(Ink::Faint, "type to narrow")
        } else {
            style.paint(Ink::Strong, self.query)
        };

        let width = style::WIDTH.saturating_sub(4 + position.chars().count());
        let padding = width.saturating_sub(display_width(self.query.max_len(width)));

        format!("{query}{}{}", " ".repeat(padding.max(1)), style.paint(Ink::Faint, position))
    }

    fn row(&self, entry: &Entry, selected: bool, style: &Style) -> String {
        let marker = if selected { CURSOR } else { " " };
        let label = entry.label();
        let path = shorten(&entry.path, self.home);

        // The label column is sized so the paths line up and can be scanned as
        // a column of their own; a label longer than it pushes its path right
        // rather than being cut, because the label is the thing being read.
        let gap = 30usize.saturating_sub(display_width(&label)).max(2);
        let ink = if selected { Ink::Strong } else { Ink::Faint };

        let mut text = format!(
            "{} {}{}{}\n",
            style.paint(ink, marker),
            style.paint(if selected { Ink::Strong } else { Ink::Faint }, &label),
            " ".repeat(gap),
            style.paint(Ink::Faint, &path)
        );

        // Only under the cursor: this is what tells two decks called `slides`
        // apart, and it is only worth a line for the one being looked at.
        if selected {
            if let Some(occasion) = entry.occasion() {
                text.push_str(&format!("    {}\n", style.paint(Ink::Faint, occasion)));
            }
        }

        text
    }

    /// The rows on screen, scrolled to keep the selection in view.
    fn window(&self) -> &[&Entry] {
        let first = self.first_visible();
        let last = (first + VISIBLE).min(self.matches.len());

        &self.matches[first..last]
    }

    /// Scrolls only as far as it has to.
    ///
    /// The list stays still while the cursor moves inside it, and moves by one
    /// when the cursor reaches an edge. A window that recentres on every
    /// keypress makes the whole page move under somebody who is reading it.
    fn first_visible(&self) -> usize {
        if self.selected < VISIBLE {
            return 0;
        }

        (self.selected + 1).saturating_sub(VISIBLE).min(self.matches.len().saturating_sub(VISIBLE))
    }

    /// How many lines [`Self::draw`] produced, so the picker knows how much to
    /// erase before it draws the next one.
    pub fn height(&self, style: &Style) -> usize {
        self.draw(style).lines().count()
    }
}

/// Trims a query for the header without panicking on a multi-byte boundary.
trait MaxLen {
    fn max_len(&self, width: usize) -> &str;
}

impl MaxLen for str {
    fn max_len(&self, width: usize) -> &str {
        match self.char_indices().nth(width) {
            Some((at, _)) => &self[..at],
            None => self,
        }
    }
}

/// `~/talks/vueconf` rather than `/home/somebody/talks/vueconf`.
///
/// Everybody's projects live under their home directory, so the prefix is the
/// same on every row and carries no information — and dropping it is what makes
/// the part that does differ fit on the line.
pub fn shorten(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

fn display_width(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(path: &str, title: Option<&str>) -> Entry {
        let mut entry = Entry::new(path);
        entry.title = title.map(str::to_string);
        entry
    }

    fn screen<'a>(matches: &'a [&'a Entry], selected: usize, query: &'a str) -> Screen<'a> {
        Screen { query, matches, selected, home: Some(Path::new("/home/somebody")) }
    }

    fn many(count: usize) -> Vec<Entry> {
        (0..count).map(|n| entry(&format!("/home/somebody/talks/{n}"), None)).collect()
    }

    fn refs(entries: &[Entry]) -> Vec<&Entry> {
        entries.iter().collect()
    }

    #[test]
    fn the_selected_row_is_the_only_one_wearing_the_cursor() {
        let entries = many(3);
        let text = screen(&refs(&entries), 1, "").draw(&Style::plain());
        let marked: Vec<&str> = text.lines().filter(|line| line.starts_with(CURSOR)).collect();

        assert_eq!(marked.len(), 1);
        assert!(marked[0].contains('1'), "{text}");
    }

    #[test]
    fn a_deck_is_shown_by_its_title_when_it_has_one() {
        let entries = vec![entry("/home/somebody/talks/vueconf", Some("Making decks fast"))];
        let text = screen(&refs(&entries), 0, "").draw(&Style::plain());

        assert!(text.contains("Making decks fast"), "{text}");
    }

    #[test]
    fn the_occasion_is_shown_only_under_the_row_being_looked_at() {
        // It is what tells two decks called `slides` apart, and it is only
        // worth a line for the one somebody has the cursor on.
        let mut first = entry("/home/somebody/a", Some("First"));
        first.event = Some("VueConf".into());
        let mut second = entry("/home/somebody/b", Some("Second"));
        second.event = Some("RustFest".into());

        let entries = vec![first, second];
        let text = screen(&refs(&entries), 0, "").draw(&Style::plain());

        assert!(text.contains("VueConf"), "{text}");
        assert!(!text.contains("RustFest"), "{text}");
    }

    #[test]
    fn a_home_directory_is_shortened_to_a_tilde() {
        // The prefix is the same on every row and carries no information.
        // Dropping it is what makes the part that differs fit.
        let entries = many(1);
        let text = screen(&refs(&entries), 0, "").draw(&Style::plain());

        assert!(text.contains("~/talks/0"), "{text}");
        assert!(!text.contains("/home/somebody"), "{text}");
    }

    #[test]
    fn a_path_outside_the_home_directory_is_shown_whole() {
        assert_eq!(
            shorten(Path::new("/opt/decks/a"), Some(Path::new("/home/somebody"))),
            "/opt/decks/a"
        );
    }

    #[test]
    fn a_machine_with_no_home_directory_still_renders_paths() {
        assert_eq!(shorten(Path::new("/opt/a"), None), "/opt/a");
    }

    #[test]
    fn the_header_says_where_in_the_list_the_cursor_is() {
        let entries = many(7);

        assert!(screen(&refs(&entries), 1, "vue").draw(&Style::plain()).contains("2 of 7"));
    }

    #[test]
    fn an_empty_query_invites_typing_rather_than_showing_a_blank_line() {
        let entries = many(2);

        assert!(screen(&refs(&entries), 0, "").draw(&Style::plain()).contains("type to narrow"));
    }

    #[test]
    fn no_matches_says_so_and_says_what_to_do_about_it() {
        // A blank picker looks broken. It has to name both ways out.
        let text = screen(&[], 0, "zzz").draw(&Style::plain());

        assert!(text.contains("no deck matches"), "{text}");
        assert!(text.contains("backspace"), "{text}");
        assert!(text.contains("esc"), "{text}");
    }

    #[test]
    fn only_a_windowful_is_drawn_however_long_the_list_is() {
        // A picker that fills the screen erases whatever somebody was reading
        // before they opened it.
        let entries = many(100);
        let text = screen(&refs(&entries), 0, "").draw(&Style::plain());
        let rows = text.lines().filter(|line| line.contains("~/talks/")).count();

        assert_eq!(rows, VISIBLE);
    }

    #[test]
    fn the_list_stays_still_until_the_cursor_reaches_the_bottom_of_it() {
        // A window that recentres on every keypress makes the whole page move
        // under somebody who is reading it.
        let entries = many(100);

        for selected in 0..VISIBLE {
            let text = screen(&refs(&entries), selected, "").draw(&Style::plain());
            assert!(text.contains("~/talks/0"), "moved too early at {selected}");
        }
    }

    #[test]
    fn the_window_follows_the_cursor_past_the_bottom_by_one_row_at_a_time() {
        let entries = many(100);
        let text = screen(&refs(&entries), VISIBLE, "").draw(&Style::plain());

        assert!(!text.contains("~/talks/0\n"), "{text}");
        assert!(text.contains(&format!("~/talks/{VISIBLE}")), "{text}");
    }

    #[test]
    fn the_last_entry_is_reachable_without_scrolling_past_the_end() {
        let entries = many(10);
        let text = screen(&refs(&entries), 9, "").draw(&Style::plain());

        assert!(text.contains("~/talks/9"), "{text}");
        assert_eq!(text.lines().filter(|line| line.contains("~/talks/")).count(), VISIBLE);
    }

    #[test]
    fn a_list_shorter_than_the_window_draws_all_of_it() {
        let entries = many(3);
        let text = screen(&refs(&entries), 0, "").draw(&Style::plain());

        assert_eq!(text.lines().filter(|line| line.contains("~/talks/")).count(), 3);
    }

    #[test]
    fn nothing_drawn_runs_past_the_fixed_width_for_an_ordinary_deck() {
        let mut long = entry("/home/somebody/code/talks/vueconf-tokyo", Some("Making decks fast"));
        long.event = Some("VueConf Tokyo".into());
        let entries = vec![long];

        for line in screen(&refs(&entries), 0, "making").draw(&Style::plain()).lines() {
            assert!(line.chars().count() <= style::WIDTH, "{} cols: {line}", line.chars().count());
        }
    }

    #[test]
    fn the_frame_is_ascii_so_it_renders_over_ssh_and_in_any_terminal() {
        let entries = many(3);

        assert!(screen(&refs(&entries), 0, "t").draw(&Style::plain()).is_ascii());
    }

    #[test]
    fn colour_changes_the_frames_height_by_nothing() {
        // The picker erases exactly as many lines as it drew. If colour changed
        // the count, every redraw would leave a trail of half-erased rows.
        let entries = many(5);
        let matches = refs(&entries);
        let view = screen(&matches, 2, "t");

        assert_eq!(view.height(&Style::plain()), view.height(&Style::colored()));
    }

    #[test]
    fn a_query_with_multi_byte_characters_does_not_split_a_codepoint() {
        // A deck searched for in Japanese. Trimming the header by bytes would
        // panic mid-character.
        let entries = many(1);
        let long: String = "日".repeat(200);

        let _ = screen(&refs(&entries), 0, &long).draw(&Style::plain());
    }

    #[test]
    fn an_entry_with_no_title_falls_back_to_its_directory_name() {
        let entries = vec![entry("/home/somebody/talks/vueconf", None)];
        let text = screen(&refs(&entries), 0, "").draw(&Style::plain());

        assert!(text.contains("vueconf"), "{text}");
    }

    #[test]
    fn a_very_long_title_pushes_its_path_along_rather_than_being_cut() {
        // The label is the thing being read. Truncating it to keep a column
        // tidy trades the useful half for the decorative one.
        let title = "A title long enough to run past the column it is given";
        let entries = vec![entry("/home/somebody/a", Some(title))];
        let text = screen(&refs(&entries), 0, "").draw(&Style::plain());

        assert!(text.contains(title), "{text}");
    }

    #[test]
    fn the_path_column_lines_up_across_rows_of_similar_titles() {
        let entries =
            vec![entry("/home/somebody/a", Some("One")), entry("/home/somebody/b", Some("Two"))];
        let text = screen(&refs(&entries), 5, "").draw(&Style::plain());
        let columns: Vec<Option<usize>> =
            text.lines().filter(|line| line.contains("~/")).map(|line| line.find("~/")).collect();

        assert_eq!(columns[0], columns[1]);
    }

    #[test]
    fn a_selection_past_the_end_of_the_list_still_draws_something() {
        // Reachable when the query narrows the list under a cursor that was
        // further down. Panicking there would take the terminal with it.
        let entries = many(2);
        let matches = refs(&entries);
        let home = PathBuf::from("/home/somebody");
        let text = Screen { query: "x", matches: &matches, selected: 99, home: Some(&home) }
            .draw(&Style::plain());

        assert!(!text.is_empty());
    }
}
