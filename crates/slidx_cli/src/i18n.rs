//! `slidx i18n` — the same talk in another language.
//!
//! Two commands with a person in between them, and the gap is deliberate.
//! `extract` writes a catalogue, somebody translates it with whatever tool they
//! trust, and `apply` writes the translated deck. slidx never translates: a
//! build that called a translation service would be neither offline nor
//! deterministic, and a tool holding an API key is a tool that has to be
//! trusted with one — the same boundary `slidx publish` holds against a
//! credential.
//!
//! There is no build-time half of this and there must not be one. `slidx i18n`
//! is something an author runs on purpose and commits the result of.
//!
//! ## Writing a deck back to the files it came from
//!
//! A deck is read as one joined source, because slide ids are allocated across
//! the whole deck and a file on its own does not know whether its heading
//! collides with another file's. It is written back one file at a time, because
//! `slides.ja/0001.md` next to `slides/0001.md` is a translation change a
//! reviewer can read line for line.
//!
//! Joining trims each file, so the whitespace a file opened and closed with is
//! put back around the bytes that came out. That is what makes applying a
//! catalogue nobody has filled in produce files identical to their originals,
//! byte for byte, rather than files that merely say the same thing.

use std::fs;
use std::path::{Path, PathBuf};

use slidx_core::DeckParseOptions;
use slidx_i18n::{Catalogue, Plan};

use crate::args::Matches;
use crate::lint::source::{self, DeckSource};
use crate::report::{self, INDENT};
use crate::style::{Ink, Style};
use crate::{Outcome, FOUND, OK};

pub fn run(action: &str, matches: &Matches, style: &Style) -> Outcome {
    match action {
        "extract" => extract(matches, style),
        "apply" => apply(matches, style),
        // Unreachable while the table and this match agree, which the suite
        // asserts.
        other => Outcome::misuse(format!("`slidx i18n {other}` is declared but not wired up.\n")),
    }
}

/// `slidx i18n extract` — write the catalogue.
fn extract(matches: &Matches, style: &Style) -> Outcome {
    let Some(lang) = matches.value("lang") else {
        return Outcome::misuse(needs_a_language());
    };

    let (deck, options) = match read(matches) {
        Ok(read) => read,
        Err(outcome) => return outcome,
    };

    let out = matches.value("out").map(PathBuf::from);

    // Reading the file we are about to write is the whole point: a translator's
    // work has to survive the author fixing a typo on slide one.
    let previous = out
        .as_ref()
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|text| Catalogue::from_po(&text));

    let mut catalogue =
        slidx_i18n::catalogue(&deck.source, &options, lang, previous.as_ref());
    catalogue.deck = deck.label.clone();

    let po = catalogue.to_po();

    let Some(path) = out else {
        return Outcome::out(po);
    };

    if let Err(error) = write(&path, &po) {
        return Outcome::misuse(error);
    }

    Outcome::out(extracted(&catalogue, &path, previous.is_some(), style))
}

/// `slidx i18n apply` — write the translated deck.
fn apply(matches: &Matches, style: &Style) -> Outcome {
    let Some(catalogue_path) = matches.value("catalogue") else {
        return Outcome::misuse(needs("--catalogue", "the translated PO file"));
    };
    let Some(out) = matches.value("out").map(PathBuf::from) else {
        return Outcome::misuse(needs("--out", "a directory for the translated deck"));
    };

    let (deck, options) = match read(matches) {
        Ok(read) => read,
        Err(outcome) => return outcome,
    };

    let catalogue = match fs::read_to_string(catalogue_path) {
        Ok(text) => Catalogue::from_po(&text),
        Err(error) => {
            return Outcome::misuse(format!("Could not read {catalogue_path}: {error}\n"))
        }
    };

    let plan = slidx_i18n::plan(&deck.source, &options, &catalogue);

    if matches.is_set("plan") {
        return report(&plan, &deck, &out, true, style);
    }

    if let Err(error) = write_deck(&deck, &plan, &out) {
        return Outcome::misuse(error);
    }

    report(&plan, &deck, &out, false, style)
}

