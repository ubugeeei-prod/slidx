//! `slidx list` — the decks on this machine, as a table.
//!
//! ## Why not just the names
//!
//! A speaker keeps five decks and half of them are called `slides`. A list of
//! paths is a list somebody has to open one by one to find the one they want,
//! which is the state the index exists to end. So a row carries what tells two
//! talks apart: the deck's title, how many slides it has, the slot it was
//! written for, when it was last worked on, and the event it was given at.
//!
//! ## Read, not remembered
//!
//! The slide count and the duration come out of the deck every time. The index
//! could cache them — it already caches the title — and then a deck edited since
//! would report a number that was true last month. A count that is sometimes
//! wrong is worse than a count that costs a file read, because nobody can tell
//! which kind they are looking at.
//!
//! Parsing is what makes that affordable: a hundred-slide deck parses in
//! milliseconds, and the index holds a few hundred projects at most.
//!
//! ## What a missing value means
//!
//! A dash, never a zero and never a guess. A deck with no `duration:` has not
//! been given a slot, which is a different thing from a slot of zero minutes,
//! and the linter is the place that has an opinion about it.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use slidx_core::{parse_deck, DeckParseOptions};

use crate::args::Matches;
use crate::home::Home;
use crate::index::{Entry, Index};
use crate::lint::source;
use crate::project;
use crate::style::{self, Ink, Style};
use crate::{Outcome, OK};

/// Past this a title is cut short, so the columns after it stay where the eye
/// learned they were. The path is not in the table — `slidx cd` is how somebody
/// gets to one — so nothing here has to hold a full path.
const TITLE_BUDGET: usize = 28;

/// One project, as a row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub path: PathBuf,
    pub title: String,
    /// `None` when the project no longer holds a deck anything can read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slides: Option<usize>,
    /// The declared slot, in seconds. Not an estimate — a deck that has not
    /// declared one has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,
    /// Unix seconds. The newest of the deck's own files, falling back to when
    /// slidx last saw the project.
    pub touched: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
}

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let home = Home::discover();
    let index = Index::load(&home.index()).pruned();

    // Cleaned while it is open, which is the moment the stat is paid for
    // anyway. See `crate::index` on why the write path never does it.
    let _ = index.save(&home.index());

    if index.is_empty() {
        return Outcome::misuse(crate::find::nothing_indexed());
    }

    let rows: Vec<Row> = index.entries().iter().map(describe).collect();

    if matches.is_set("json") {
        return match serde_json::to_string_pretty(&rows) {
            Ok(json) => Outcome::out(format!("{json}\n")),
            Err(error) => Outcome::misuse(format!("could not serialise the list: {error}\n")),
        };
    }

    Outcome::out(table(&rows, now(), style)).with_code(OK)
}

/// Everything one row says, read off disk where disk is the honest source.
pub fn describe(entry: &Entry) -> Row {
    let deck = project::primary_deck(&entry.path).and_then(|path| read(&path));

    Row {
        path: entry.path.clone(),
        title: label(entry, deck.as_ref()),
        slides: deck.as_ref().map(|deck| deck.slides),
        duration_seconds: deck.as_ref().and_then(|deck| deck.duration_seconds),
        touched: deck.as_ref().and_then(|deck| deck.touched).unwrap_or(entry.last_seen),
        event: deck.as_ref().and_then(|deck| deck.event.clone()).or_else(|| entry.event.clone()),
    }
}

/// What one project's deck says about itself.
#[derive(Debug, Clone)]
struct Read {
    title: Option<String>,
    event: Option<String>,
    slides: usize,
    duration_seconds: Option<u32>,
    touched: Option<u64>,
}

fn read(path: &Path) -> Option<Read> {
    let deck_source = source::read(path, &DeckParseOptions::default().separator).ok()?;
    let deck = parse_deck(&deck_source.source, &DeckParseOptions::default());

    let touched = if deck_source.files.is_empty() {
        project::touched(&[path.to_path_buf()])
    } else {
        project::touched(&deck_source.paths())
    };

    Some(Read {
        title: deck.meta.title.clone(),
        event: deck.meta.talk.event.clone(),
        slides: deck.slides.len(),
        duration_seconds: deck.meta.duration_seconds,
        touched,
    })
}

/// The deck's own title, then the index's, then the directory name.
///
/// The deck's own comes first because it is the one that is current: an entry
/// recorded before the author retitled the talk is stale, and the table would
/// otherwise show a name that is no longer on the title slide.
fn label(entry: &Entry, deck: Option<&Read>) -> String {
    deck.and_then(|deck| deck.title.clone())
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| entry.label())
}

