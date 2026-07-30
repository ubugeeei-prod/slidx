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
//! So the rule is written down rather than inferred, and it is exactly four
//! sentences long:
//!
//! 1. Files sort by name — which is why the convention is `0001.md`, and why
//!    inserting a slide is a rename rather than a renumbering.
//! 2. Each file is trimmed and joined with the deck separator alone on its own
//!    line, with a blank line on each side of it.
//! 3. A file that already opens with a separator supplies its own, and gets a
//!    blank line rather than a second one.
//! 4. The first file's frontmatter is the deck's, so it stays at the very
//!    start.
//!
//! Sentences 2 and 3 are the ones that used to be wrong here. The separator was
//! written with no blank line after it, and a file opening with its own
//! frontmatter block got a separator anyway — so `---` immediately followed by a
//! slide's body, and then by another `---`, read back as *that slide's
//! frontmatter*. The whole slide, its notes included, disappeared from the deck
//! the linter saw, while the build read three slides and printed them. That is
//! the failure this file exists to prevent, and it was reachable through
//! `slidx add --notes`.
//!
//! A single file holding several slides is read as-is. Small decks and pasted
//! drafts should not need a directory.

use std::fs;
use std::path::{Path, PathBuf};

use slidx_core::ByteSpan;

/// What the vite plugin calls `extensions`, at its default.
pub const SLIDE_EXTENSION: &str = ".md";

/// Where `slidx lint` looks when it is given no path.
///
/// The plugin's default `srcDir`. Typing `slidx lint` in a project laid out the
/// standard way has to lint that project, or the command is only useful to
/// people who already know the layout.
pub const DEFAULT_DIR: &str = "slides";

/// One slide file, and where its bytes ended up in the joined source.
///
/// Joining trims each file, so the two coordinate systems differ by exactly that
/// trim. Recording it is what lets `slidx i18n apply` rewrite the file a byte
/// came from rather than the joined text nobody has on disk — and it is why a
/// translation that changes nothing writes files back byte for byte instead of
/// quietly normalising everybody's trailing newline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideFile {
    pub path: PathBuf,
    /// The file's contribution to the joined source.
    pub joined: ByteSpan,
    /// Where that contribution starts in the file itself, past the leading
    /// whitespace the join removed.
    pub offset: usize,
}

/// A deck, assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckSource {
    /// What to call this deck in a message. A path as the user typed it.
    pub label: String,
    /// The files it came from, in deck order. Empty for a single-file deck.
    pub files: Vec<SlideFile>,
    /// The joined source, as the parser will see it.
    pub source: String,
}

impl DeckSource {
    /// How many files were read. One, for a single-file deck.
    pub fn file_count(&self) -> usize {
        self.files.len().max(1)
    }

    /// The file an offset in the joined source belongs to, and the offset within
    /// it.
    ///
    /// `None` for a single-file deck, whose source is its file untouched, and for
    /// an offset that fell in a separator this module inserted rather than in any
    /// file's own bytes.
    pub fn locate(&self, offset: usize) -> Option<(&SlideFile, usize)> {
        let file = self
            .files
            .iter()
            .find(|file| file.joined.start <= offset && offset <= file.joined.end)?;

        Some((file, offset - file.joined.start + file.offset))
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
        sources.push(source);
    }

    let (source, spans) = join_tracking(&sources, separator);
    let parts = files
        .into_iter()
        .zip(&sources)
        .zip(spans)
        .map(|((path, text), joined)| SlideFile {
            path,
            joined,
            offset: text.len() - text.trim_start().len(),
        })
        .collect();

    Ok(DeckSource { label, files: parts, source })
}

/// Joins slide sources into one deck, as rules 2 and 3 above state it.
///
/// Public because the joining is the rule, and a second caller has appeared:
/// `slidx save` reads a deck out of a git commit to compare it with the one on
/// disk, and a deck assembled two different ways would diff against itself.
///
/// The shape is `joinDeck` in `packages/vite-plugin/src/files.ts`, line for
/// line. It is not the obvious `sources.join(separator)` because of the two
/// cases the obvious one gets wrong: a separator with no blank line under it
/// opens what the parser reads as a frontmatter block, and a file that already
/// starts with a separator does not need one in front of it.
pub fn join(sources: &[String], separator: &str) -> String {
    join_tracking(sources, separator).0
}

/// The same, saying where each source's bytes landed.
///
/// One span per input, empty for a source that contributed nothing. `slidx i18n
/// apply` writes a translated deck back one file at a time and has to know which
/// file each byte came from — and the seams are exactly what [`join`] decides,
/// so a caller computing them again would be a second answer to a rule that has
/// already been wrong here once.
pub fn join_tracking(sources: &[String], separator: &str) -> (String, Vec<ByteSpan>) {
    let mut joined = String::new();
    let mut spans = Vec::with_capacity(sources.len());

    for source in sources.iter().map(|source| source.trim()) {
        if source.is_empty() {
            spans.push(ByteSpan::empty(joined.len()));
            continue;
        }

        if !joined.is_empty() {
            joined.push_str(&if opens_with_separator(source, separator) {
                "\n\n".to_string()
            } else {
                format!("\n\n{separator}\n\n")
            });
        }

        spans.push(ByteSpan::new(joined.len(), joined.len() + source.len()));
        joined.push_str(source);
    }

    (joined, spans)
}

