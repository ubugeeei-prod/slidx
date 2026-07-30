//! `slidx add` — one more slide, spliced in by [`slidx_edit`].
//!
//! ## Why this does not write Markdown
//!
//! It would be four lines: format a heading, open a file, write, done. And it
//! would be the second writer of deck Markdown in slidx, which is the one thing
//! the architecture is arranged to prevent.
//!
//! The visual editor's promise is that an edit is a **byte-range splice into
//! the file the author saved** — their blank lines, their `*` bullets and their
//! hand-wrapped paragraphs survive, because nothing ever re-serialises the
//! document. That promise is a property of there being exactly one thing that
//! writes decks. A second writer with its own idea of where a separator goes
//! and how many blank lines follow a heading does not break the property
//! loudly; it breaks it in somebody's diff, months later, in a repository where
//! both tools have been used.
//!
//! So this command computes nothing. [`EditOp::InsertSlide`] is handed the
//! position and the new slide's own body, [`EditOp::SetNotes`] writes the notes,
//! and every byte that lands on disk is a splice the edit crate produced.
//!
//! ## One slide per file, and what that costs
//!
//! A deck kept as a directory is one file per slide, sorted by name — so
//! inserting in the middle is a question about file *names*, which no edit
//! operation has an opinion about. The slide's bytes come from the splice; the
//! name is worked out here, by moving the files after it along one. A deck whose
//! files are not numbered gets a plain refusal rather than a guess, because a
//! guessed name that sorts wrong reorders somebody's talk.
//!
//! Adding a slide *before the first one* is refused for a deck kept as a
//! directory. The deck's frontmatter — its title, theme and slot — is the first
//! slide's, and it has to open the first file; a new first slide would mean
//! moving that block between files, which is a different operation and should
//! look like one.

use std::fs;
use std::path::{Path, PathBuf};

use slidx_core::{parse_deck, DeckParseOptions};
use slidx_edit::{apply, slide_spans, EditOp};

use crate::args::Matches;
use crate::home::Home;
use crate::index::{self, Entry};
use crate::lint::project_root;
use crate::lint::source::{self, DeckSource};
use crate::style::{Ink, Style};
use crate::Outcome;

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let separator =
        matches.value("separator").map(str::to_string).unwrap_or_else(|| "---".to_string());
    let path = matches
        .first_positional()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source::DEFAULT_DIR));

    let deck_source = match source::read(&path, &separator) {
        Ok(read) => read,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    let options = DeckParseOptions { separator: separator.clone(), ..DeckParseOptions::default() };
    let deck = parse_deck(&deck_source.source, &options);
    let slides = deck.slides.len();

    let at = match position(matches, slides) {
        Ok(at) => at,
        Err(message) => return Outcome::misuse(message),
    };

    let title = matches.value("title").unwrap_or("New slide");
    let edited = match splice(&deck_source.source, &options, at, title, matches.value("notes")) {
        Ok(edited) => edited,
        Err(message) => return Outcome::misuse(message),
    };

    let written = match write(&deck_source, &edited, &options, at) {
        Ok(written) => written,
        Err(message) => return Outcome::misuse(message),
    };

    // The index fills itself, so a deck a slide was just added to is a deck
    // slidx has seen. Best-effort, as everywhere — see `index::remember`.
    if let Some(root) = project_root(&path) {
        let after = parse_deck(&edited, &options);
        index::remember(&Home::discover().index(), Entry::new(root).describing(&after));
    }

    Outcome::out(report(&written, at, title, style))
}

/// Where the new slide goes, as a zero-based index.
///
/// `--at` is one-based, because that is how a speaker counts slides and how
/// every other slidx surface numbers them. `--at 3` puts the new slide third,
/// which is the reading somebody expects when they say it out loud.
fn position(matches: &Matches, slides: usize) -> Result<usize, String> {
    let Some(given) = matches.value("at") else {
        return Ok(slides);
    };

    match given.parse::<usize>() {
        // `--at <slides + 1>` is the end, which is also the default. Allowing it
        // means a script can compute a position without a special case.
        Ok(number) if (1..=slides + 1).contains(&number) => Ok(number - 1),
        Ok(_) => Err(format!(
            "`--at {given}` is outside a deck of {slides} {}.\n\n\
             Slides count from one, and {} is the end.\n",
            if slides == 1 { "slide" } else { "slides" },
            slides + 1
        )),
        Err(_) => Err(format!("`--at {given}` is not a slide number.\n\nTry: slidx add --at 3\n")),
    }
}

