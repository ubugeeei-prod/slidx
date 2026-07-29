//! Reading a deck off disk the way the build reads it.
//!
//! This is the one rule in the CLI that is also stated somewhere else:
//! `packages/vite-plugin/src/deck.ts` joins slide files for `vite build`, and
//! this joins them for `slidx lint`. Two implementations of one rule is
//! normally a bug, and it is only tolerable here because the two live in
//! different languages on opposite sides of the wasm boundary.
//!
//! What is *not* tolerable is the two disagreeing, because then the linter and
//! the build are looking at different decks and a green CI run means nothing.
//! So the rule is written down rather than inferred, and it is exactly three
//! sentences long:
//!
//! 1. Files sort by name — which is why the convention is `0001.md`, and why
//!    inserting a slide is a rename rather than a renumbering.
//! 2. Each file is trimmed and joined with the deck separator on its own line.
//! 3. The first file's frontmatter is the deck's, so it stays at the very
//!    start.
//!
//! A single file holding several slides is read as-is. Small decks and pasted
//! drafts should not need a directory.

use std::fs;
use std::path::{Path, PathBuf};

/// What the vite plugin calls `extensions`, at its default.
pub const SLIDE_EXTENSION: &str = ".md";

/// Where `slidx lint` looks when it is given no path.
///
/// The plugin's default `srcDir`. Typing `slidx lint` in a project laid out the
/// standard way has to lint that project, or the command is only useful to
/// people who already know the layout.
pub const DEFAULT_DIR: &str = "slides";

/// A deck, assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckSource {
    /// What to call this deck in a message. A path as the user typed it.
    pub label: String,
    /// The files it came from, in deck order. Empty for a single-file deck.
    pub files: Vec<PathBuf>,
    /// The joined source, as the parser will see it.
    pub source: String,
}

impl DeckSource {
    /// How many files were read. One, for a single-file deck.
    pub fn file_count(&self) -> usize {
        self.files.len().max(1)
    }
}

/// Reads a deck from a file or a directory of slide files.
///
/// The error is addressed to the person who typed the path: it says what was
/// looked for and where, because "no such file or directory" leaves somebody
/// guessing whether they got the path wrong or the layout wrong.
pub fn read(path: &Path, separator: &str) -> Result<DeckSource, String> {
    let label = path.display().to_string();

    let metadata = fs::metadata(path).map_err(|_| missing(path))?;

    if metadata.is_file() {
        let source = fs::read_to_string(path).map_err(|error| unreadable(path, &error))?;
        return Ok(DeckSource { label, files: Vec::new(), source });
    }

    let files = slide_files(path)?;

    if files.is_empty() {
        return Err(format!(
            "No {SLIDE_EXTENSION} files in {label}.\n\n\
             A deck is one file per slide, named so they sort: {label}/0001{SLIDE_EXTENSION},\n\
             {label}/0002{SLIDE_EXTENSION}, and so on. A single file holding the whole deck\n\
             works too — pass it directly."
        ));
    }

    let mut sources = Vec::with_capacity(files.len());
    for file in &files {
        let source = fs::read_to_string(file).map_err(|error| unreadable(file, &error))?;
        sources.push(source.trim().to_string());
    }

    Ok(DeckSource { label, files, source: sources.join(&format!("\n\n{separator}\n")) })
}

/// Slide files in one directory, in deck order.
///
/// Sorted by the raw file name. The plugin sorts with `localeCompare(_, "en")`,
/// which differs from a byte sort only for names outside ASCII — and a slide
/// file named to sort is named in ASCII by construction. Anything else is
/// already relying on an ordering nobody can see.
fn slide_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(directory).map_err(|error| unreadable(directory, &error))?;

    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| unreadable(directory, &error))?;

        // Symlinked slide files are followed on purpose: a shared title slide
        // linked into several decks is a reasonable thing to have done.
        if !entry.path().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_lowercase().ends_with(SLIDE_EXTENSION) {
            names.push(name);
        }
    }

    names.sort_unstable();

    Ok(names.into_iter().map(|name| directory.join(name)).collect())
}

fn missing(path: &Path) -> String {
    format!(
        "There is nothing at {}.\n\n\
         `slidx lint` takes a deck file or a directory of slide files, and looks in\n\
         ./{DEFAULT_DIR} when given neither.",
        path.display()
    )
}

