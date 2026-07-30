//! Where the decks are inside a project.
//!
//! The index remembers *projects* — `~/talks/vueconf`, the directory with the
//! git repository and the vite config in it — because that is the directory
//! somebody wants to open a year later. Every command that works across
//! projects then has the same question to answer: given that directory, which
//! files are the deck?
//!
//! Answered once here, because two answers is two ideas of what a deck is, and
//! then `slidx list` counts slides in one place and `slidx grep` searches
//! another.
//!
//! ## The two shapes
//!
//! `<project>/slides` is the conventional layout — the plugin's default
//! `srcDir`, one file per slide — and it is looked for first. A project without
//! one keeps its deck as a single Markdown file at the top level, which is what
//! a pasted draft looks like before anybody split it up.
//!
//! Nested projects exist: a monorepo with a talk in `packages/keynote` is a
//! reasonable thing to have, so the search descends. It stops at
//! [`MAX_DEPTH`] and skips [`IGNORED`] — `node_modules` alone holds thousands
//! of Markdown files, none of them anybody's slides, and a search that reads
//! them is a search nobody types twice.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::lint::source::{DEFAULT_DIR, SLIDE_EXTENSION};

/// Directory names never descended into.
///
/// Package trees, build output, and git's own storage. Each holds Markdown that
/// looks like a deck to a text search and is nobody's slides — a `node_modules`
/// with a hundred packages in it can hold more `.md` files than every deck on
/// the machine put together.
pub const IGNORED: &[&str] = &["node_modules", "target", "dist", ".git", ".svn", "vendor"];

/// How far below a project a deck is looked for.
///
/// A talk inside a monorepo is usually `packages/<name>/slides` or
/// `apps/<name>/slides`, which is three. Four leaves room for one more level of
/// grouping and stops a search from walking somebody's entire home directory
/// because they indexed a project at the top of it.
pub const MAX_DEPTH: usize = 4;

/// Every deck source in a project, nearest the top first.
///
/// A path here is what [`crate::lint::source::read`] takes: a directory of
/// slide files, or a single file holding the whole deck.
pub fn decks(project: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut level = vec![project.to_path_buf()];

    for _ in 0..MAX_DEPTH {
        let mut next = Vec::new();

        for directory in level {
            let (slides, others) = split_children(&directory);

            match slides {
                // A directory with slides in it is a deck, and its own
                // subdirectories are assets rather than more decks.
                Some(slides) => found.push(slides),
                None => found.extend(single_file_decks(&directory)),
            }

            next.extend(others);
        }

        if next.is_empty() {
            break;
        }

        level = next;
    }

    found
}

/// The one deck a project is mostly about.
///
/// The shallowest, which for every layout anybody uses is the deck at the top
/// of the project. A list has one row per project and needs one answer.
pub fn primary_deck(project: &Path) -> Option<PathBuf> {
    decks(project).into_iter().next()
}

/// When any of these files was last written, in unix seconds.
///
/// The newest of them, which is what "last touched" means for a deck kept as
/// one file per slide: editing slide nine is working on the deck.
pub fn touched(files: &[PathBuf]) -> Option<u64> {
    files
        .iter()
        .filter_map(|file| fs::metadata(file).ok())
        .filter_map(|metadata| metadata.modified().ok())
        .filter_map(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|since| since.as_secs())
        .max()
}

/// A project's `slides` directory, and the subdirectories worth descending.
fn split_children(directory: &Path) -> (Option<PathBuf>, Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return (None, Vec::new());
    };

    let mut slides = None;
    let mut others = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();

        if !entry.path().is_dir() || skipped(&name) {
            continue;
        }

        if name == DEFAULT_DIR {
            slides = Some(entry.path());
        } else {
            others.push(entry.path());
        }
    }

    // Sorted, so two runs of a search report the same decks in the same order.
    others.sort();

    (slides, others)
}

/// Markdown files sitting directly in a directory, each its own deck.
fn single_file_decks(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_lowercase())
                .is_some_and(|name| name.ends_with(SLIDE_EXTENSION) && !skipped(&name))
        })
        .collect();

    files.sort();
    files
}

/// True for a name nothing descends into or reads.
///
/// Dotfiles go too: `.github` holds workflow Markdown, and a dot-prefixed
/// directory is by convention something a tool put there rather than something
/// a person wrote.
fn skipped(name: &str) -> bool {
    name.starts_with('.') || IGNORED.contains(&name)
}