/// The deck source with the slide in it.
///
/// Two operations, both from [`slidx_edit`]: the slide, then its notes. The
/// notes are a second operation rather than part of the body because the comment
/// they live in is the edit crate's spelling to know, not this command's.
fn splice(
    source: &str,
    options: &DeckParseOptions,
    at: usize,
    title: &str,
    notes: Option<&str>,
) -> Result<String, String> {
    let body = format!("# {}", title.trim());
    let inserted = apply(source, options, &EditOp::InsertSlide { at, body })
        .map_err(|error| format!("slidx could not add the slide: {error}\n"))?;

    let Some(notes) = notes else {
        return Ok(inserted);
    };

    apply(&inserted, options, &EditOp::SetNotes { slide: at.into(), notes: notes.to_string() })
        .map_err(|error| format!("the slide was added and the notes were not: {error}\n"))
}

/// What reached the disk.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Written {
    /// The file the new slide is in.
    file: PathBuf,
    /// Files moved along to make room, old name and new.
    renames: Vec<(PathBuf, PathBuf)>,
}

/// Puts the spliced source back where it came from.
///
/// A single-file deck is the whole source, which is the splice and nothing else.
/// A directory deck gets one new file holding the new slide's bytes — cut from
/// the spliced source at the spans the operations themselves agreed the seams
/// are — and not one byte of any existing slide file is rewritten.
fn write(
    deck: &DeckSource,
    edited: &str,
    options: &DeckParseOptions,
    at: usize,
) -> Result<Written, String> {
    if deck.files.is_empty() {
        let file = PathBuf::from(&deck.label);
        fs::write(&file, edited).map_err(|error| unwritable(&file, &error))?;

        return Ok(Written { file, renames: Vec::new() });
    }

    let spans = slide_spans(edited, options);
    let slide = spans
        .get(at)
        .ok_or_else(|| "the slide was spliced and then could not be found\n".to_string())?;
    let body = format!("{}\n", slide.content.slice(edited).trim());

    let plan = name(&deck.paths(), at, spans.len())?;

    // Every rename is checked before any of them happens: a deck half-renumbered
    // is a deck whose slides are in the wrong order, and that is worse than a
    // refusal. A name that another slide is about to vacate is free — the moves
    // run from the end backwards for exactly that reason.
    let vacated: Vec<&Path> = plan.renames.iter().map(|(from, _)| from.as_path()).collect();
    for (_, to) in &plan.renames {
        if to.exists() && !vacated.contains(&to.as_path()) {
            return Err(format!(
                "{} is already there and is not a slide, so slidx will not move one onto\n\
                 it. Move it aside and try again.\n",
                to.display()
            ));
        }
    }

    for (from, to) in plan.renames.iter().rev() {
        fs::rename(from, to).map_err(|error| {
            format!("could not move {} to {}: {error}\n", from.display(), to.display())
        })?;
    }

    fs::write(&plan.file, body).map_err(|error| unwritable(&plan.file, &error))?;

    Ok(plan)
}

/// A slide file's name, split where the number ends.
///
/// `0002-what-goes-wrong.md` is a number and a label. The number decides the
/// order and the label is the author's note to themselves, so a file moved along
/// keeps its label and only its number changes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Numbered {
    number: u32,
    /// Digits as written, so `0002` stays four wide rather than becoming `2`.
    width: usize,
    /// Everything after the digits, extension included.
    rest: String,
}

impl Numbered {
    fn parse(name: &str) -> Option<Self> {
        let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return None;
        }

        Some(Self {
            number: digits.parse().ok()?,
            width: digits.len(),
            rest: name[digits.len()..].to_string(),
        })
    }

    /// This number, spelled at its own width, with a new tail.
    fn spelled(&self, rest: &str) -> String {
        format!("{:0width$}{rest}", self.number, width = self.width)
    }

    /// The next number along, which is what a file moved along is called.
    fn next(&self, rest: &str) -> String {
        Self { number: self.number + 1, ..self.clone() }.spelled(rest)
    }
}

