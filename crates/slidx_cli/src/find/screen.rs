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
//!
//! ## What matched is marked
//!
//! Each row brackets the characters the query landed on, from the positions
//! [`super::scoring`] already computed — see [`super::highlight`]. Without it a
//! row says *that* it matched and never *where*, so a list of eight has to be
//! re-read rather than scanned.
//!
//! The columns are measured in cells and not in characters, which is what keeps
//! a Japanese title from pushing the path column half a column left on its row
//! and nowhere else.

use std::path::Path;

use super::highlight;
use super::Hit;
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
    pub matches: &'a [Hit<'a>],
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

        for (offset, hit) in self.window().iter().enumerate() {
            let index = self.first_visible() + offset;
            text.push_str(&self.row(hit, index == self.selected, style));
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

        let room = style::WIDTH.saturating_sub(4 + display_width(&position));
        let padding = room.saturating_sub(display_width(style::width::clip(self.query, room)));

        format!("{query}{}{}", " ".repeat(padding.max(1)), style.paint(Ink::Faint, position))
    }

    fn row(&self, hit: &Hit<'_>, selected: bool, style: &Style) -> String {
        let entry = hit.entry;
        let marker = if selected { CURSOR } else { " " };
        let label = entry.label();
        let path = shorten(&entry.path, self.home);

        let ink = if selected { Ink::Strong } else { Ink::Faint };
        let marked_label = highlight::marked(&label, &matched_in_label(hit), ink, style);
        let marked_path = highlight::marked(&path, &matched_in_path(hit, &path), Ink::Faint, style);

        // The label column is sized so the paths line up and can be scanned as
        // a column of their own; a label longer than it pushes its path right
        // rather than being cut, because the label is the thing being read.
        // Measured on what is drawn, so the brackets a highlight adds are part
        // of the column rather than something that shifts it.
        let gap = 30usize.saturating_sub(display_width(&marked_label)).max(2);

        let mut text = format!(
            "{} {}{}{}\n",
            style.paint(ink, marker),
            marked_label,
            " ".repeat(gap),
            marked_path
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
    fn window(&self) -> &[Hit<'_>] {
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

/// Where the query landed inside the label.
///
/// A haystack is the path, then the title, then the event. The label is the
/// title when there is one and the last segment of the path when there is not,
/// so the positions are rebased onto whichever of those the label came from.
/// Anything outside it belongs to the path's column or to the event, and is that
/// column's to show or to leave.
fn matched_in_label(hit: &Hit<'_>) -> Vec<usize> {
    let entry = hit.entry;
    let path = entry.path.display().to_string();

    let span = match entry.title.as_deref().filter(|title| !title.trim().is_empty()) {
        // `Entry::haystack` joins the path and the title with one space.
        Some(title) => path.len() + 1..path.len() + 1 + title.len(),
        None => {
            let label = entry.label();
            match path.len().checked_sub(label.len()) {
                Some(start) if path.ends_with(&label) => start..path.len(),
                _ => return Vec::new(),
            }
        }
    };

    highlight::rebased(&hit.found.positions, span)
}

/// Where the query landed inside the path, as offsets into the shortened form
/// the row actually shows.
fn matched_in_path(hit: &Hit<'_>, drawn: &str) -> Vec<usize> {
    let full = hit.entry.path.display().to_string();
    let inside = highlight::rebased(&hit.found.positions, 0..full.len());

    // `~/talks/x` for `/home/somebody/talks/x`: the home directory and the
    // separator after it became `~/`, so what is still on screen starts one
    // character in, and a match inside the home directory is not on screen at
    // all. The boundary is the length of the prefix that was replaced, which is
    // the difference in length plus the one character it was replaced with.
    match full.len().checked_sub(drawn.len()) {
        Some(0) | None => inside,
        Some(shorter) => highlight::after_prefix(&inside, shorter + 1, 1),
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

/// Cells, not characters.
///
/// [`crate::style::width`] is the one answer to this in the binary. Counting
/// characters here is what put the path column half a column left on any row
/// with a Japanese title in it, and only on those rows.
fn display_width(text: &str) -> usize {
    style::width::of(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::index::Entry;

    fn entry(path: &str, title: Option<&str>) -> Entry {
        let mut entry = Entry::new(path);
        entry.title = title.map(str::to_string);
        entry
    }

    fn screen<'a>(matches: &'a [Hit<'a>], selected: usize, query: &'a str) -> Screen<'a> {
        Screen { query, matches, selected, home: Some(Path::new("/home/somebody")) }
    }

    fn many(count: usize) -> Vec<Entry> {
        (0..count).map(|n| entry(&format!("/home/somebody/talks/{n}"), None)).collect()
    }

    /// Entries with nothing highlighted, for the tests about layout.
    fn refs(entries: &[Entry]) -> Vec<Hit<'_>> {
        entries
            .iter()
            .map(|entry| Hit { entry, found: super::super::scoring::Match::default() })
            .collect()
    }

    /// Entries scored against a real query, for the tests about the highlight.
    fn hits<'a>(entries: &'a [Entry], query: &str) -> Vec<Hit<'a>> {
        entries
            .iter()
            .filter_map(|entry| {
                super::super::scoring::score(query, &entry.haystack())
                    .map(|found| Hit { entry, found })
            })
            .collect()
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

    #[test]
    fn a_row_shows_which_characters_of_the_path_the_query_matched() {
        // The whole point: eight rows that all match are a list you re-read.
        // Eight rows that say where they matched are a list you scan.
        let entries = vec![entry("/home/somebody/talks/vueconf", None)];
        let found = hits(&entries, "vue");
        let text = screen(&found, 0, "vue").draw(&Style::plain());

        assert!(text.contains("[vue]conf"), "{text}");
    }

    #[test]
    fn a_row_shows_which_characters_of_the_title_the_query_matched() {
        // `fast` cannot match anywhere in this path, so all of it lands in the
        // title — which is what makes the expectation here exact.
        let entries = vec![entry("/home/somebody/a", Some("Making decks fast"))];
        let found = hits(&entries, "fast");
        let text = screen(&found, 0, "fast").draw(&Style::plain());

        assert!(text.contains("[fast]"), "{text}");
    }

    #[test]
    fn a_query_that_spans_the_path_and_the_title_is_marked_in_both() {
        // The haystack is one string and the row is two columns. A highlight
        // that gave up when a match crossed the join would go missing on
        // exactly the queries people type, which are prefixes of both.
        let entries = vec![entry("/home/somebody/talks/vueconf", Some("Making decks fast"))];
        let found = hits(&entries, "vuefast");
        let text = screen(&found, 0, "vuefast").draw(&Style::plain());

        let row = text.lines().find(|line| line.contains("~/")).expect("a row");
        let (label, path) = row.split_at(row.find("~/").expect("a path"));

        assert!(label.contains('['), "nothing marked in the title: {row}");
        assert!(path.contains('['), "nothing marked in the path: {row}");
    }

    #[test]
    fn a_match_inside_the_home_directory_is_not_marked_because_it_is_not_shown() {
        // The path is drawn as `~/…`, so a match on `somebody` is off screen.
        // Marking it anyway would put brackets around the tilde.
        let entries = vec![entry("/home/somebody/talks/x", None)];
        let found = hits(&entries, "somebody");
        let text = screen(&found, 0, "somebody").draw(&Style::plain());

        assert!(text.contains("~/talks/x"), "{text}");
        assert!(!text.contains('['), "{text}");
    }

    #[test]
    fn a_highlight_in_a_japanese_title_leaves_the_path_column_where_it_was() {
        // Two things at once, and both were broken before: the mark falls on
        // whole characters, and the column beside it is measured in cells.
        let plain = vec![entry("/home/somebody/a", Some("Making decks fast"))];
        let japanese = vec![entry("/home/somebody/b", Some("日本語のトーク"))];

        let unmarked = screen(&refs(&plain), 0, "").draw(&Style::plain());
        let marked = screen(&hits(&japanese, "トーク"), 0, "トーク").draw(&Style::plain());

        assert!(marked.contains("[トーク]"), "{marked}");
        assert_eq!(
            marked.lines().next().map(|_| ()),
            unmarked.lines().next().map(|_| ()),
            "both drew a header"
        );

        // The path starts in the same column on both rows, which is the thing a
        // character count got wrong.
        let column = |text: &str| {
            text.lines()
                .find(|line| line.contains("~/"))
                .map(|line| style::width::of(&line[..line.find("~/").expect("a path")]))
        };
        assert_eq!(column(&marked), column(&unmarked), "{marked}{unmarked}");
    }

    #[test]
    fn the_highlight_survives_a_terminal_with_no_colour_at_all() {
        // NO_COLOR, a pipe, a dumb terminal. Colour is never the only carrier.
        let entries = vec![entry("/home/somebody/talks/vueconf", None)];
        let found = hits(&entries, "vue");

        let plain = screen(&found, 0, "vue").draw(&Style::plain());
        let colored = screen(&found, 0, "vue").draw(&Style::colored());

        assert!(plain.contains("[vue]"), "{plain}");
        assert!(!plain.contains('\u{1b}'));
        assert!(colored.contains("[vue]"), "{colored}");
    }

    #[test]
    fn colour_does_not_move_a_column_the_highlight_is_on() {
        // The picker erases what it drew. A frame whose width depended on colour
        // would leave half-erased rows behind on every keystroke.
        let entries = vec![entry("/home/somebody/talks/vueconf", Some("日本語のトーク"))];
        let found = hits(&entries, "vue");

        let plain = screen(&found, 0, "vue").draw(&Style::plain());
        let colored = screen(&found, 0, "vue").draw(&Style::colored());

        let widths = |text: &str| text.lines().map(style::width::of).collect::<Vec<_>>();
        assert_eq!(widths(&plain), widths(&colored));
    }

    #[test]
    fn an_empty_query_marks_nothing_rather_than_every_row() {
        let entries = many(3);

        assert!(!screen(&refs(&entries), 0, "").draw(&Style::plain()).contains('['));
    }
}
