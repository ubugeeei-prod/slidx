//! `slidx grep` — the text of every deck this machine has seen.
//!
//! ## Why not `grep -r`
//!
//! Because a line number in a Markdown file is not where a speaker keeps their
//! content. `slides/0007.md:12` answers a question nobody asked; "slide 7 of
//! the VueConf deck" is the address the talk exists at, and it is the one
//! somebody can act on — open the deck, walk to slide 7, fix the line.
//!
//! Getting there needs the parser, which is the whole reason this lives in
//! slidx rather than in a shell alias. A slide is not a file: several slides
//! share one file in a single-file deck, and a deck kept one slide per file
//! still has a separator, frontmatter and notes that shift the numbering.
//!
//! ## Fast enough to type on a whim
//!
//! Two things make that true.
//!
//! **Only decks are read.** [`crate::project`] finds the deck inside a project
//! and never descends into `node_modules`, build output or dot directories. One
//! package tree holds more Markdown than every deck on the machine.
//!
//! **The parse waits for a hit.** Reading a file and scanning its lines is
//! cheap; parsing it into a deck is the expensive half, and only a deck with a
//! match in it needs slide numbers. A search that matches nothing therefore
//! parses nothing.
//!
//! ## Matching
//!
//! Plain text, anywhere in a line. There is no pattern syntax, which is a
//! decision rather than a gap: a regular expression engine is a dependency in a
//! binary people are asked to pipe into a shell, and `slidx grep "note("`
//! should find that line rather than fail on an unclosed group.
//!
//! Case is smart — a query in all lowercase matches either case, a query with a
//! capital in it is matched exactly. That is what makes `Vue` find the
//! framework and not `revue`, without a flag nobody would remember to pass.

use std::path::{Path, PathBuf};

use serde::Serialize;
use slidx_core::{parse_deck, Deck, DeckParseOptions};

use crate::args::Matches;
use crate::home::Home;
use crate::index::{Entry, Index};
use crate::lint::source;
use crate::project;
use crate::style::{self, Ink, Style};
use crate::{Outcome, FOUND, OK};

/// How many matches are printed before the search stops.
///
/// A query like `the` matches thousands of lines across a few hundred decks,
/// and a screen of them is already more than anybody reads. Stopping is also
/// what keeps the search cheap: the files after the limit are never opened.
const DEFAULT_LIMIT: usize = 100;

/// Past this a matching line is cut short.
///
/// Six columns of indent, and two more for the ellipses that mark each cut end
/// — so a clipped line still fits the fixed width rather than being the one
/// thing in a report that wraps.
const EXCERPT_WIDTH: usize = style::WIDTH - 8;

/// One line of one slide of one deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    /// The project directory, which is what `slidx cd` takes.
    pub project: PathBuf,
    /// What to call the deck: its title, or the directory it is in.
    pub deck: String,
    /// One-based, the way a speaker counts slides.
    pub slide: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide_title: Option<String>,
    /// The slug in the deck's URL, so a hit is a link.
    pub slide_id: String,
    /// One-based line in the deck source. For an editor, not for a person.
    pub line: usize,
    pub text: String,
}

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let Some(query) = matches.first_positional().filter(|query| !query.is_empty()) else {
        return Outcome::misuse(needs_something_to_find());
    };

    let limit = match matches.value("limit").map(str::parse::<usize>) {
        Some(Ok(limit)) if limit > 0 => limit,
        Some(_) => return Outcome::misuse(bad_limit(matches.value("limit").unwrap_or_default())),
        None => DEFAULT_LIMIT,
    };

    let home = Home::discover();
    let index = Index::load(&home.index()).pruned();

    // Cleaned while it is open, which is when the stat is paid for anyway.
    let _ = index.save(&home.index());

    if index.is_empty() {
        return Outcome::misuse(crate::find::nothing_indexed());
    }

    let hits = search(&index, home.root(), query, limit);

    if matches.is_set("json") {
        return match serde_json::to_string_pretty(&hits) {
            Ok(json) => Outcome::out(format!("{json}\n")).with_code(code(&hits)),
            Err(error) => Outcome::misuse(format!("could not serialise the matches: {error}\n")),
        };
    }

    if hits.is_empty() {
        return Outcome {
            stderr: no_match(query, index.entries().len()),
            code: FOUND,
            ..Outcome::default()
        };
    }

    Outcome::out(render(&hits, query, limit, style)).with_code(OK)
}