/// True when a file's first line is the deck separator and nothing else.
///
/// Such a file carries its own separator — usually as the opening delimiter of
/// its slide's frontmatter block — and putting another one in front of it would
/// leave an empty slide between the two.
fn opens_with_separator(source: &str, separator: &str) -> bool {
    source.lines().next().map(str::trim_end) == Some(separator)
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
    use slidx_core::{parse_deck, DeckParseOptions};
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
    fn slide_files_are_joined_with_the_separator_alone_between_blank_lines() {
        // A separator with anything else on its line is not a slide break, and
        // one with a slide's body on the line under it opens what the parser
        // reads as that slide's frontmatter. The joining has to produce exactly
        // this shape, which is the shape the plugin's `joinDeck` produces.
        let scratch = Scratch::new("join");
        scratch.write("0001.md", "# One\n");
        scratch.write("0002.md", "# Two\n");

        assert_eq!(read(scratch.path(), "---").expect("a deck").source, "# One\n\n---\n\n# Two");
    }

    #[test]
    fn a_slide_that_ends_with_its_notes_survives_being_joined_to_the_next_one() {
        // The failure this rule exists to prevent, reached the way it was
        // actually reached: `slidx add --notes` writes a slide whose body ends
        // with a note comment. Joined with a separator and no blank line under
        // it, the parser read `---`, a body, and the next `---` as one
        // frontmatter block — and the slide, its notes included, was gone from
        // the deck the linter saw while the build printed it.
        let scratch = Scratch::new("notes");
        scratch.write("0001.md", "# One\n");
        scratch.write("0002.md", "# Two\n\n<!-- notes: the wifi story -->\n");
        scratch.write("0003.md", "# Three\n");

        let deck = parse_deck(
            &read(scratch.path(), "---").expect("a deck").source,
            &DeckParseOptions::default(),
        );

        assert_eq!(
            deck.slides.iter().map(|slide| slide.display_title()).collect::<Vec<_>>(),
            ["One", "Two", "Three"]
        );
        assert_eq!(deck.slides[1].notes_text(), "the wifi story");
    }

    #[test]
    fn a_file_that_opens_with_its_own_separator_does_not_get_a_second_one() {
        // A slide file whose first line is `---` is opening its own frontmatter
        // block. Another separator in front of it leaves an empty slide between
        // the two, and the deck gains a blank page the build never had.
        let scratch = Scratch::new("own-separator");
        scratch.write("0001.md", "# One\n");
        scratch.write("0002.md", "---\nbudget: 90s\n---\n\n# Two\n");

        let source = read(scratch.path(), "---").expect("a deck").source;
        let deck = parse_deck(&source, &DeckParseOptions::default());

        assert_eq!(source, "# One\n\n---\nbudget: 90s\n---\n\n# Two");
        assert_eq!(deck.slides.len(), 2, "{source:?}");
        assert_eq!(deck.slides[1].budget_seconds, Some(90));
    }

    #[test]
    fn an_empty_slide_file_adds_no_separator_of_its_own() {
        // A file somebody emptied rather than deleted. Joining it as a slide
        // would put a blank page in the middle of the talk.
        let scratch = Scratch::new("empty-file");
        scratch.write("0001.md", "# One\n");
        scratch.write("0002.md", "\n\n");
        scratch.write("0003.md", "# Three\n");

        let source = read(scratch.path(), "---").expect("a deck").source;

        assert_eq!(source, "# One\n\n---\n\n# Three");
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
    fn every_byte_of_the_joined_source_maps_back_to_the_file_it_came_from() {
        // What `slidx i18n apply` writes through. A mapping that was off by the
        // whitespace the join trimmed would translate the wrong bytes of a file.
        let scratch = Scratch::new("locate");
        scratch.write("0001.md", "\n\n# One\n\n");
        scratch.write("0002.md", "# Two\n\nBody.\n");

        let deck = read(scratch.path(), "---").expect("a deck");

        for (index, file) in deck.files.iter().enumerate() {
            let text = fs::read_to_string(&file.path).expect("read back");
            let (found, at) = deck.locate(file.joined.start).expect("a file");

            assert_eq!(found.path, file.path, "file {index}");
            assert_eq!(&text[at..at + file.joined.len()], file.joined.slice(&deck.source));
        }
    }

    #[test]
    fn a_single_file_deck_maps_to_no_slide_file_because_it_needs_no_mapping() {
        let scratch = Scratch::new("locate-single");
        let file = scratch.write("talk.md", "# One\n");

        assert!(read(&file, "---").expect("a deck").locate(0).is_none());
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