/// The table, most recently touched first.
fn table(rows: &[Row], now: u64, style: &Style) -> String {
    let mut sorted: Vec<&Row> = rows.iter().collect();
    sorted.sort_by_key(|row| std::cmp::Reverse(row.touched));

    // Cells rather than characters. A Japanese title is one character and two
    // cells, so a column sized by character count is half the width the row
    // needs — and every column after it starts in a different place on that row
    // than on the others.
    let width = sorted
        .iter()
        .map(|row| style::width::of(&clipped(&row.title)))
        .max()
        .unwrap_or(0)
        .min(TITLE_BUDGET);

    let mut text = format!(
        "  {}  {}  {}  {}  {}\n",
        style.pad(Ink::Faint, "DECK", width),
        style.pad(Ink::Faint, "SLIDES", 6),
        style.pad(Ink::Faint, "SLOT", 5),
        style.pad(Ink::Faint, "TOUCHED", 9),
        style.paint(Ink::Faint, "EVENT")
    );

    for row in &sorted {
        text.push_str(&format!(
            "  {}  {}  {}  {}  {}\n",
            style.pad(Ink::Strong, &clipped(&row.title), width),
            style.pad(Ink::Faint, &count(row.slides), 6),
            style.pad(Ink::Faint, &slot(row.duration_seconds), 5),
            style.pad(Ink::Faint, &project::ago(row.touched, now), 9),
            style.paint(Ink::Faint, row.event.as_deref().unwrap_or(MISSING))
        ));
    }

    text.push_str(&format!(
        "\n  {}\n",
        style.paint(
            Ink::Faint,
            format!(
                "{} {}. `slidx cd <query>` to go to one.",
                sorted.len(),
                if sorted.len() == 1 { "deck" } else { "decks" }
            )
        )
    ));

    text
}

/// What a value the deck does not carry looks like.
///
/// A dash rather than `0` or `?`: a deck with no declared slot has not been
/// given one, and printing a number would make it look measured.
const MISSING: &str = "—";

fn count(slides: Option<usize>) -> String {
    slides.map(|slides| slides.to_string()).unwrap_or_else(|| MISSING.to_string())
}

/// The declared slot, in the same spelling the frontmatter uses.
fn slot(seconds: Option<u32>) -> String {
    match seconds {
        None => MISSING.to_string(),
        Some(seconds) if seconds.is_multiple_of(60) => format!("{}m", seconds / 60),
        Some(seconds) => format!("{}m{}s", seconds / 60, seconds % 60),
    }
}

