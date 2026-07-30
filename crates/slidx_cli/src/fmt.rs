//! `slidx fmt` — the formatter, on a deck on disk.
//!
//! Nothing is decided here. What canonical means is [`slidx_fmt`]'s business,
//! and this module reads files, writes the ones that changed, and picks an exit
//! code. The same is true of the language server's `textDocument/formatting`,
//! which is why both can be trusted to agree.
//!
//! ## Why each file is formatted on its own
//!
//! `slidx lint` joins a directory of slide files into one deck, because a rule
//! about a time budget or a heading level needs the whole talk. Formatting does
//! not: every construct it touches is local to the file it is in. Joining first
//! and splicing afterwards would mean mapping byte ranges back through the
//! separators the join inserted — a second answer to what a file is, for no
//! gain.
//!
//! ## `--check` is the point
//!
//! It writes nothing and exits `1` when a file is not already formatted, which
//! is what makes this usable in somebody's CI. A file that could not be read
//! exits `2` — see the crate docs. A job that mistyped a path has to fail
//! differently from one whose deck needs formatting.

use std::fs;
use std::path::{Path, PathBuf};

use slidx_core::DeckParseOptions;

use crate::args::Matches;
use crate::lint::source;
use crate::report;
use crate::style::{Ink, Style};
use crate::{Outcome, FOUND, OK};

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let separator =
        matches.value("separator").map(str::to_string).unwrap_or_else(|| "---".to_string());
    let options = DeckParseOptions { separator: separator.clone(), ..DeckParseOptions::default() };

    let path = matches
        .first_positional()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source::DEFAULT_DIR));

    let files = match files(&path, &separator) {
        Ok(files) => files,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    let checking = matches.is_set("check");
    let mut changed = Vec::new();

    for file in &files {
        match reformat(file, &options, checking) {
            Ok(true) => changed.push(file.clone()),
            Ok(false) => {}
            Err(message) => return Outcome::misuse(format!("{message}\n")),
        }
    }

    let code = if checking && !changed.is_empty() { FOUND } else { OK };

    Outcome::out(render(&path.display().to_string(), &files, &changed, checking, style))
        .with_code(code)
}

/// The files a path names, in deck order.
///
/// One for a file, every `.md` for a directory — read through the same
/// [`source`] module `slidx lint` uses, so `slidx fmt` and `slidx lint` never
/// disagree about which files are part of a deck.
fn files(path: &Path, separator: &str) -> Result<Vec<PathBuf>, String> {
    let deck = source::read(path, separator)?;

    Ok(match deck.files.is_empty() {
        true => vec![path.to_path_buf()],
        false => deck.files,
    })
}

/// Formats one file, and returns whether it was not already formatted.
fn reformat(file: &Path, options: &DeckParseOptions, checking: bool) -> Result<bool, String> {
    let source = fs::read_to_string(file)
        .map_err(|error| format!("Could not read {}: {error}", file.display()))?;

    let edit = slidx_fmt::plan(&source, options);
    if edit.is_empty() {
        return Ok(false);
    }

    if !checking {
        fs::write(file, edit.apply(&source))
            .map_err(|error| format!("Could not write {}: {error}", file.display()))?;
    }

    Ok(true)
}

/// The report, as a person reads it.
fn render(
    label: &str,
    files: &[PathBuf],
    changed: &[PathBuf],
    checking: bool,
    style: &Style,
) -> String {
    let mut text = format!(
        "{} {}\n\n  {}\n",
        style.paint(Ink::Strong, "slidx fmt"),
        style.paint(Ink::Faint, label),
        verdict(files.len(), changed.len(), checking, style)
    );

    for file in changed {
        text.push('\n');
        text.push_str(&report::block(
            if checking { "changed" } else { "written" },
            if checking { Ink::Warn } else { Ink::Pass },
            &file.display().to_string(),
            if checking { "not formatted" } else { "formatted" },
            None,
            style,
        ));
    }

    text
}

