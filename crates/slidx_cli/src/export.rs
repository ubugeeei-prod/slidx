//! `slidx export` — the deck, as a file for somewhere else.
//!
//! A talk leaves the machine it was written on in more than one shape. A
//! conference wants a PDF, a review form wants one file per slide, a blog post
//! wants images, a static host wants the site, and a co-presenter wants
//! something they can open in the tool they already use. All five are the same
//! deck, and none of them is a reason to render it twice.
//!
//! ## This is orchestration, not a renderer
//!
//! **`slidx export` never renders a deck.** It runs the build the author
//! already has — `@slidx/vite-plugin`, driving a real browser over the print
//! shell it emitted — and packages what that wrote. slidx still has no `build`,
//! and this is not one wearing another name: without the plugin installed it
//! produces nothing at all, and says so.
//!
//! That is the difference between an export and a second pipeline. A second
//! pipeline would mean the file handed to a conference could differ from the
//! deck that was linted, checked in a browser, and rehearsed — and nobody would
//! find out until it was on a screen in front of a room.
//!
//! ## Nothing leaves the machine
//!
//! No upload, no API client, no OAuth, no token. slidx produces a file and the
//! author opens it, which is the same boundary [`crate::publish`] holds and for
//! the same reason: a tool that can post as you is a tool that has to be trusted
//! with a credential.
//!
//! ## One stop, one page — everywhere
//!
//! Each export treats the *stop* as its unit, not the slide. That is not a new
//! decision: the print shell already made it, on the grounds that a handout
//! collapsing an eight-step build into one slide shows the punchline without the
//! setup. Being consistent with that is worth more than any per-target
//! cleverness, and every target says in one line what survives the trip.

pub mod build;
pub mod package;

use std::path::{Path, PathBuf};

use slidx_core::{parse_deck, slugify, DeckParseOptions};
use slidx_export::{ExportTarget, EXPORT_TARGETS};

use crate::args::Matches;
use crate::home::Home;
use crate::index::{self, Entry};
use crate::lint::source;
use crate::preview::{Built, DEFAULT_OUT_DIR};
use crate::report::{self, VALUE_INDENT};
use crate::style::{Ink, Style};
use crate::Outcome;

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let target = match target(matches) {
        Ok(target) => target,
        Err(message) => return Outcome::misuse(message),
    };

    let separator =
        matches.value("separator").map(str::to_string).unwrap_or_else(|| "---".to_string());
    let path = matches
        .first_positional()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source::DEFAULT_DIR));

    let deck_source = match source::read(&path, &separator) {
        Ok(deck_source) => deck_source,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    let deck = parse_deck(
        &deck_source.source,
        &DeckParseOptions { separator, ..DeckParseOptions::default() },
    );

    let root = crate::lint::project_root(&path).unwrap_or_else(|| PathBuf::from("."));

    // The index fills itself: running a command on a deck is what puts it in the
    // list. Best-effort in the strongest sense — see `index::remember`.
    index::remember(&Home::discover().index(), Entry::new(root.clone()).describing(&deck));

    let dist = matches.value("dist").map(PathBuf::from);

    if !matches.is_set("no-build") {
        let asked = build::Build { root: &root, dist: dist.as_deref(), frame: target.frame() };

        if let Err(message) = build::run(&asked) {
            return Outcome::misuse(message);
        }
    }

    let dist = dist.unwrap_or_else(|| root.join(DEFAULT_OUT_DIR));

    let Some(built) = Built::find(&dist) else {
        return Outcome::misuse(nothing_built(&dist, matches.is_set("no-build")));
    };

    let slug = slug(&deck);

    let out = Path::new(matches.value("out").unwrap_or("."));

    match package::package(target, &built, &deck, out, &slug) {
        Ok(packaged) => Outcome::out(wrote(target, &packaged, style)),
        Err(message) => Outcome::misuse(message),
    }
}

/// Which target was asked for.
///
/// Required, and named in full when it is missing. `slidx export` on its own
/// cannot pick one for somebody: a PDF and a zip of images are different jobs,
/// and guessing would produce the wrong file slowly.
fn target(matches: &Matches) -> Result<ExportTarget, String> {
    let Some(name) = matches.value("target") else {
        return Err(format!("`slidx export` needs a target.\n\n{}", offered()));
    };

    ExportTarget::parse(name)
        .ok_or_else(|| format!("`{name}` is not something slidx exports.\n\n{}", offered()))
}

/// The list, written out of the table so it cannot fall behind it.
fn offered() -> String {
    let widest = EXPORT_TARGETS.iter().map(|target| target.as_token().len()).max().unwrap_or(0);

    let list: String = EXPORT_TARGETS
        .iter()
        .map(|target| {
            format!("  {:widest$}  {}\n", target.as_token(), target.summary(), widest = widest)
        })
        .collect();

    format!("It can produce:\n\n{list}\nFor example:\n\n  slidx export --target pdf\n")
}