/// A title cut to the column, with an ellipsis so nobody reads a clipped title
/// as the whole one.
fn clipped(title: &str) -> String {
    if style::width::of(title) <= TITLE_BUDGET {
        return title.to_string();
    }

    // The budget is cells, so a title of Japanese is cut at fourteen characters
    // rather than at twenty-eight — which is what keeps it inside the column
    // instead of pushing the four columns after it along.
    format!("{}…", style::width::clip(title, TITLE_BUDGET - 1).trim_end())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(title: &str, slides: Option<usize>, seconds: Option<u32>, touched: u64) -> Row {
        Row {
            path: PathBuf::from(format!("/talks/{}", title.to_lowercase().replace(' ', "-"))),
            title: title.to_string(),
            slides,
            duration_seconds: seconds,
            touched,
            event: None,
        }
    }

    fn rendered(rows: &[Row], now: u64) -> String {
        table(rows, now, &Style::plain())
    }

    #[test]
    fn a_row_says_more_than_the_name_because_half_of_them_are_called_slides() {
        let text = rendered(&[row("Making decks fast", Some(24), Some(1200), 0)], 0);

        assert!(text.contains("Making decks fast"), "{text}");
        assert!(text.contains("24"), "{text}");
        assert!(text.contains("20m"), "{text}");
    }

    #[test]
    fn the_most_recently_touched_deck_is_first() {
        // What somebody is looking for is almost always what they touched last,
        // which is the same order the picker opens on.
        let day = 24 * 60 * 60;
        let text = rendered(
            &[
                row("Older talk", Some(1), None, 0),
                row("Newest talk", Some(1), None, 10 * day),
                row("Middle talk", Some(1), None, 5 * day),
            ],
            10 * day,
        );

        let order: Vec<&str> = text
            .lines()
            .filter_map(|line| {
                ["Newest talk", "Middle talk", "Older talk"]
                    .into_iter()
                    .find(|title| line.contains(title))
            })
            .collect();

        assert_eq!(order, ["Newest talk", "Middle talk", "Older talk"]);
    }

    #[test]
    fn a_deck_with_no_declared_slot_shows_a_dash_rather_than_zero_minutes() {
        // A deck without a `duration:` has not been given a slot. `0m` would
        // read as one that is over budget by definition.
        let text = rendered(&[row("A brown bag", Some(4), None, 0)], 0);

        assert!(text.contains(MISSING), "{text}");
        assert!(!text.contains("0m"), "{text}");
    }

    #[test]
    fn a_project_whose_deck_cannot_be_read_still_gets_a_row() {
        // A project can be in the index with its deck half-moved, and dropping
        // the row would be the tool saying the project is gone.
        let text = rendered(&[row("Half moved", None, None, 0)], 0);

        assert!(text.contains("Half moved"), "{text}");
        assert!(text.contains(MISSING), "{text}");
    }

    #[test]
    fn a_slot_that_is_not_whole_minutes_keeps_its_seconds() {
        assert_eq!(slot(Some(1200)), "20m");
        assert_eq!(slot(Some(90)), "1m30s");
        assert_eq!(slot(None), MISSING);
    }

    #[test]
    fn a_japanese_title_keeps_the_columns_after_it_where_they_are() {
        // The bug this replaced: the column was sized in characters and the
        // title drawn in cells, so `SLIDES` began seven cells further right on
        // the one row with a Japanese title in it — and this maintainer's decks
        // are in Japanese.
        let text = rendered(
            &[
                row("日本語のトーク", Some(12), Some(1200), 0),
                row("Making decks fast", Some(9), None, 0),
            ],
            0,
        );

        let starts: Vec<usize> = text
            .lines()
            .filter(|line| line.contains("12") || line.contains('9'))
            .filter_map(|line| line.rfind("  ").map(|_| column_of(line, "  ")))
            .collect();

        assert!(starts.windows(2).all(|pair| pair[0] == pair[1]), "{starts:?}\n{text}");
    }

    #[test]
    fn a_long_japanese_title_is_cut_by_the_cells_it_occupies() {
        // Twenty-eight characters of Japanese is fifty-six cells, which is most
        // of the page. The budget is a column width, so it has to be cells.
        let title = "日".repeat(40);
        let text = rendered(&[row(&title, Some(3), None, 0)], 0);

        assert!(text.contains('…'), "{text}");
        for line in text.lines() {
            assert!(
                crate::style::width::of(line) <= crate::style::WIDTH,
                "{} columns: {line}",
                crate::style::width::of(line)
            );
        }
    }

    /// Where the last run of two spaces ends, in cells — the start of the final
    /// column on a row.
    fn column_of(line: &str, separator: &str) -> usize {
        let at = line.rfind(separator).map(|at| at + separator.len()).unwrap_or(0);

        crate::style::width::of(&line[..at])
    }

    #[test]
    fn a_long_title_is_cut_short_so_the_columns_after_it_stay_put() {
        // The eye learns where a column starts. A title allowed to run wide
        // moves every number on that row and makes the table unscannable.
        let title = "A talk with an extremely long title that nobody would fit on a slide";
        let text = rendered(&[row(title, Some(3), None, 0)], 0);

        assert!(text.contains('…'), "{text}");
        for line in text.lines() {
            assert!(
                line.chars().count() <= crate::style::WIDTH,
                "{} columns: {line}",
                line.chars().count()
            );
        }
    }

    #[test]
    fn a_title_that_fits_is_left_exactly_as_the_author_wrote_it() {
        assert_eq!(clipped("Making decks fast"), "Making decks fast");
    }

    #[test]
    fn the_columns_line_up_in_every_row() {
        let text = rendered(
            &[
                row("Short", Some(2), Some(600), 0),
                row("A longer deck title", Some(30), Some(2700), 0),
            ],
            0,
        );

        let starts: Vec<Option<usize>> =
            text.lines().take(3).map(|line| line.find("  ").map(|_| line.len())).collect();
        assert!(starts.iter().all(Option::is_some));

        // Every row is the same width, which is what a padded column means.
        let widths: Vec<usize> = text
            .lines()
            .filter(|line| line.contains("Short") || line.contains("A longer deck title"))
            .map(|line| line.find(|c: char| c.is_ascii_digit()).unwrap_or(0))
            .collect();
        assert_eq!(widths[0], widths[1], "{text}");
    }

    #[test]
    fn the_table_says_how_to_get_to_one_of_the_decks_it_listed() {
        // A list of decks somebody cannot act on is a list they have to
        // translate into a path by hand.
        assert!(rendered(&[row("A talk", Some(1), None, 0)], 0).contains("slidx cd"));
    }

    #[test]
    fn one_deck_is_counted_in_the_singular() {
        assert!(rendered(&[row("A talk", Some(1), None, 0)], 0).contains("1 deck."));
        assert!(rendered(&[row("A", Some(1), None, 0), row("B", Some(1), None, 0)], 0)
            .contains("2 decks."));
    }

    #[test]
    fn the_table_carries_no_escape_sequences_when_colour_is_off() {
        assert!(!rendered(&[row("A talk", Some(1), Some(600), 0)], 0).contains('\u{1b}'));
    }

    #[test]
    fn json_carries_the_path_that_the_table_leaves_out() {
        // The table is for reading and the JSON is for scripting, so the JSON
        // is the one that has to be complete.
        let json = serde_json::to_string(&[row("A talk", Some(3), Some(600), 42)]).expect("json");

        assert!(json.contains("\"path\""), "{json}");
        assert!(json.contains("\"slides\":3"), "{json}");
        assert!(json.contains("\"durationSeconds\":600"), "{json}");
        assert!(json.contains("\"touched\":42"), "{json}");
    }

    #[test]
    fn json_leaves_out_what_a_deck_does_not_say_rather_than_writing_null() {
        // A consumer checking `row.slides === 0` would be wrong; one checking
        // for the key's absence cannot be.
        let json = serde_json::to_string(&[row("A talk", None, None, 0)]).expect("json");

        assert!(!json.contains("slides"), "{json}");
        assert!(!json.contains("null"), "{json}");
    }
}