/// When `seconds` ago was, said the way somebody would say it.
///
/// Relative rather than a date, because the question a list answers is "which
/// of these did I touch recently", and answering it from `2026-03-14` means
/// doing arithmetic in your head against today. Coarse on purpose: nobody needs
/// to know a deck was edited 37 minutes ago rather than an hour.
pub fn ago(seconds: u64, now: u64) -> String {
    let elapsed = now.saturating_sub(seconds);

    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    // A month as the average length of one, so twelve of them are a year.
    const MONTH: u64 = 30 * DAY + 10 * HOUR + 30 * MINUTE;
    const YEAR: u64 = 12 * MONTH;

    match elapsed {
        0..MINUTE => "just now".to_string(),
        seconds if seconds < HOUR => format!("{}m ago", seconds / MINUTE),
        seconds if seconds < DAY => format!("{}h ago", seconds / HOUR),
        seconds if seconds < MONTH => format!("{}d ago", seconds / DAY),
        seconds if seconds < YEAR => format!("{}mo ago", seconds / MONTH),
        seconds => format!("{}y ago", seconds / YEAR),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch project tree that cleans up after itself.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-project-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        fn write(&self, relative: &str, body: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().expect("a parent")).expect("directories");
            fs::write(&path, body).expect("write");
            path
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_project_laid_out_the_conventional_way_has_one_deck_in_its_slides_directory() {
        let scratch = Scratch::new("conventional");
        scratch.write("slides/0001.md", "# One");
        scratch.write("slides/0002.md", "# Two");

        assert_eq!(decks(scratch.path()), [scratch.path().join(DEFAULT_DIR)]);
    }

    #[test]
    fn a_project_with_no_slides_directory_keeps_its_deck_as_a_file_at_the_top() {
        // What a pasted draft looks like before anybody split it up.
        let scratch = Scratch::new("single");
        let file = scratch.write("talk.md", "# One\n\n---\n\n# Two\n");

        assert_eq!(decks(scratch.path()), [file]);
    }

    #[test]
    fn a_readme_beside_a_slides_directory_is_not_read_as_a_deck() {
        // Otherwise every project reports two decks, one of them its own
        // documentation, and `slidx list` counts the wrong slides.
        let scratch = Scratch::new("readme");
        scratch.write("slides/0001.md", "# One");
        scratch.write("README.md", "# How to build this");

        assert_eq!(decks(scratch.path()), [scratch.path().join(DEFAULT_DIR)]);
    }

    #[test]
    fn a_deck_nested_in_a_monorepo_is_found() {
        // A talk in `packages/keynote` is a reasonable thing to have done, and
        // the index records the repository root.
        let scratch = Scratch::new("monorepo");
        scratch.write("packages/keynote/slides/0001.md", "# One");

        assert_eq!(decks(scratch.path()), [scratch.path().join("packages/keynote/slides")]);
    }

    #[test]
    fn nothing_inside_node_modules_is_ever_read_as_a_deck() {
        // The reason a search over the index is fast enough to type. One
        // package tree holds more Markdown than every deck on the machine.
        let scratch = Scratch::new("modules");
        scratch.write("slides/0001.md", "# One");
        scratch.write("node_modules/some-package/slides/0001.md", "# Not a slide of mine");
        scratch.write("node_modules/some-package/README.md", "# Nor this");

        assert_eq!(decks(scratch.path()), [scratch.path().join(DEFAULT_DIR)]);
    }

    #[test]
    fn build_output_is_skipped_so_a_search_never_reports_a_generated_copy() {
        let scratch = Scratch::new("output");
        scratch.write("dist/slides/0001.md", "# Built");
        scratch.write("target/doc/slides/0001.md", "# Also built");
        scratch.write("slides/0001.md", "# Source");

        assert_eq!(decks(scratch.path()), [scratch.path().join(DEFAULT_DIR)]);
    }

    #[test]
    fn a_dot_directory_is_left_alone_because_a_tool_put_it_there() {
        let scratch = Scratch::new("dotfiles");
        scratch.write(".github/ISSUE_TEMPLATE.md", "# Not a deck");
        scratch.write("talk.md", "# A deck");

        assert_eq!(decks(scratch.path()), [scratch.path().join("talk.md")]);
    }

    #[test]
    fn a_deck_below_the_depth_limit_is_not_looked_for() {
        // The limit that stops a project indexed at the top of a home
        // directory from turning one search into a filesystem walk.
        let scratch = Scratch::new("deep");
        scratch.write("a/b/c/d/e/slides/0001.md", "# Too far down");

        assert!(decks(scratch.path()).is_empty());
    }

    #[test]
    fn the_primary_deck_is_the_one_nearest_the_top_of_the_project() {
        // A list has one row per project, so it needs one answer, and the deck
        // at the top of a project is the project's deck.
        let scratch = Scratch::new("primary");
        scratch.write("slides/0001.md", "# The talk");
        scratch.write("packages/other/slides/0001.md", "# Something else");

        assert_eq!(primary_deck(scratch.path()), Some(scratch.path().join(DEFAULT_DIR)));
    }

    #[test]
    fn a_project_that_is_not_there_has_no_decks_rather_than_failing() {
        // The index can name a directory that has since been deleted, and
        // every reader of it has to keep working.
        assert!(decks(Path::new("/nowhere/at/all")).is_empty());
        assert_eq!(primary_deck(Path::new("/nowhere/at/all")), None);
    }

    #[test]
    fn the_newest_slide_file_is_when_the_deck_was_last_touched() {
        // Editing slide nine is working on the deck, so the deck's time is the
        // newest of its files rather than the first one's.
        let scratch = Scratch::new("touched");
        let files = vec![scratch.write("slides/0001.md", "# One")];

        assert!(touched(&files).is_some());
        assert!(touched(&[PathBuf::from("/nowhere/at/all")]).is_none());
    }

    #[test]
    fn an_age_reads_as_a_person_would_say_it() {
        let day = 24 * 60 * 60;

        assert_eq!(ago(100, 100), "just now");
        assert_eq!(ago(0, 5 * 60), "5m ago");
        assert_eq!(ago(0, 3 * 60 * 60), "3h ago");
        assert_eq!(ago(0, 6 * day), "6d ago");
        assert_eq!(ago(0, 90 * day), "2mo ago");
        assert_eq!(ago(0, 800 * day), "2y ago");
    }

    #[test]
    fn a_deck_touched_in_the_future_reads_as_just_now_rather_than_underflowing() {
        // A file copied off a machine with a wrong clock, or an archive
        // restored with its timestamps. Nothing about a list is worth a panic.
        assert_eq!(ago(500, 100), "just now");
    }
}