/// Every match, in index order — most recently seen project first.
///
/// `slidx_home` is excluded rather than filtered afterwards: archived projects
/// live under it, and an archive is a place work was put *away*, so a search
/// that surfaced it would undo the filing. Installed slidx versions are under
/// there too, and their own decks are nobody's talk.
pub fn search(index: &Index, slidx_home: &Path, query: &str, limit: usize) -> Vec<Hit> {
    let sensitive = query.chars().any(char::is_uppercase);
    let mut hits = Vec::new();

    for entry in index.live().filter(|entry| !entry.path.starts_with(slidx_home)) {
        for deck in project::decks(&entry.path) {
            if hits.len() >= limit {
                return hits;
            }

            hits.extend(in_deck(entry, &deck, query, sensitive, limit - hits.len()));
        }
    }

    hits
}

/// The matches in one deck, with the slide each one is on.
///
/// The read and the scan happen first and the parse only if something matched.
/// That ordering is the difference between a search somebody types and one they
/// schedule.
fn in_deck(entry: &Entry, path: &Path, query: &str, sensitive: bool, room: usize) -> Vec<Hit> {
    let Ok(deck_source) = source::read(path, &DeckParseOptions::default().separator) else {
        return Vec::new();
    };

    let lines: Vec<(usize, &str)> = deck_source
        .source
        .lines()
        .enumerate()
        .filter(|(_, line)| contains(line, query, sensitive))
        .take(room)
        .collect();

    if lines.is_empty() {
        return Vec::new();
    }

    let deck = parse_deck(&deck_source.source, &DeckParseOptions::default());
    let label = deck
        .meta
        .title
        .clone()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| entry.label());

    lines
        .into_iter()
        .map(|(index, text)| {
            let slide = slide_at(&deck, index + 1);

            Hit {
                project: entry.path.clone(),
                deck: label.clone(),
                slide: slide.map(|slide| slide.index as usize + 1).unwrap_or(1),
                slide_title: slide.and_then(|slide| slide.title.clone()),
                slide_id: slide.map(|slide| slide.id.clone()).unwrap_or_default(),
                line: index + 1,
                text: text.trim().to_string(),
            }
        })
        .collect()
}

/// The slide a line of the deck source belongs to.
///
/// The last slide that starts at or before the line. A slide records the line
/// it starts on, so this needs no second model of where slides are — and a
/// match in the deck's own frontmatter, which is above every slide, lands on
/// the first one.
fn slide_at(deck: &Deck, line: usize) -> Option<&slidx_core::Slide> {
    deck.slides
        .iter()
        .rev()
        .find(|slide| slide.source_line as usize <= line)
        .or_else(|| deck.slides.first())
}

fn contains(line: &str, query: &str, sensitive: bool) -> bool {
    if sensitive {
        return line.contains(query);
    }

    line.to_lowercase().contains(&query.to_lowercase())
}

fn code(hits: &[Hit]) -> u8 {
    // The convention every other search tool follows: nothing found is a 1, so
    // `slidx grep x || echo none` works.
    if hits.is_empty() {
        FOUND
    } else {
        OK
    }
}

/// The matches, grouped under the deck they are in.
fn render(hits: &[Hit], query: &str, limit: usize, style: &Style) -> String {
    let mut text = String::new();
    let mut deck: Option<&str> = None;

    for hit in hits {
        if deck != Some(&hit.deck) {
            if deck.is_some() {
                text.push('\n');
            }
            text.push_str(&format!("  {}\n", style.paint(Ink::Strong, &hit.deck)));
            deck = Some(&hit.deck);
        }

        let title =
            hit.slide_title.as_deref().map(|title| format!("  {title}")).unwrap_or_default();

        text.push_str(&format!(
            "    {}{}\n      {}\n",
            style.paint(Ink::Warn, format!("slide {}", hit.slide)),
            style.paint(Ink::Faint, title),
            excerpt(&hit.text, query, style)
        ));
    }

    text.push_str(&format!(
        "\n  {}\n",
        style.paint(
            Ink::Faint,
            match hits.len() >= limit {
                // Said rather than implied: a truncated search that looked
                // complete would have somebody concluding a phrase is nowhere
                // in their decks.
                true => format!(
                    "the first {limit} matches. `--limit {}` for more.",
                    limit.saturating_mul(2)
                ),
                false => format!(
                    "{} {} in {} {}.",
                    hits.len(),
                    if hits.len() == 1 { "match" } else { "matches" },
                    decks(hits),
                    if decks(hits) == 1 { "deck" } else { "decks" }
                ),
            }
        )
    ));

    text
}