/// The name the exported file carries, from the deck's own title.
///
/// A title is what somebody looking in a downloads folder recognises. Falling
/// back to the aspect of the deck nobody chose — a timestamp, the directory
/// name — would produce a file whose name says nothing, so an untitled deck is
/// called what the rest of slidx calls it.
fn slug(deck: &slidx_core::Deck) -> String {
    let slug = slugify(deck.meta.display_title());

    if slug.is_empty() {
        "deck".to_string()
    } else {
        slug
    }
}

fn wrote(target: ExportTarget, packaged: &package::Packaged, style: &Style) -> String {
    let mut text = format!("{}\n\n", style.paint(Ink::Strong, "slidx export"));

    text.push_str(&report::flowed(
        &packaged.path.display().to_string(),
        VALUE_INDENT,
        Ink::Strong,
        style,
    ));

    let count = if packaged.parts > 1 {
        format!("{} files — {}", packaged.parts, target.summary())
    } else {
        target.summary().to_string()
    };

    text.push_str(&report::flowed(&count, VALUE_INDENT, Ink::Faint, style));
    // The honest half. An export that silently lost the animation would be
    // worse than one that said it would, and this is the moment somebody is
    // still looking.
    text.push_str(&report::flowed(target.keeps(), VALUE_INDENT, Ink::Faint, style));

    text
}

