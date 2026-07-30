//! `slidx preview` — look at what the build produced.
//!
//! Two things, both about an artifact that already exists: open the PDF with
//! whatever opens PDFs on this machine, or serve the built deck on loopback
//! and open a browser at it.
//!
//! ## It does not build
//!
//! Not "it builds if it has to" — it does not build, and when there is nothing
//! to preview it says so and names `vite build`. The reasoning is the same one
//! that keeps `slidx build` from existing at all: the pipeline is
//! `@slidx/vite-plugin`, and a second thing that can produce a deck is two
//! answers to one question. A preview command that quietly rebuilt would be
//! exactly that, wearing a different name.
//!
//! It also means what you are looking at is the artifact — the same files a
//! static host would serve, not a transformed view of the source. That is the
//! only kind of preview worth trusting the night before a talk.
//!
//! ## Why serve at all rather than open the file
//!
//! A slide with more than one stop imports `./runtime.js` as a module, and a
//! browser refuses a module import from a `file://` origin whatever the path
//! says. Opened off disk, a staged deck sits frozen on its first stop and
//! looks broken. So `--web` puts it on a real origin, which is what the deck
//! was built for.
//!
//! ## Loopback, always
//!
//! See [`server`]. An unreleased talk should not be reachable from conference
//! wifi, and nothing here offers to make it so.

pub mod opener;
pub mod server;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::args::Matches;
use crate::report::{self, VALUE_INDENT};
use crate::style::{Ink, Style};
use crate::{Outcome, MISUSE, OK};

/// Where `vite build` writes, at Vite's own default.
pub const DEFAULT_OUT_DIR: &str = "dist";

/// What the plugin calls `base`, at its default — the directory the deck's
/// `index.html` lands in.
const DEFAULT_BASE: &str = "slides";

/// The PDF name the plugin uses when nobody chooses one.
const DEFAULT_PDF: &str = "deck.pdf";

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let out = matches
        .first_positional()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT_DIR));

    if !out.is_dir() {
        return Outcome::misuse(nothing_built(&out));
    }

    let Some(built) = Built::find(&out) else {
        return Outcome::misuse(not_a_deck(&out));
    };

    if matches.is_set("web") {
        web(&built, matches, style)
    } else {
        pdf(&built, matches, style)
    }
}

/// What a build left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    /// The output directory, which is what the server serves.
    pub root: PathBuf,
    /// The route the deck's first slide is at, `/slides/`.
    pub route: String,
    /// The exported PDF, if the build made one.
    pub pdf: Option<PathBuf>,
}

impl Built {
    /// Looks at an output directory and works out what is in it.
    ///
    /// Found rather than assumed, because `base` and the PDF's name are both
    /// options somebody may have changed — and a preview that only worked for
    /// the defaults would be a preview that fails for exactly the people who
    /// configured something.
    pub fn find(out: &Path) -> Option<Self> {
        let route = deck_route(out)?;

        Some(Self { root: out.to_path_buf(), route, pdf: find_pdf(out) })
    }

    /// The URL the deck is at, once a port is known.
    pub fn url(&self, port: u16) -> String {
        format!("http://127.0.0.1:{port}{}", self.route)
    }
}

/// The route of the deck's first slide.
///
/// The plugin emits `runtime.js` next to the deck's `index.html` and nothing
/// else in a build does, so that pair is what identifies a deck rather than a
/// bare `index.html` — which an ordinary web project also has, at the root,
/// right next to the deck it embeds. Guessing on `index.html` alone previews
/// the site instead of the talk.
///
/// The plugin's own default is checked first because it is the answer almost
/// every time, and scanning a directory to confirm it would be work for
/// nothing.
fn deck_route(out: &Path) -> Option<String> {
    if is_deck(&out.join(DEFAULT_BASE)) {
        return Some(format!("/{DEFAULT_BASE}/"));
    }

    let bases: Vec<String> = fs::read_dir(out)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| is_deck(&entry.path()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    if let Some(base) = bases.iter().min() {
        return Some(format!("/{base}/"));
    }

    if is_deck(out) {
        return Some("/".to_string());
    }

    // Nothing carries a runtime. A single-stop deck at the root is still a
    // deck, and this is the last reading that is not a guess.
    out.join("index.html").is_file().then(|| "/".to_string())
}

/// True for a directory holding a built deck's first slide.
fn is_deck(directory: &Path) -> bool {
    directory.join("index.html").is_file() && directory.join("runtime.js").is_file()
}

/// The exported PDF, preferring the plugin's default name.
///
/// Any other `.pdf` at the top of the output counts, because `pdf.fileName` is
/// an option — but the deck's own is the one to open when both are there.
fn find_pdf(out: &Path) -> Option<PathBuf> {
    let default = out.join(DEFAULT_PDF);
    if default.is_file() {
        return Some(default);
    }

    let mut found: Vec<PathBuf> = fs::read_dir(out)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|it| it == "pdf"))
        .collect();

    found.sort();
    found.into_iter().next()
}