fn decks(hits: &[Hit]) -> usize {
    let mut names: Vec<&str> = hits.iter().map(|hit| hit.deck.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    names.len()
}

/// The matching line, cut to the terminal with the match still in it.
///
/// A long line clipped from the start can lose the very thing that was searched
/// for, which turns a hit into a row somebody has to take on trust. So the
/// window follows the match, and an ellipsis marks each end that was cut.
fn excerpt(text: &str, query: &str, style: &Style) -> String {
    if style::width::of(text) <= EXCERPT_WIDTH {
        return style.paint(Ink::Strong, text);
    }

    let characters: Vec<char> = text.chars().collect();
    let at = position(text, query).unwrap_or(0);
    // A few words of lead-in, so the match is not flush against the ellipsis.
    //
    // The window is counted in characters and then measured in cells, because a
    // line of Japanese fills the excerpt in half as many characters — and one
    // cut to the character count runs past the right-hand edge.
    let room = budget(&characters);
    let start = at.saturating_sub(16).min(characters.len().saturating_sub(room));
    let end = (start + room).min(characters.len());

    let head = if start > 0 { "…" } else { "" };
    let tail = if end < characters.len() { "…" } else { "" };
    let window: String = characters[start..end].iter().collect();

    format!("{head}{}{tail}", style.paint(Ink::Strong, window.trim()))
}

/// How many characters of this line fit in the excerpt's cells.
///
/// The widest character in the line decides, so a line mixing scripts is cut by
/// its own worst case rather than by an average that would overrun on the row
/// where it mattered.
fn budget(characters: &[char]) -> usize {
    let widest = characters.iter().copied().any(style::width::is_wide);

    if widest {
        EXCERPT_WIDTH / 2
    } else {
        EXCERPT_WIDTH
    }
}

/// Where the match starts, in characters — the unit the excerpt is measured in.
fn position(text: &str, query: &str) -> Option<usize> {
    let haystack = text.to_lowercase();
    let byte = haystack.find(&query.to_lowercase())?;

    Some(text[..byte].chars().count())
}

fn needs_something_to_find() -> String {
    "`slidx grep` needs something to search for.\n\n\
     It is plain text rather than a pattern, so nothing has to be escaped:\n\n\
     \x20 slidx grep \"the venue wifi\"\n"
        .to_string()
}

fn bad_limit(given: &str) -> String {
    format!("`--limit {given}` is not a number of matches.\n\nTry: slidx grep <text> --limit 20\n")
}

fn no_match(query: &str, projects: usize) -> String {
    format!(
        "Nothing in {projects} {} matches `{query}`.\n\n\
         Matching is plain text, and a capital letter makes it exact — `{query}` in\n\
         lowercase would match either case.\n",
        if projects == 1 { "deck" } else { "decks" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-grep-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        /// A project with one deck in it, laid out the conventional way.
        fn project(&self, name: &str, slides: &[&str]) -> PathBuf {
            let root = self.0.join(name);
            fs::create_dir_all(root.join("slides")).expect("slides");

            for (index, slide) in slides.iter().enumerate() {
                fs::write(root.join(format!("slides/{:04}.md", index + 1)), slide).expect("write");
            }

            root
        }

        fn write(&self, relative: &str, body: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
            fs::write(&path, body).expect("write");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn index_over(paths: &[PathBuf]) -> Index {
        let mut index = Index::default();

        for (position, path) in paths.iter().enumerate() {
            index.record(Entry::new(path).seen_at(100 - position as u64));
        }

        index
    }

    fn found(index: &Index, query: &str) -> Vec<Hit> {
        search(index, Path::new("/nowhere/slidx-home"), query, DEFAULT_LIMIT)
    }

    #[test]
    fn a_match_is_reported_by_the_slide_it_is_on_rather_than_the_line_of_a_file() {
        // The whole reason this is not `grep -r`. A speaker knows their content
        // as "slide 7", and a line number in a joined Markdown file is an
        // address that exists nowhere they can look.
        let scratch = Scratch::new("slide-number");
        let project = scratch.project(
            "vueconf",
            &[
                "---\ntitle: Making decks fast\n---\n\n# Making decks fast\n",
                "# What goes wrong\n\nthe venue wifi is down\n",
                "# The fix\n",
            ],
        );

        let hits = found(&index_over(&[project]), "venue wifi");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].slide, 2);
        assert_eq!(hits[0].slide_title.as_deref(), Some("What goes wrong"));
        assert_eq!(hits[0].deck, "Making decks fast");
        assert_eq!(hits[0].text, "the venue wifi is down");
    }

    #[test]
    fn a_match_in_the_last_slide_of_a_long_deck_is_numbered_from_one() {
        // Off-by-one here would be invisible in a small deck and wrong in every
        // real one.
        let scratch = Scratch::new("last-slide");
        let slides: Vec<String> = (1..=9).map(|n| format!("# Slide {n}\n\nbody {n}\n")).collect();
        let refs: Vec<&str> = slides.iter().map(String::as_str).collect();
        let project = scratch.project("deck", &refs);

        let hits = found(&index_over(&[project]), "body 9");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].slide, 9);
    }

    #[test]
    fn a_single_file_deck_reports_the_slide_the_match_is_on_and_not_the_file() {
        // Several slides in one file is exactly the case a line number cannot
        // answer, and the case a pasted draft is always in.
        let scratch = Scratch::new("single-file");
        scratch.write("draft/talk.md", "# One\n\nfirst\n\n---\n\n# Two\n\nsecond thing\n");
        let project = scratch.0.join("draft");

        let hits = found(&index_over(&[project]), "second thing");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].slide, 2);
    }

    #[test]
    fn every_project_in_the_index_is_searched() {
        let scratch = Scratch::new("across");
        let one = scratch.project("one", &["# One\n\na shared phrase\n"]);
        let two = scratch.project("two", &["# Two\n\nalso a shared phrase\n"]);

        let hits = found(&index_over(&[one, two]), "shared phrase");

        assert_eq!(hits.len(), 2);
        assert_ne!(hits[0].project, hits[1].project);
    }

    #[test]
    fn a_lowercase_query_matches_either_case_and_a_capital_makes_it_exact() {
        // Smart case, so the common search needs no flag and the precise one
        // needs no escaping.
        let scratch = Scratch::new("case");
        let project = scratch.project("deck", &["# One\n\nVue and revue\n"]);
        let index = index_over(&[project]);

        assert_eq!(found(&index, "vue").len(), 1);
        assert_eq!(found(&index, "Vue").len(), 1);
        assert_eq!(found(&index, "REVUE").len(), 0);
    }

    #[test]
    fn nothing_under_the_slidx_home_is_searched() {
        // Archived projects live there, and an archive is where work was put
        // away. A search that surfaced it would undo the filing.
        let scratch = Scratch::new("archive");
        let archived = scratch.project("archive/vueconf", &["# One\n\nthe archived line\n"]);

        let hits = search(&index_over(&[archived]), &scratch.0, "archived line", DEFAULT_LIMIT);

        assert!(hits.is_empty());
    }

    #[test]
    fn nothing_in_node_modules_is_searched_however_much_markdown_is_in_there() {
        let scratch = Scratch::new("modules");
        let project = scratch.project("deck", &["# One\n\nthe real line\n"]);
        scratch.write("deck/node_modules/thing/slides/0001.md", "# Fake\n\nthe real line\n");

        assert_eq!(found(&index_over(&[project]), "the real line").len(), 1);
    }

    #[test]
    fn a_project_that_has_been_deleted_is_skipped_rather_than_failing_the_search() {
        // The index can name a directory that is gone, and one dead entry must
        // not take the whole search with it.
        let scratch = Scratch::new("gone");
        let project = scratch.project("here", &["# One\n\nfindable\n"]);
        let index = index_over(&[project, PathBuf::from("/nowhere/at/all")]);

        assert_eq!(found(&index, "findable").len(), 1);
    }

    #[test]
    fn the_search_stops_at_the_limit_and_says_that_it_did() {
        // A truncated search that looked complete would have somebody
        // concluding a phrase is nowhere in their decks.
        let scratch = Scratch::new("limit");
        let project = scratch.project("deck", &["# One\n\nrepeat\nrepeat\nrepeat\nrepeat\n"]);

        let hits = search(&index_over(&[project]), Path::new("/nowhere"), "repeat", 2);
        assert_eq!(hits.len(), 2);

        let text = render(&hits, "repeat", 2, &Style::plain());
        assert!(text.contains("the first 2 matches"), "{text}");
        assert!(text.contains("--limit 4"), "{text}");
    }

    #[test]
    fn matches_are_grouped_under_the_deck_they_are_in() {
        // Otherwise a screen of hits is a screen of rows all beginning with the
        // same deck name, and the eye has nowhere to rest.
        let scratch = Scratch::new("grouped");
        let one =
            scratch.project("one", &["---\ntitle: First deck\n---\n\n# One\n\nphrase\nphrase\n"]);
        let two = scratch.project("two", &["---\ntitle: Second deck\n---\n\n# Two\n\nphrase\n"]);

        let text =
            render(&found(&index_over(&[one, two]), "phrase"), "phrase", 100, &Style::plain());

        assert_eq!(text.matches("First deck").count(), 1, "{text}");
        assert_eq!(text.matches("Second deck").count(), 1, "{text}");
        assert!(text.contains("3 matches in 2 decks"), "{text}");
    }

    #[test]
    fn nothing_a_search_prints_runs_past_the_fixed_width() {
        let hit = Hit {
            project: PathBuf::from("/talks/a"),
            deck: "A deck".into(),
            slide: 3,
            slide_title: Some("A slide with a fairly long title on it".into()),
            slide_id: "a-slide".into(),
            line: 12,
            text: "the venue wifi is down and the deck's fonts were on a content delivery network somewhere far away".into(),
        };

        for line in render(&[hit], "fonts", 100, &Style::plain()).lines() {
            assert!(
                line.chars().count() <= style::WIDTH,
                "{} columns: {line}",
                line.chars().count()
            );
        }
    }

    #[test]
    fn a_long_line_is_cut_around_the_match_rather_than_from_the_start() {
        // Clipping from the start can lose the very thing that was searched
        // for, which leaves a row nobody can check.
        let text = format!("{}the needle{}", "lead ".repeat(30), " trailing".repeat(10));
        let shown = excerpt(&text, "needle", &Style::plain());

        assert!(shown.contains("needle"), "{shown}");
        assert!(shown.starts_with('…'), "{shown}");
        assert!(shown.chars().count() <= EXCERPT_WIDTH + 2, "{shown}");
    }

    #[test]
    fn a_long_japanese_line_is_cut_to_the_cells_it_will_occupy() {
        // A line of Japanese fills the excerpt in half as many characters. Cut
        // to the character count it runs off the right-hand side of the window,
        // and the slide it belongs to scrolls off the top.
        let text = format!("{}探すもの{}", "前置き".repeat(20), "あと".repeat(20));
        let shown = excerpt(&text, "探すもの", &Style::plain());

        assert!(shown.contains("探すもの"), "{shown}");
        assert!(
            style::width::of(&shown) <= EXCERPT_WIDTH + 2,
            "{} cells: {shown}",
            style::width::of(&shown)
        );
    }

    #[test]
    fn a_line_that_fits_is_shown_as_it_was_written() {
        assert_eq!(
            excerpt("the venue wifi is down", "venue", &Style::plain()),
            "the venue wifi is down"
        );
    }

    #[test]
    fn no_match_exits_one_so_a_shell_can_branch_on_it() {
        // The convention every search tool follows: `slidx grep x || echo none`.
        assert_eq!(code(&[]), FOUND);
    }

    #[test]
    fn an_empty_query_is_a_misuse_rather_than_every_line_of_every_deck() {
        assert!(needs_something_to_find().contains("slidx grep"));
    }

    #[test]
    fn json_carries_the_project_path_so_a_hit_can_be_opened() {
        let scratch = Scratch::new("json");
        let project = scratch.project("deck", &["---\ntitle: A deck\n---\n\n# One\n\nfindable\n"]);
        let hits = found(&index_over(&[project]), "findable");

        let json = serde_json::to_string(&hits).expect("json");

        assert!(json.contains("\"project\""), "{json}");
        assert!(json.contains("\"slide\":1"), "{json}");
        assert!(json.contains("\"slideId\""), "{json}");
        assert!(json.contains("\"line\":"), "{json}");
    }
}