fn nothing_built(dist: &Path, skipped: bool) -> String {
    let cause = if skipped {
        "`--no-build` was given, so slidx packaged what was already there — and there\n\
         is no build in it."
    } else {
        "The build reported success and left nothing that looks like a deck, which\n\
         usually means it built a different project in this directory."
    };

    format!(
        "{} does not hold a built deck.\n\n{cause}\n\n\
         Point slidx at the build's output directory with `--dist <path>`.\n",
        dist.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MISUSE, OK};
    use slidx_export::zip;
    use std::fs;

    /// A deck, and a directory holding what a build of it left behind.
    ///
    /// The build is written rather than run: what this command does with a
    /// build's output is a different question from whether it can start one, and
    /// a test that shelled out to Vite would answer neither quickly. Starting
    /// one for real is `tests/export.rs`.
    struct Project(PathBuf);

    impl Project {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("slidx-export-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("slides")).expect("scratch");
            fs::write(
                root.join("slides/0001.md"),
                "---\ntitle: Making Decks Fast\n---\n\n# One\n\n<!-- notes: open with the outcome -->\n",
            )
            .expect("deck");

            fs::create_dir_all(root.join("dist/slides/2")).expect("dist");
            fs::write(root.join("dist/slides/index.html"), "<h1>One</h1>").expect("page");
            fs::write(root.join("dist/slides/2/index.html"), "<h1>Two</h1>").expect("page");
            fs::write(root.join("dist/slides/runtime.js"), "export const x = 1;").expect("runtime");

            Self(root)
        }

        fn pdf(self) -> Self {
            fs::write(self.0.join("dist/deck.pdf"), b"%PDF-1.7\n%%EOF\n").expect("pdf");
            self
        }

        fn frame(self, under: &str, name: &str, bytes: &[u8]) -> Self {
            let directory = self.0.join("dist").join(under);
            fs::create_dir_all(&directory).expect("frames");
            fs::write(directory.join(name), bytes).expect("frame");
            self
        }

        /// Runs the command the way a person would, minus starting a build.
        fn export(&self, target: &str) -> Outcome {
            let line = format!(
                "export --target {target} {} --dist {} --out {} --no-build",
                self.0.join("slides").display(),
                self.0.join("dist").display(),
                self.0.join("out").display()
            );
            let argv: Vec<String> = line.split_whitespace().map(String::from).collect();

            crate::run(&argv, &Style::plain())
        }

        fn read(&self, name: &str) -> Vec<u8> {
            fs::read(self.0.join("out").join(name)).unwrap_or_default()
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_site_export_is_named_for_the_deck_and_is_an_archive() {
        // Both halves of "reachable": the command ran end to end, and what it
        // left behind is the kind of file it claims to be.
        let project = Project::new("site");
        let outcome = project.export("browser");

        assert_eq!(outcome.code, OK, "{}{}", outcome.stdout, outcome.stderr);
        assert_eq!(&project.read("making-decks-fast-site.zip")[..4], b"PK\x03\x04");
    }

    #[test]
    fn the_site_export_holds_the_pages_the_build_wrote() {
        let project = Project::new("pages");
        project.export("browser");

        assert_eq!(
            zip::names(&project.read("making-decks-fast-site.zip")),
            ["slides/2/index.html", "slides/index.html", "slides/runtime.js"]
        );
    }

    #[test]
    fn the_pdf_export_is_a_pdf() {
        let project = Project::new("pdf").pdf();
        let outcome = project.export("pdf");

        assert_eq!(outcome.code, OK, "{}{}", outcome.stdout, outcome.stderr);
        assert_eq!(&project.read("making-decks-fast.pdf")[..5], b"%PDF-");
    }

    #[test]
    fn the_per_slide_export_holds_one_document_per_slide() {
        let project = Project::new("pdfzip")
            .frame("export/pdf", "slide-01.pdf", b"%PDF-1.7\n")
            .frame("export/pdf", "slide-02.pdf", b"%PDF-1.7\n");
        let outcome = project.export("pdf-zip");

        assert_eq!(outcome.code, OK, "{}{}", outcome.stdout, outcome.stderr);
        assert_eq!(
            zip::names(&project.read("making-decks-fast-pdfs.zip")),
            ["slide-01.pdf", "slide-02.pdf"]
        );
    }

    #[test]
    fn the_image_export_holds_one_image_per_stop() {
        // The stop, not the slide: the answer the print shell already gave.
        let project = Project::new("png")
            .frame("export/png", "slide-02-stop-01.png", b"\x89PNG\r\n")
            .frame("export/png", "slide-02-stop-02.png", b"\x89PNG\r\n");
        let outcome = project.export("png");

        assert_eq!(outcome.code, OK, "{}{}", outcome.stdout, outcome.stderr);
        assert_eq!(
            zip::names(&project.read("making-decks-fast-pngs.zip")),
            ["slide-02-stop-01.png", "slide-02-stop-02.png"]
        );
    }

    #[test]
    fn the_report_says_how_many_files_went_in_and_what_survived_the_trip() {
        // A zip holding one page when the deck has forty looks identical in a
        // listing, so the count is the part of the report worth printing.
        let project = Project::new("report");
        let outcome = project.export("browser");

        assert!(outcome.stdout.contains("3 files"), "{}", outcome.stdout);
        assert!(outcome.stdout.contains("presenter view"), "{}", outcome.stdout);
    }

    #[test]
    fn no_target_lists_every_target_rather_than_picking_one() {
        // A PDF and a zip of images are different jobs. Guessing would produce
        // the wrong file, slowly.
        let argv: Vec<String> = vec!["export".to_string()];
        let outcome = crate::run(&argv, &Style::plain());

        assert_eq!(outcome.code, MISUSE);
        for target in EXPORT_TARGETS {
            assert!(outcome.stderr.contains(target.as_token()), "{}", outcome.stderr);
        }
    }

    #[test]
    fn the_flag_that_takes_a_target_lists_every_target_there_is() {
        // The one place the table cannot write itself: a flag summary is a
        // const, so it is typed by hand and can fall behind the enum beside it.
        // A target missing here is one nobody discovers from `--help`.
        let flag = crate::command::find("export")
            .and_then(|command| command.flag("target"))
            .expect("--target");

        for target in EXPORT_TARGETS {
            assert!(
                flag.summary.contains(target.as_token()),
                "--target does not mention {}",
                target.as_token()
            );
        }
    }

    #[test]
    fn a_target_nobody_has_names_the_ones_that_exist() {
        let project = Project::new("typo");
        let outcome = project.export("keynote");

        assert_eq!(outcome.code, MISUSE);
        assert!(outcome.stderr.contains("pdf-zip"), "{}", outcome.stderr);
    }

    #[test]
    fn a_deck_that_is_not_there_exits_two_rather_than_writing_an_empty_archive() {
        let argv: Vec<String> = "export --target browser /nowhere/at/all --no-build"
            .split(' ')
            .map(String::from)
            .collect();
        let outcome = crate::run(&argv, &Style::plain());

        assert_eq!(outcome.code, MISUSE);
        assert!(outcome.stdout.is_empty());
    }

    #[test]
    fn packaging_a_directory_with_no_build_in_it_says_so_rather_than_writing_nothing() {
        let project = Project::new("nobuild");
        let _ = fs::remove_dir_all(project.0.join("dist"));
        fs::create_dir_all(project.0.join("dist")).expect("empty dist");

        let outcome = project.export("browser");

        assert_eq!(outcome.code, MISUSE);
        assert!(outcome.stderr.contains("--dist"), "{}", outcome.stderr);
        assert!(outcome.stderr.contains("--no-build"), "{}", outcome.stderr);
    }

    #[test]
    fn the_report_carries_no_escape_sequences_when_colour_is_off() {
        let project = Project::new("plain");

        assert!(!project.export("browser").stdout.contains('\u{1b}'));
    }
}