/// Opens the exported PDF.
fn pdf(built: &Built, matches: &Matches, style: &Style) -> Outcome {
    let Some(pdf) = &built.pdf else {
        return Outcome::misuse(no_pdf(&built.root));
    };

    let path = pdf.display().to_string();
    let mut text = format!("{}\n\n", style.paint(Ink::Strong, "slidx preview"));
    text.push_str(&report::flowed(&path, VALUE_INDENT, Ink::Strong, style));

    if matches.is_set("no-open") {
        return Outcome::out(text);
    }

    if !opener::open(&path) {
        // Not an error: over SSH or in a container there is nothing to open
        // with, and the useful thing — where the PDF is — is still true.
        text.push_str(&report::flowed(
            "nothing on this machine opens a PDF, so it is left where it is",
            VALUE_INDENT,
            Ink::Faint,
            style,
        ));
    }

    Outcome::out(text)
}

/// Serves the built deck and opens a browser at it.
fn web(built: &Built, matches: &Matches, style: &Style) -> Outcome {
    let port = match matches.value("port").map(str::parse::<u16>) {
        Some(Ok(port)) => port,
        Some(Err(_)) => return Outcome::misuse(bad_port(matches.value("port").unwrap_or(""))),
        // 0 asks the operating system for a free one, so two previews at once
        // do not fight over a number nobody chose.
        None => 0,
    };

    let listener = match server::bind(port) {
        Ok(listener) => listener,
        Err(error) => {
            return Outcome::misuse(format!("could not listen on port {port}: {error}\n"))
        }
    };

    let Ok(address) = listener.local_addr() else {
        return Outcome::misuse("could not read the port that was bound\n".to_string());
    };

    let url = built.url(address.port());

    // Written now rather than returned, because the next call blocks until
    // somebody stops the process: an Outcome printed afterwards would appear
    // when the server is already gone.
    let mut out = std::io::stdout();
    let _ = write!(out, "{}", ready(&url, built, style));
    let _ = out.flush();

    if !matches.is_set("no-open") && !opener::open(&url) {
        let _ = write!(
            std::io::stderr(),
            "{}",
            report::flowed(
                "no browser opened; the address above still works",
                VALUE_INDENT,
                Ink::Faint,
                style
            )
        );
    }

    server::serve(&listener, &server::Site::new(&built.root));

    Outcome::default().with_code(OK)
}

/// What is printed the moment the port is bound.
pub fn ready(url: &str, built: &Built, style: &Style) -> String {
    let mut text = format!("{}\n\n", style.paint(Ink::Strong, "slidx preview"));

    text.push_str(&report::flowed(url, VALUE_INDENT, Ink::Strong, style));
    text.push_str(&report::flowed(
        &format!("serving {} — loopback only, nothing else can reach it", built.root.display()),
        VALUE_INDENT,
        Ink::Faint,
        style,
    ));
    text.push_str(&report::flowed("ctrl-c to stop", VALUE_INDENT, Ink::Faint, style));

    text
}

fn nothing_built(out: &Path) -> String {
    format!(
        "There is nothing at {}.\n\n\
         `slidx preview` shows what a build produced; it does not produce one.\n\
         Building a deck is @slidx/vite-plugin's job:\n\n\
         \x20 vite build\n\n\
         Then `slidx preview` again, or point it at another directory.\n",
        out.display()
    )
}

fn not_a_deck(out: &Path) -> String {
    format!(
        "{} does not look like a built deck — nothing in it has an index.html.\n\n\
         `slidx preview` reads the output of @slidx/vite-plugin:\n\n\
         \x20 vite build\n",
        out.display()
    )
}

fn no_pdf(out: &Path) -> String {
    format!(
        "{} has no PDF in it.\n\n\
         The plugin exports one when `pdf` is on and a browser is available to\n\
         render it. To look at the deck itself instead:\n\n\
         \x20 slidx preview --web\n",
        out.display()
    )
}

fn bad_port(given: &str) -> String {
    format!("`{given}` is not a port number.\n\n  slidx preview --web --port 5173\n")
}