/// Which file the new slide goes in, and what has to move for it.
///
/// `slides` is how many the deck has *after* the splice, which is what says
/// whether one file holds one slide. A file holding two makes a position
/// ambiguous — the third slide could be in the second file or the third — and
/// guessing there puts a slide in the wrong place in the talk.
fn name(files: &[PathBuf], at: usize, slides: usize) -> Result<Written, String> {
    let last = files.last().ok_or_else(|| "a deck with no slide files\n".to_string())?;
    let directory = last.parent().unwrap_or(Path::new(".")).to_path_buf();

    let numbered =
        |file: &Path| -> Option<Numbered> { Numbered::parse(&file.file_name()?.to_string_lossy()) };

    // Appending needs no seam: the new slide goes after the last file, whatever
    // the files before it hold.
    if at >= files.len() {
        let Some(number) = numbered(last) else {
            return Err(unnumbered(last));
        };

        return Ok(Written { file: directory.join(number.next(".md")), renames: Vec::new() });
    }

    if at == 0 {
        return Err(first_slide_moves_the_frontmatter(&files[0]));
    }

    if files.len() + 1 != slides {
        return Err(more_than_one_slide_in_a_file(files.len(), slides - 1));
    }

    // Below the seam every file has to move along, and each keeps its own
    // label. A file with no number cannot be moved along, so nothing is.
    let mut renames = Vec::new();
    for file in &files[at..] {
        let Some(number) = numbered(file) else {
            return Err(unnumbered(file));
        };

        renames.push((file.clone(), directory.join(number.next(&number.rest))));
    }

    // The new slide takes the vacated number and none of the label that went
    // with it: `0002-what-goes-wrong.md` is a note about the slide that used to
    // be there, and it moved along with its slide.
    let taken = numbered(&files[at]).ok_or_else(|| unnumbered(&files[at]))?;

    Ok(Written { file: directory.join(taken.spelled(".md")), renames })
}