/// The one line somebody reads if they read only one.
fn verdict(files: usize, changed: usize, checking: bool, style: &Style) -> String {
    let counted = format!("{files} file{}", if files == 1 { "" } else { "s" });

    if changed == 0 {
        return style.paint(Ink::Pass, format!("Nothing to change. {counted}, all formatted."));
    }

    let action = if checking { "would change" } else { "formatted" };

    format!("{} of {counted}.", style.paint(Ink::Warn, format!("{action} {changed}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory that cleans up after itself.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-fmt-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch directory");
            Self(path)
        }

        fn write(&self, name: &str, body: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, body).expect("write");
            path
        }

        fn read(&self, name: &str) -> String {
            fs::read_to_string(self.0.join(name)).expect("read")
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

    fn run_on(path: &Path, flags: &str) -> Outcome {
        let line = format!("fmt {} {flags}", path.display());
        let argv: Vec<String> = line.split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => run(&matches, &Style::plain()),
            other => panic!("expected a run, got {other:?}"),
        }
    }

    const UNFORMATTED: &str = "---\ntheme: minimal\ntitle: T\n---\n\n- a <!--step-->\n";
    const FORMATTED: &str = "---\ntitle: T\ntheme: minimal\n---\n\n- a <!-- step -->\n";

    #[test]
    fn a_file_that_needs_formatting_is_rewritten_and_the_run_still_passes() {
        // `fmt` did what it was asked. Exiting non-zero because it had work to
        // do would fail every pre-commit hook that runs it.
        let scratch = Scratch::new("write");
        let file = scratch.write("deck.md", UNFORMATTED);

        let outcome = run_on(&file, "");

        assert_eq!(scratch.read("deck.md"), FORMATTED);
        assert_eq!(outcome.code, OK);
        assert!(outcome.stdout.contains("formatted 1"), "{}", outcome.stdout);
    }

    #[test]
    fn check_writes_nothing_and_exits_non_zero() {
        // The form a CI job runs. Writing here would mean a green build had
        // silently changed the tree it was checking.
        let scratch = Scratch::new("check");
        let file = scratch.write("deck.md", UNFORMATTED);

        let outcome = run_on(&file, "--check");

        assert_eq!(scratch.read("deck.md"), UNFORMATTED, "--check wrote to the file");
        assert_eq!(outcome.code, FOUND);
        assert!(outcome.stdout.contains("deck.md"), "it does not say which file");
    }

    #[test]
    fn a_file_already_formatted_is_not_written_to_at_all() {
        // Not "written with the same bytes" — not written. A formatter that
        // touched every file would invalidate every build cache in the tree.
        let scratch = Scratch::new("clean");
        let file = scratch.write("deck.md", FORMATTED);
        let before = fs::metadata(&file).expect("metadata").modified().expect("mtime");

        let outcome = run_on(&file, "");

        assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), before);
        assert_eq!(outcome.code, OK);
        assert!(outcome.stdout.contains("all formatted"), "{}", outcome.stdout);
    }

    #[test]
    fn a_clean_deck_passes_check() {
        let scratch = Scratch::new("clean-check");
        let file = scratch.write("deck.md", FORMATTED);

        assert_eq!(run_on(&file, "--check").code, OK);
    }

    #[test]
    fn every_file_of_a_multi_file_deck_is_formatted_on_its_own() {
        // Joining them first would mean splicing byte ranges back out through
        // the separators the join inserted, and one file's frontmatter is the
        // deck's — so the join is not reversible.
        let scratch = Scratch::new("directory");
        scratch.write("0001.md", UNFORMATTED);
        scratch.write("0002.md", "---\nlayout: split\nbudget: 90s\n---\n\n# Two\n");

        let outcome = run_on(scratch.path(), "");

        assert_eq!(scratch.read("0001.md"), FORMATTED);
        assert_eq!(scratch.read("0002.md"), "---\nlayout: split\nbudget: 90s\n---\n\n# Two\n");
        assert_eq!(outcome.code, OK);
        assert!(outcome.stdout.contains("2 files"), "{}", outcome.stdout);
    }

    #[test]
    fn a_deck_that_is_not_there_exits_two_rather_than_reporting_a_clean_run() {
        // Exit 1 means "checked, and found something". A mistyped path that
        // exited 0 would report a formatted deck that was never opened.
        let outcome = run_on(Path::new("./no-such-deck"), "--check");

        assert_eq!(outcome.code, crate::MISUSE);
        assert!(outcome.stdout.is_empty());
    }

    #[test]
    fn a_custom_separator_reaches_the_formatter() {
        // A deck that shows Markdown source uses its own separator, and
        // normalising `---` inside it would split a slide the author did not.
        let scratch = Scratch::new("separator");
        let file = scratch.write("deck.md", "# One\n\n  ---  \n\n  ===  \n\n# Two\n");

        run_on(&file, "--separator ===");

        assert_eq!(scratch.read("deck.md"), "# One\n\n  ---  \n\n===\n\n# Two\n");
    }
}