fn unreadable(path: &Path, error: &std::io::Error) -> String {
    format!("Could not read {}: {error}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A scratch directory that cleans up after itself.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-cli-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch directory");
            Self(path)
        }

        fn write(&self, name: &str, body: &str) -> PathBuf {
            let path = self.0.join(name);
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
    fn a_directory_is_read_in_file_name_order_rather_than_directory_order() {
        // The reason the convention is 0001.md. A filesystem hands entries back
        // in whatever order it likes, and a deck whose slides reorder between
        // machines is unusable.
        let scratch = Scratch::new("order");
        scratch.write("0003.md", "# Third");
        scratch.write("0001.md", "# First");
        scratch.write("0002.md", "# Second");

        let deck = read(scratch.path(), "---").expect("a deck");

        assert!(deck.source.find("First") < deck.source.find("Second"));
        assert!(deck.source.find("Second") < deck.source.find("Third"));
    }

    #[test]
    fn slide_files_are_joined_with_the_separator_on_its_own_line() {
        // A separator with anything else on its line is not a slide break, so
        // the joining has to produce exactly this shape.
        let scratch = Scratch::new("join");
        scratch.write("0001.md", "# One\n");
        scratch.write("0002.md", "# Two\n");

        assert_eq!(read(scratch.path(), "---").expect("a deck").source, "# One\n\n---\n# Two");
    }

    #[test]
    fn the_deck_separator_is_whatever_the_deck_uses() {
        let scratch = Scratch::new("separator");
        scratch.write("0001.md", "# One");
        scratch.write("0002.md", "# Two");

        assert!(read(scratch.path(), "***").expect("a deck").source.contains("\n***\n"));
    }

    #[test]
    fn the_first_files_frontmatter_stays_at_the_very_start_of_the_deck() {
        // Deck metadata is read from the top of the source. A blank line or a
        // separator ahead of it and the deck has no title, theme, or duration.
        let scratch = Scratch::new("frontmatter");
        scratch.write("0001.md", "---\ntitle: A talk\n---\n\n# One");
        scratch.write("0002.md", "# Two");

        assert!(read(scratch.path(), "---").expect("a deck").source.starts_with("---\ntitle:"));
    }

    #[test]
    fn a_single_file_deck_is_read_without_being_touched() {
        // Whatever separator convention the file already uses is its own.
        let scratch = Scratch::new("single");
        let file = scratch.write("talk.md", "# One\n\n---\n\n# Two\n");

        let deck = read(&file, "---").expect("a deck");

        assert_eq!(deck.source, "# One\n\n---\n\n# Two\n");
        assert!(deck.files.is_empty());
        assert_eq!(deck.file_count(), 1);
    }

    #[test]
    fn files_that_are_not_slides_are_left_out() {
        // Decks live next to their assets. A README or an image in the same
        // directory is not a slide and must not become one.
        let scratch = Scratch::new("mixed");
        scratch.write("0001.md", "# One");
        scratch.write("notes.txt", "not a slide");
        scratch.write("diagram.png", "not a slide either");

        let deck = read(scratch.path(), "---").expect("a deck");

        assert_eq!(deck.files.len(), 1);
        assert!(!deck.source.contains("not a slide"));
    }

    #[test]
    fn a_subdirectory_is_not_mistaken_for_a_slide() {
        let scratch = Scratch::new("nested");
        scratch.write("0001.md", "# One");
        fs::create_dir(scratch.path().join("assets.md")).expect("a directory");

        assert_eq!(read(scratch.path(), "---").expect("a deck").files.len(), 1);
    }

    #[test]
    fn a_path_that_does_not_exist_says_where_it_looked_and_what_it_wanted() {
        let message = read(Path::new("/nowhere/at/all"), "---").expect_err("no such deck");

        assert!(message.contains("/nowhere/at/all"), "{message}");
        assert!(message.contains(DEFAULT_DIR), "{message}");
    }

    #[test]
    fn a_directory_with_no_slides_explains_the_layout_rather_than_reporting_zero() {
        // The state every new project is in. "0 diagnostics" would be a green
        // run over a deck that was never found.
        let scratch = Scratch::new("empty");
        let message = read(scratch.path(), "---").expect_err("no slides");

        assert!(message.contains("0001.md"), "{message}");
    }

    #[test]
    fn the_extension_match_ignores_case() {
        // `.MD` off a case-insensitive filesystem, or a file somebody renamed
        // on Windows. Skipping it would silently drop a slide.
        let scratch = Scratch::new("case");
        scratch.write("0001.MD", "# One");

        assert_eq!(read(scratch.path(), "---").expect("a deck").files.len(), 1);
    }
}