fn report(written: &Written, at: usize, title: &str, style: &Style) -> String {
    let mut text = format!(
        "  {}  {}\n",
        style.paint(Ink::Pass, format!("slide {}", at + 1)),
        style.paint(Ink::Strong, title.trim())
    );

    text.push_str(&format!("  {}\n", style.paint(Ink::Faint, written.file.display())));

    for (from, to) in &written.renames {
        text.push_str(&format!(
            "  {}\n",
            style.paint(
                Ink::Faint,
                format!("moved along: {} -> {}", file_name(from), file_name(to))
            )
        ));
    }

    text
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

fn unwritable(path: &Path, error: &std::io::Error) -> String {
    format!("Could not write {}: {error}\n", path.display())
}

fn unnumbered(file: &Path) -> String {
    format!(
        "The slide files in this deck are not numbered, so slidx cannot name a new\n\
         one: {} does not start with a number.\n\n\
         Deck files are read in name order, so a name that sorts wrong reorders the\n\
         talk. Rename them 0001.md, 0002.md and so on, or add the slide by hand.\n",
        file_name(file)
    )
}

fn more_than_one_slide_in_a_file(files: usize, slides: usize) -> String {
    format!(
        "This deck is {files} files and {slides} slides, so one of them holds more than\n\
         one slide and slidx cannot tell which file a position is in.\n\n\
         Adding at the end works whatever the files hold: `slidx add` with no --at.\n\
         Otherwise split the file so each one holds a single slide.\n"
    )
}

fn first_slide_moves_the_frontmatter(first: &Path) -> String {
    format!(
        "slidx will not add a slide before the first one in a deck kept as a directory.\n\n\
         The deck's own frontmatter — its title, theme and slot — is the first slide's,\n\
         and it has to be at the top of {}. A new first slide means moving that block\n\
         to another file, which is a different operation and should look like one.\n\n\
         Add it second and reorder it in the editor, or move the block yourself.\n",
        file_name(first)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-add-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        fn deck(&self, files: &[(&str, &str)]) -> PathBuf {
            let slides = self.0.join("slides");
            fs::create_dir_all(&slides).expect("slides");

            for (name, body) in files {
                fs::write(slides.join(name), body).expect("write");
            }

            slides
        }

        fn names(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(self.0.join("slides"))
                .expect("read")
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }

        fn read(&self, name: &str) -> String {
            fs::read_to_string(self.0.join("slides").join(name)).expect("read")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn options() -> DeckParseOptions {
        DeckParseOptions::default()
    }

    fn added(source: &str, at: usize, title: &str) -> String {
        splice(source, &options(), at, title, None).expect("a splice")
    }

    const DECK: &str = "---\ntitle: A talk\n---\n\n# One\n\nbody\n\n---\n\n# Two\n";

    #[test]
    fn a_slide_added_at_the_end_leaves_every_other_byte_of_the_source_alone() {
        // The law the edit crate exists for, restated where a second writer
        // would break it: adding a slide must not touch the slides that were
        // already there.
        let after = added(DECK, 2, "Three");

        assert!(after.starts_with(DECK.trim_end()), "{after}");
        assert!(after.contains("# Three"), "{after}");
    }

    #[test]
    fn a_slide_added_in_the_middle_pushes_the_one_that_was_there_down() {
        let after = added(DECK, 1, "Inserted");
        let deck = parse_deck(&after, &options());

        assert_eq!(
            deck.slides.iter().map(|slide| slide.display_title()).collect::<Vec<_>>(),
            ["One", "Inserted", "Two"]
        );
    }

    #[test]
    fn an_authors_own_spacing_and_bullets_survive_a_slide_being_added() {
        // What a re-serialising writer would quietly regularise, and the reason
        // this command owns no formatting at all.
        let source = "---\ntitle: A talk\n---\n\n#   One\n\n*  a\n*  b\n\n\n---\n\n# Two\n";
        let after = added(source, 2, "Three");

        assert!(after.contains("#   One"), "{after}");
        assert!(after.contains("*  a"), "{after}");
        assert!(after.contains("\n\n\n---"), "{after}");
    }

    #[test]
    fn the_title_is_the_new_slides_heading_and_nothing_else_is_composed() {
        let after = added(DECK, 2, "What goes wrong");

        assert!(after.contains("# What goes wrong"), "{after}");
    }

    #[test]
    fn notes_are_written_by_the_edit_crate_rather_than_spelled_out_here() {
        // The comment notes live in is the edit crate's spelling to know. A
        // second spelling of it would be a second dialect.
        let after =
            splice(DECK, &options(), 2, "Three", Some("open with the outcome")).expect("a splice");
        let deck = parse_deck(&after, &options());

        assert_eq!(deck.slides[2].notes_text(), "open with the outcome");
    }

    #[test]
    fn a_single_file_deck_is_written_back_as_the_spliced_source() {
        let scratch = Scratch::new("single");
        let file = scratch.0.join("talk.md");
        fs::write(&file, DECK).expect("write");

        let deck = source::read(&file, "---").expect("a deck");
        let edited = added(&deck.source, 2, "Three");
        let written = write(&deck, &edited, &options(), 2).expect("write");

        assert_eq!(written.file, file);
        assert_eq!(fs::read_to_string(&file).expect("read"), edited);
    }

    #[test]
    fn a_slide_added_to_a_directory_deck_becomes_one_new_file_and_touches_no_other() {
        // The minimal write: the slides that were already there are not
        // rewritten, so `git diff` shows one added file and nothing else.
        let scratch = Scratch::new("append");
        let slides = scratch
            .deck(&[("0001.md", "---\ntitle: A talk\n---\n\n# One\n"), ("0002.md", "# Two\n")]);
        let before = scratch.read("0001.md");

        let deck = source::read(&slides, "---").expect("a deck");
        let edited = added(&deck.source, 2, "Three");
        write(&deck, &edited, &options(), 2).expect("write");

        assert_eq!(scratch.names(), ["0001.md", "0002.md", "0003.md"]);
        assert_eq!(scratch.read("0003.md"), "# Three\n");
        assert_eq!(scratch.read("0001.md"), before, "an existing slide file was rewritten");
    }

    #[test]
    fn a_slide_added_in_the_middle_moves_the_files_after_it_along() {
        // Deck files are read in name order, so making room is a rename. The
        // slides that move keep their bytes; only their names change.
        let scratch = Scratch::new("middle");
        let slides = scratch.deck(&[
            ("0001.md", "---\ntitle: A talk\n---\n\n# One\n"),
            ("0002.md", "# Two\n"),
            ("0003.md", "# Three\n"),
        ]);

        let deck = source::read(&slides, "---").expect("a deck");
        let edited = added(&deck.source, 1, "Inserted");
        write(&deck, &edited, &options(), 1).expect("write");

        assert_eq!(scratch.names(), ["0001.md", "0002.md", "0003.md", "0004.md"]);
        assert_eq!(scratch.read("0002.md"), "# Inserted\n");
        assert_eq!(scratch.read("0003.md"), "# Two\n");
        assert_eq!(scratch.read("0004.md"), "# Three\n");
    }

    #[test]
    fn a_file_moved_along_keeps_the_label_its_author_gave_it() {
        // `0002-what-goes-wrong.md` is a number and a note to self. Only the
        // number is slidx's business.
        let scratch = Scratch::new("labels");
        let slides = scratch.deck(&[
            ("0001-intro.md", "---\ntitle: A talk\n---\n\n# One\n"),
            ("0002-what.md", "# Two\n"),
        ]);

        let deck = source::read(&slides, "---").expect("a deck");
        let edited = added(&deck.source, 1, "Inserted");
        write(&deck, &edited, &options(), 1).expect("write");

        assert_eq!(scratch.names(), ["0001-intro.md", "0002.md", "0003-what.md"]);
    }

    #[test]
    fn a_deck_whose_files_are_not_numbered_is_refused_rather_than_guessed_at() {
        // A guessed name that sorts wrong reorders somebody's talk, and they
        // find out on stage.
        let scratch = Scratch::new("unnumbered");
        let slides = scratch.deck(&[("intro.md", "# One\n"), ("outro.md", "# Two\n")]);

        let deck = source::read(&slides, "---").expect("a deck");
        let edited = added(&deck.source, 2, "Three");
        let message = write(&deck, &edited, &options(), 2).expect_err("refused");

        assert!(message.contains("not numbered"), "{message}");
        assert!(message.contains("0001.md"), "{message}");
        assert_eq!(scratch.names(), ["intro.md", "outro.md"], "the deck was changed anyway");
    }

    #[test]
    fn adding_before_the_first_slide_of_a_directory_deck_says_why_it_will_not() {
        // The deck's frontmatter is the first slide's and has to open the first
        // file. Moving it between files is a different operation.
        let scratch = Scratch::new("first");
        let slides = scratch
            .deck(&[("0001.md", "---\ntitle: A talk\n---\n\n# One\n"), ("0002.md", "# Two\n")]);

        let deck = source::read(&slides, "---").expect("a deck");
        let edited = added(&deck.source, 0, "Before");
        let message = write(&deck, &edited, &options(), 0).expect_err("refused");

        assert!(message.contains("frontmatter"), "{message}");
        assert_eq!(scratch.names(), ["0001.md", "0002.md"]);
    }

    #[test]
    fn a_name_another_slide_is_about_to_vacate_is_not_treated_as_taken() {
        // The moves run from the end backwards, so a contiguously numbered deck
        // shifts along without any name ever being occupied twice. Reading
        // `0003.md` as "in the way" would refuse every ordinary insert.
        let scratch = Scratch::new("vacated");
        let slides = scratch.deck(&[
            ("0001.md", "---\ntitle: A talk\n---\n\n# One\n"),
            ("0002.md", "# Two\n"),
            ("0003.md", "# Three\n"),
        ]);

        let deck = source::read(&slides, "---").expect("a deck");
        let edited = added(&deck.source, 1, "Inserted");
        write(&deck, &edited, &options(), 1).expect("the insert is ordinary");

        assert_eq!(scratch.names(), ["0001.md", "0002.md", "0003.md", "0004.md"]);
        assert_eq!(scratch.read("0004.md"), "# Three\n");
    }

    #[test]
    fn a_rename_onto_something_that_is_not_a_slide_stops_before_anything_moves() {
        // Better a refusal than a deck half-renumbered, which is a deck whose
        // slides are in the wrong order and whose author finds out on stage.
        let scratch = Scratch::new("collision");
        let slides = scratch
            .deck(&[("0001.md", "---\ntitle: A talk\n---\n\n# One\n"), ("0002.md", "# Two\n")]);
        // A directory, so the deck reader does not see it as a slide and the
        // rename has something in its way that is nobody's slide.
        fs::create_dir(slides.join("0003.md")).expect("a directory");

        let deck = source::read(&slides, "---").expect("a deck");
        let edited = added(&deck.source, 1, "Inserted");
        let message = write(&deck, &edited, &options(), 1).expect_err("refused");

        assert!(message.contains("already there"), "{message}");
        assert_eq!(scratch.read("0002.md"), "# Two\n", "a slide moved before the check");
    }

    #[test]
    fn a_file_holding_two_slides_makes_a_position_ambiguous_and_is_said_so() {
        // Which file is the third slide in? Guessing puts a slide somewhere
        // else in the talk. Appending still works, and the message says that.
        let scratch = Scratch::new("two-in-one");
        let slides = scratch.deck(&[
            ("0001.md", "---\ntitle: A talk\n---\n\n# One\n\n---\n\n# Two\n"),
            ("0002.md", "# Three\n"),
        ]);

        let deck = source::read(&slides, "---").expect("a deck");
        let edited = added(&deck.source, 1, "Inserted");
        let message = write(&deck, &edited, &options(), 1).expect_err("refused");

        assert!(message.contains("2 files and 3 slides"), "{message}");
        assert!(message.contains("no --at"), "{message}");

        // And appending is allowed, because it needs no seam.
        let edited = added(&deck.source, 3, "Fourth");
        write(&deck, &edited, &options(), 3).expect("appending is fine");
        assert_eq!(scratch.read("0003.md"), "# Fourth\n");
    }

    #[test]
    fn the_new_slide_is_reported_by_its_number_and_its_file() {
        let written = Written {
            file: PathBuf::from("/talks/a/slides/0003.md"),
            renames: vec![(
                PathBuf::from("/talks/a/slides/0003.md"),
                PathBuf::from("/talks/a/slides/0004.md"),
            )],
        };

        let text = report(&written, 2, "What goes wrong", &Style::plain());

        assert!(text.contains("slide 3"), "{text}");
        assert!(text.contains("What goes wrong"), "{text}");
        assert!(text.contains("0003.md"), "{text}");
        assert!(text.contains("moved along: 0003.md -> 0004.md"), "{text}");
    }

    #[test]
    fn a_slide_number_is_one_based_because_that_is_how_a_speaker_counts() {
        let matches = matches_for("add --at 3");
        assert_eq!(position(&matches, 5), Ok(2));

        // One past the end is the end, so a script can compute a position
        // without a special case.
        assert_eq!(position(&matches_for("add --at 6"), 5), Ok(5));
        assert_eq!(position(&matches_for("add"), 5), Ok(5));
    }

    #[test]
    fn a_slide_number_outside_the_deck_says_what_the_end_is() {
        let message = position(&matches_for("add --at 9"), 3).expect_err("refused");

        assert!(message.contains("outside a deck of 3 slides"), "{message}");
        assert!(message.contains("4 is the end"), "{message}");
    }

    #[test]
    fn a_slide_number_that_is_not_a_number_is_a_misuse() {
        assert!(position(&matches_for("add --at last"), 3).is_err());
        assert!(position(&matches_for("add --at 0"), 3).is_err());
    }

    #[test]
    fn a_file_name_keeps_its_width_when_it_moves_along() {
        // `0002.md` becoming `3.md` would sort before `0002.md` and reorder the
        // deck.
        let numbered = Numbered::parse("0002-what.md").expect("numbered");

        assert_eq!(numbered.number, 2);
        assert_eq!(numbered.next(".md"), "0003.md");
        assert_eq!(numbered.next(&numbered.rest), "0003-what.md");
        assert_eq!(Numbered::parse("intro.md"), None);

        // And the names it produces sort where they were meant to, which is the
        // only thing that decides a deck's order.
        let mut names = ["0001.md".to_string(), numbered.next(".md"), "0002-what.md".to_string()];
        names.sort();
        assert_eq!(names, ["0001.md", "0002-what.md", "0003.md"]);
    }

    fn matches_for(line: &str) -> Matches {
        let argv: Vec<String> = line.split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }
}