/// The deck named on the command line, and how to parse it.
fn read(matches: &Matches) -> Result<(DeckSource, DeckParseOptions), Outcome> {
    let separator =
        matches.value("separator").map(str::to_string).unwrap_or_else(|| "---".to_string());

    let path = matches
        .first_positional()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source::DEFAULT_DIR));

    match source::read(&path, &separator) {
        Ok(deck) => {
            Ok((deck, DeckParseOptions { separator, ..DeckParseOptions::default() }))
        }
        Err(message) => Err(Outcome::misuse(format!("{message}\n"))),
    }
}

/// Writes the translated deck, one file per file that was read.
///
/// The whitespace the join trimmed off each file goes back around the bytes
/// that came out of it, so a file nothing translated is written back exactly as
/// it was rather than approximately.
fn write_deck(deck: &DeckSource, plan: &Plan, out: &Path) -> Result<(), String> {
    let mut marks: Vec<usize> =
        deck.files.iter().flat_map(|file| [file.joined.start, file.joined.end]).collect();

    let translated = plan.apply_tracking(&deck.source, &mut marks);

    fs::create_dir_all(out).map_err(|error| unwritable(out, &error))?;

    if deck.files.is_empty() {
        let name = Path::new(&deck.label).file_name().unwrap_or_else(|| "deck.md".as_ref());
        return write(&out.join(name), &translated);
    }

    for (index, file) in deck.files.iter().enumerate() {
        let original = fs::read_to_string(&file.path)
            .map_err(|error| format!("Could not read {}: {error}", file.path.display()))?;

        let body = translated.get(marks[index * 2]..marks[index * 2 + 1]).unwrap_or_default();
        let opened = &original[..file.offset.min(original.len())];
        let closed = &original[(file.offset + file.joined.len()).min(original.len())..];

        let name = file.path.file_name().unwrap_or_else(|| "slide.md".as_ref());
        write(&out.join(name), &format!("{opened}{body}{closed}"))?;
    }

    Ok(())
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| unwritable(parent, &error))?;
    }

    fs::write(path, text).map_err(|error| unwritable(path, &error))
}

fn extracted(catalogue: &Catalogue, path: &Path, merged: bool, style: &Style) -> String {
    let total = catalogue.entries.len();
    let done = total - catalogue.untranslated();

    let mut text = format!(
        "{}\n",
        style.paint(Ink::Strong, &format!("{} string(s) to {}", total, catalogue.lang))
    );

    text.push_str(&report::block(
        "WROTE",
        Ink::Pass,
        &path.display().to_string(),
        &if merged {
            format!("{done} of {total} translations carried over from the catalogue that was there")
        } else {
            "a fresh catalogue".to_string()
        },
        None,
        style,
    ));

    text.push_str(&report::flowed(
        "Translate the msgstr lines, keeping every %1, %2 … where the grammar wants them, then \
         run `slidx i18n apply`.",
        INDENT,
        Ink::Strong,
        style,
    ));

    text
}

/// What applying did, or would do.
fn report(plan: &Plan, deck: &DeckSource, out: &Path, dry: bool, style: &Style) -> Outcome {
    let mut text = format!(
        "{}\n",
        style.paint(
            Ink::Strong,
            &format!("{} translated, {} left in the original language", plan.translated, plan.untranslated)
        )
    );

    let verb = if dry { "WOULD" } else { "WROTE" };
    text.push_str(&report::block(
        verb,
        Ink::Pass,
        &out.display().to_string(),
        &format!("{} file(s) from {}", deck.file_count(), deck.label),
        None,
        style,
    ));

    for id in plan.pinned_ids() {
        text.push_str(&report::block(
            "PINNED",
            Ink::Warn,
            id,
            "The translated heading would have moved this slide, so its id is written out. \
             Every link and QR code pointing at it keeps working.",
            None,
            style,
        ));
    }

    for problem in &plan.problems {
        text.push_str(&report::block("REFUSED", Ink::Fail, "catalogue", &problem.to_string(), None, style));
    }

    // Neither of these can be carried across, and saying nothing would let an
    // author assume both had been.
    text.push_str(&report::block(
        "TODO",
        Ink::Warn,
        "budget and overflow",
        "Per-slide budgets do not transfer — speaking rate is not language independent. Nor does \
         the linter's overflow verdict: a slide that fitted in one language may not in another.",
        Some(&format!("run `slidx lint {}` before you rehearse it", out.display())),
        style,
    ));

    let code = if plan.problems.is_empty() { OK } else { FOUND };
    Outcome::out(text).with_code(code)
}