/// Exit code for a preview that had nothing to show.
pub const NOTHING_TO_SHOW: u8 = MISUSE;

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory laid out the way `vite build` leaves one.
    struct Build(PathBuf);

    impl Build {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("slidx-preview-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("scratch");

            Self(root)
        }

        fn deck(self, base: &str) -> Self {
            let directory = if base.is_empty() { self.0.clone() } else { self.0.join(base) };
            fs::create_dir_all(directory.join("2")).expect("slides");
            fs::write(directory.join("index.html"), "<h1>one</h1>").expect("slide");
            fs::write(directory.join("2/index.html"), "<h1>two</h1>").expect("slide");
            fs::write(directory.join("runtime.js"), "export const x = 1;").expect("runtime");
            self
        }

        fn pdf(self, name: &str) -> Self {
            fs::write(self.0.join(name), b"%PDF-1.4\n").expect("pdf");
            self
        }

        fn found(&self) -> Option<Built> {
            Built::find(&self.0)
        }
    }

    impl Drop for Build {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_default_build_is_found_at_the_route_the_plugin_writes_it_to() {
        let build = Build::new("default").deck(DEFAULT_BASE);

        assert_eq!(build.found().expect("a deck").route, "/slides/");
    }

    #[test]
    fn a_deck_built_under_another_base_is_found_too() {
        // `base` is an option. A preview that only worked for the default
        // would fail for exactly the people who configured something.
        let build = Build::new("base").deck("talk");

        assert_eq!(build.found().expect("a deck").route, "/talk/");
    }

    #[test]
    fn a_deck_built_at_the_root_of_the_output_is_found_at_the_root() {
        let build = Build::new("root").deck("");

        assert_eq!(build.found().expect("a deck").route, "/");
    }

    #[test]
    fn a_deck_embedded_in_a_web_project_is_found_rather_than_the_site_around_it() {
        // An ordinary project has an index.html at the root too. Guessing on
        // that alone previews the site instead of the talk.
        let build = Build::new("embedded").deck("talk");
        fs::write(build.0.join("index.html"), "<h1>the marketing site</h1>").expect("site");

        assert_eq!(build.found().expect("a deck").route, "/talk/");
    }

    #[test]
    fn a_single_stop_deck_with_no_runtime_is_still_previewable() {
        // The last reading that is not a guess: an index.html at the root and
        // nothing carrying a runtime.
        let build = Build::new("norumtime");
        fs::write(build.0.join("index.html"), "<h1>one</h1>").expect("slide");

        assert_eq!(build.found().expect("a deck").route, "/");
    }

    #[test]
    fn a_directory_with_no_deck_in_it_is_not_a_build() {
        let build = Build::new("empty");

        assert!(build.found().is_none());
    }

    #[test]
    fn the_decks_own_pdf_is_preferred_when_the_output_holds_more_than_one() {
        let build = Build::new("pdfs").deck(DEFAULT_BASE).pdf("appendix.pdf").pdf(DEFAULT_PDF);

        assert_eq!(
            build.found().expect("a deck").pdf.expect("a pdf").file_name().unwrap(),
            DEFAULT_PDF
        );
    }

    #[test]
    fn a_pdf_under_another_name_is_still_found() {
        // `pdf.fileName` is an option too.
        let build = Build::new("named").deck(DEFAULT_BASE).pdf("making-decks-fast.pdf");

        assert_eq!(
            build.found().expect("a deck").pdf.expect("a pdf").file_name().unwrap(),
            "making-decks-fast.pdf"
        );
    }

    #[test]
    fn a_build_with_no_pdf_is_still_a_build() {
        // The PDF is optional — it needs a browser at build time — and the
        // deck is previewable without one.
        let build = Build::new("nopdf").deck(DEFAULT_BASE);

        assert!(build.found().expect("a deck").pdf.is_none());
    }

    #[test]
    fn the_url_points_at_loopback_and_at_the_decks_own_route() {
        let build = Build::new("url").deck(DEFAULT_BASE);

        assert_eq!(build.found().expect("a deck").url(5173), "http://127.0.0.1:5173/slides/");
    }

    #[test]
    fn nothing_built_names_the_command_that_would_build_it() {
        // The command does not build, so it has to say what does — otherwise
        // it is a dead end for somebody who has just cloned a deck.
        let message = nothing_built(Path::new("dist"));

        assert!(message.contains("vite build"), "{message}");
        assert!(message.contains("does not produce one"), "{message}");
        assert!(message.contains("@slidx/vite-plugin"), "{message}");
    }

    #[test]
    fn a_directory_that_is_not_a_deck_says_what_it_was_looking_for() {
        let message = not_a_deck(Path::new("dist"));

        assert!(message.contains("index.html"), "{message}");
        assert!(message.contains("vite build"), "{message}");
    }

    #[test]
    fn no_pdf_points_at_the_other_way_to_look_at_the_deck() {
        let message = no_pdf(Path::new("dist"));

        assert!(message.contains("slidx preview --web"), "{message}");
    }

    #[test]
    fn a_port_that_is_not_a_number_says_so_rather_than_binding_something_else() {
        assert!(bad_port("http").contains("not a port number"));
    }

    #[test]
    fn the_ready_line_says_the_url_the_directory_and_that_it_is_loopback_only() {
        let build = Build::new("ready").deck(DEFAULT_BASE);
        let found = build.found().expect("a deck");
        let text = ready(&found.url(5173), &found, &Style::plain());
        // Read flat, because the line is flowed around a real directory: where
        // it wraps depends on how long this machine's temp path is, and a
        // phrase that lands either side of a break still says the same thing.
        // Asserting against the wrapped text tests the runner, not the report.
        let said = text.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(said.contains("http://127.0.0.1:5173/slides/"), "{text}");
        assert!(said.contains(&found.root.display().to_string()), "{text}");
        assert!(said.contains("loopback only"), "{text}");
        assert!(said.contains("ctrl-c"), "{text}");
    }

    #[test]
    fn the_ready_line_carries_no_escape_sequences_when_colour_is_off() {
        let build = Build::new("plain").deck(DEFAULT_BASE);
        let found = build.found().expect("a deck");

        assert!(!ready(&found.url(1), &found, &Style::plain()).contains('\u{1b}'));
    }
}