fn needs_a_language() -> String {
    "`slidx i18n extract` needs the language it is extracting for.\n\n\
     It is a BCP 47 tag, and it goes into the catalogue's header and the translated deck's \
     `lang:`.\n\n\
     For example:\n\n  slidx i18n extract slides --lang ja --out i18n/ja.po\n"
        .to_string()
}

fn needs(flag: &str, what: &str) -> String {
    format!(
        "`slidx i18n apply` needs {flag}: {what}.\n\n\
         For example:\n\n  slidx i18n apply slides --catalogue i18n/ja.po --out slides.ja\n"
    )
}

fn unwritable(path: &Path, error: &std::io::Error) -> String {
    format!("Could not write {}: {error}\n", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A scratch directory that cleans up after itself.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("slidx-i18n-{name}-{}", std::process::id()));
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

    fn run_line(line: &str) -> Outcome {
        let argv: Vec<String> = line.split_whitespace().map(String::from).collect();
        crate::run(&argv, &Style::plain())
    }

    #[test]
    fn extract_writes_a_po_file_a_translation_tool_can_open() {
        let scratch = Scratch::new("extract");
        scratch.write("0001.md", "---\ntitle: Fast Decks\n---\n\n# Fast Decks\n");

        let outcome = run_line(&format!("i18n extract {} --lang ja", scratch.path().display()));

        assert_eq!(outcome.code, OK, "{}", outcome.stderr);
        assert!(outcome.stdout.contains("msgctxt \"deck/title\""), "{}", outcome.stdout);
        assert!(outcome.stdout.contains("\"Language: ja\\n\""), "{}", outcome.stdout);
    }

    #[test]
    fn extract_without_a_language_says_what_to_type() {
        let outcome = run_line("i18n extract");

        assert_eq!(outcome.code, crate::MISUSE);
        assert!(outcome.stderr.contains("--lang ja"), "{}", outcome.stderr);
    }

    #[test]
    fn applying_a_catalogue_that_translates_nothing_leaves_every_file_byte_identical() {
        // The property the whole design rests on. Not "says the same thing" —
        // the same bytes, including the blank line this file opens with and the
        // one it does not end with.
        let scratch = Scratch::new("identical");
        let deck = scratch.path().join("slides");
        fs::create_dir_all(&deck).expect("deck directory");

        let files = [("0001.md", "\n---\ntitle: T\n---\n\n# One\n\n"), ("0002.md", "# Two")];
        for (name, body) in files {
            fs::write(deck.join(name), body).expect("write");
        }

        let catalogue = scratch.write("empty.po", "msgid \"\"\nmsgstr \"Language: ja\\n\"\n");
        let out = scratch.path().join("slides.ja");

        let outcome = run_line(&format!(
            "i18n apply {} --catalogue {} --out {}",
            deck.display(),
            catalogue.display(),
            out.display()
        ));

        assert_eq!(outcome.code, OK, "{}", outcome.stderr);
        for (name, body) in files {
            assert_eq!(fs::read_to_string(out.join(name)).expect("written"), body, "{name}");
        }
    }

    #[test]
    fn applying_a_translation_writes_a_sibling_deck_and_keeps_the_slide_ids() {
        let scratch = Scratch::new("sibling");
        let deck = scratch.path().join("slides");
        fs::create_dir_all(&deck).expect("deck directory");
        fs::write(deck.join("0001.md"), "# Fast Decks\n").expect("write");

        let catalogue = scratch.write(
            "ja.po",
            "msgid \"\"\nmsgstr \"Language: ja\\n\"\n\n\
             msgctxt \"fast-decks/heading\"\nmsgid \"Fast Decks\"\nmsgstr \"速いデッキ\"\n",
        );
        let out = scratch.path().join("slides.ja");

        let outcome = run_line(&format!(
            "i18n apply {} --catalogue {} --out {}",
            deck.display(),
            catalogue.display(),
            out.display()
        ));

        let written = fs::read_to_string(out.join("0001.md")).expect("written");

        assert_eq!(outcome.code, OK, "{}", outcome.stderr);
        assert!(written.contains("速いデッキ"), "{written}");
        assert!(written.contains("id: fast-decks"), "{written}");
        assert!(outcome.stdout.contains("PINNED"), "{}", outcome.stdout);
    }

    #[test]
    fn a_dry_run_says_what_would_happen_and_writes_nothing() {
        let scratch = Scratch::new("dry");
        let deck = scratch.path().join("slides");
        fs::create_dir_all(&deck).expect("deck directory");
        fs::write(deck.join("0001.md"), "# One\n").expect("write");

        let catalogue = scratch.write("ja.po", "msgid \"\"\nmsgstr \"Language: ja\\n\"\n");
        let out = scratch.path().join("nowhere");

        let outcome = run_line(&format!(
            "i18n apply {} --catalogue {} --out {} --plan",
            deck.display(),
            catalogue.display(),
            out.display()
        ));

        assert!(outcome.stdout.contains("WOULD"), "{}", outcome.stdout);
        assert!(!out.exists(), "a dry run created {}", out.display());
    }

    #[test]
    fn a_translation_that_dropped_a_mark_key_is_refused_and_exits_non_zero() {
        // Exit 1 rather than 0: in CI the difference is between "your
        // translation has a problem" and a deck whose animation quietly stopped.
        let scratch = Scratch::new("refused");
        let deck = scratch.path().join("slides");
        fs::create_dir_all(&deck).expect("deck directory");
        fs::write(deck.join("0001.md"), "Latency dropped to [120ms]{#latency}.\n").expect("write");

        let catalogue = scratch.write(
            "ja.po",
            "msgid \"\"\nmsgstr \"Language: ja\\n\"\n\n\
             msgctxt \"slide-1/body/1\"\nmsgid \"Latency dropped to [120ms]%1.\"\n\
             msgstr \"レイテンシが下がりました。\"\n",
        );
        let out = scratch.path().join("slides.ja");

        let outcome = run_line(&format!(
            "i18n apply {} --catalogue {} --out {}",
            deck.display(),
            catalogue.display(),
            out.display()
        ));

        assert_eq!(outcome.code, FOUND);
        assert!(outcome.stdout.contains("%1"), "{}", outcome.stdout);
        assert!(
            fs::read_to_string(out.join("0001.md")).expect("written").contains("{#latency}"),
            "the untranslatable entry was left alone"
        );
    }

    #[test]
    fn re_extracting_over_a_catalogue_keeps_the_translations_already_in_it() {
        // Otherwise the command is usable exactly once.
        let scratch = Scratch::new("merge");
        let deck = scratch.path().join("slides");
        fs::create_dir_all(&deck).expect("deck directory");
        fs::write(deck.join("0001.md"), "# One\n\nBody.\n").expect("write");

        let catalogue = scratch.write(
            "ja.po",
            "msgid \"\"\nmsgstr \"Language: ja\\n\"\n\n\
             msgctxt \"one/heading\"\nmsgid \"One\"\nmsgstr \"一\"\n",
        );

        run_line(&format!(
            "i18n extract {} --lang ja --out {}",
            deck.display(),
            catalogue.display()
        ));

        let text = fs::read_to_string(&catalogue).expect("rewritten");
        assert!(text.contains("msgstr \"一\""), "{text}");
        assert!(text.contains("msgid \"Body.\""), "{text}");
    }

    #[test]
    fn applying_always_says_what_a_translation_cannot_carry_across() {
        // Budgets and the overflow verdict. Silence would let an author assume
        // both had come with the words.
        let scratch = Scratch::new("todo");
        let deck = scratch.path().join("slides");
        fs::create_dir_all(&deck).expect("deck directory");
        fs::write(deck.join("0001.md"), "# One\n").expect("write");
        let catalogue = scratch.write("ja.po", "msgid \"\"\nmsgstr \"Language: ja\\n\"\n");

        let outcome = run_line(&format!(
            "i18n apply {} --catalogue {} --out {} --plan",
            deck.display(),
            catalogue.display(),
            scratch.path().join("out").display()
        ));

        assert!(outcome.stdout.contains("budget"), "{}", outcome.stdout);
        assert!(outcome.stdout.contains("slidx lint"), "{}", outcome.stdout);
    }
}
