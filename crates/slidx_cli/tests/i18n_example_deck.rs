//! `slidx i18n`, driven as a command over the deck in `examples/deck`.
//!
//! Every assertion here is about the real thing rather than a fixture, because
//! this repository has a documented history of features that were implemented,
//! tested, merged, and unreachable. A unit test over a string proves the
//! algorithm; this proves that typing the command at the deck the README shows
//! produces the file it claims to.
//!
//! The example deck is the right subject and not an arbitrary one. It carries
//! deck frontmatter, a Rust fence, a table, speaker notes, `autoSteps`, and
//! `[120ms]{#latency}[38ms]{#latency}` — which is the exact construct whose key
//! a translation must not touch.

use std::fs;
use std::path::{Path, PathBuf};

use slidx_cli::style::Style;
use slidx_cli::{Outcome, OK};
use slidx_core::{parse_deck, Deck, DeckParseOptions};

/// The deck the README is written about.
fn example_deck() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/deck/slides")
}

/// A scratch directory that cleans up after itself.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("slidx-i18n-deck-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(line: &str) -> Outcome {
    let argv: Vec<String> = line.split_whitespace().map(String::from).collect();
    slidx_cli::run(&argv, &Style::plain())
}

/// The deck as slidx reads it, from a directory of slide files.
fn read(directory: &Path) -> Deck {
    let mut names: Vec<PathBuf> = fs::read_dir(directory)
        .expect("a deck directory")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect();
    names.sort();

    let joined: Vec<String> = names
        .iter()
        .map(|path| fs::read_to_string(path).expect("a slide file").trim().to_string())
        .collect();

    parse_deck(&joined.join("\n\n---\n"), &DeckParseOptions::default())
}

/// Every file in a directory, by name, with its bytes.
fn files(directory: &Path) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = fs::read_dir(directory)
        .expect("a directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read_to_string(entry.path()).expect("a file"),
            )
        })
        .collect();

    found.sort();
    found
}

/// A catalogue translating every entry, with the placeholders left alone.
fn translate(po: &str) -> String {
    let mut out = String::new();
    let mut source = String::new();

    for line in po.lines() {
        if let Some(rest) = line.strip_prefix("msgid \"") {
            source = rest.trim_end_matches('"').to_string();
        }

        if line == "msgstr \"\"" && !source.is_empty() {
            out.push_str(&format!("msgstr \"これは{source}の訳です\"\n"));
            source.clear();
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    out
}

#[test]
fn extract_runs_over_the_example_deck_and_writes_a_catalogue() {
    let scratch = Scratch::new("extract");
    let catalogue = scratch.join("ja.po");

    let outcome = run(&format!(
        "i18n extract {} --lang ja --out {}",
        example_deck().display(),
        catalogue.display()
    ));

    let po = fs::read_to_string(&catalogue).expect("a catalogue");

    assert_eq!(outcome.code, OK, "{}", outcome.stderr);
    assert!(po.contains("\"Language: ja\\n\""), "{po}");
    assert!(po.contains("msgid \"Making Decks Fast\""), "{po}");
    assert!(po.contains("msgctxt \"deck/description\""), "{po}");
}

#[test]
fn nothing_the_example_deck_addresses_reaches_the_catalogue() {
    // The whole list, checked against the deck the README shows rather than
    // against a fixture built to pass. Every one of these is something a naive
    // pass would have handed a translator.
    let scratch = Scratch::new("protected");
    let catalogue = scratch.join("ja.po");

    run(&format!(
        "i18n extract {} --lang ja --out {}",
        example_deck().display(),
        catalogue.display()
    ));

    let po = fs::read_to_string(&catalogue).expect("a catalogue");
    let translatable: String =
        po.lines().filter(|line| line.starts_with("msgid ")).collect::<Vec<_>>().join("\n");

    for addressed in [
        "#latency",             // a mark key a `steps:` entry points at
        "timeline.frame",       // the body of a fenced code block
        "let frame",            // the same
        "minimal",              // the theme name
        "16:9",                 // the aspect
        "autoSteps",            // a frontmatter key
        "budget",               // another
        "slidx",                // the hashtag, which an audience searches for
    ] {
        assert!(!translatable.contains(addressed), "`{addressed}` was offered for translation");
    }

    // And the prose around them did reach it, so the check above is not passing
    // because nothing was extracted at all.
    assert!(translatable.contains("120ms"), "the marked text is content");
    assert!(translatable.contains("Latency dropped to"), "the sentence is content");
}

#[test]
fn applying_a_catalogue_that_translates_nothing_leaves_the_deck_byte_identical() {
    // The property everything else rests on, on the real deck. Not "renders the
    // same" — the same bytes, so a translation in progress can be applied at any
    // point without a diff nobody asked for.
    let scratch = Scratch::new("identical");
    let catalogue = scratch.join("ja.po");
    let out = scratch.join("slides.ja");

    run(&format!(
        "i18n extract {} --lang ja --out {}",
        example_deck().display(),
        catalogue.display()
    ));

    let outcome = run(&format!(
        "i18n apply {} --catalogue {} --out {}",
        example_deck().display(),
        catalogue.display(),
        out.display()
    ));

    assert_eq!(outcome.code, OK, "{}", outcome.stderr);
    assert_eq!(files(&out), files(&example_deck()));
}

#[test]
fn applying_a_full_translation_keeps_every_slide_id_and_every_mark_key() {
    // The two things that break silently. A slide id is a URL somebody pasted
    // into a chat and a QR code somebody printed; a mark key is what a `steps:`
    // entry addresses, and a deck whose key moved renders perfectly and does not
    // animate.
    let scratch = Scratch::new("translated");
    let catalogue = scratch.join("ja.po");
    let out = scratch.join("slides.ja");

    run(&format!(
        "i18n extract {} --lang ja --out {}",
        example_deck().display(),
        catalogue.display()
    ));
    let po = fs::read_to_string(&catalogue).expect("a catalogue");
    fs::write(&catalogue, translate(&po)).expect("a translated catalogue");

    let outcome = run(&format!(
        "i18n apply {} --catalogue {} --out {}",
        example_deck().display(),
        catalogue.display(),
        out.display()
    ));

    assert_eq!(outcome.code, OK, "{}", outcome.stderr);

    let before = read(&example_deck());
    let after = read(&out);

    assert_eq!(
        before.slides.iter().map(|slide| slide.id.clone()).collect::<Vec<_>>(),
        after.slides.iter().map(|slide| slide.id.clone()).collect::<Vec<_>>(),
    );

    for (one, two) in before.slides.iter().zip(&after.slides) {
        assert_eq!(one.marks, two.marks, "a mark changed on {}", one.id);
        assert_eq!(one.timeline.len(), two.timeline.len(), "stops changed on {}", one.id);
        assert_eq!(one.notes.len(), two.notes.len(), "notes changed on {}", one.id);
    }
}

#[test]
fn the_translated_deck_is_in_the_language_it_says_it_is() {
    // Everything the model needs to know two decks are the same talk: which
    // language each is in, and which one came first.
    let scratch = Scratch::new("lang");
    let catalogue = scratch.join("ja.po");
    let out = scratch.join("slides.ja");

    run(&format!(
        "i18n extract {} --lang ja --out {}",
        example_deck().display(),
        catalogue.display()
    ));
    let po = fs::read_to_string(&catalogue).expect("a catalogue");
    fs::write(&catalogue, translate(&po)).expect("a translated catalogue");

    run(&format!(
        "i18n apply {} --catalogue {} --out {}",
        example_deck().display(),
        catalogue.display(),
        out.display()
    ));

    let meta = read(&out).meta;

    assert_eq!(meta.lang.as_deref(), Some("ja"));
    assert!(meta.translation_of.is_some(), "the deck it came from is recorded");
    assert!(meta.title.as_deref().is_some_and(|title| title.contains('訳')));
}

#[test]
fn the_fenced_code_in_the_example_deck_comes_through_untouched() {
    // A translated code comment no longer matches the recording of the talk, and
    // the fence in this deck is the one the README quotes.
    let scratch = Scratch::new("fence");
    let catalogue = scratch.join("ja.po");
    let out = scratch.join("slides.ja");

    run(&format!(
        "i18n extract {} --lang ja --out {}",
        example_deck().display(),
        catalogue.display()
    ));
    let po = fs::read_to_string(&catalogue).expect("a catalogue");
    fs::write(&catalogue, translate(&po)).expect("a translated catalogue");

    run(&format!(
        "i18n apply {} --catalogue {} --out {}",
        example_deck().display(),
        catalogue.display(),
        out.display()
    ));

    let translated: String =
        files(&out).into_iter().map(|(_, body)| body).collect::<Vec<_>>().join("\n");

    assert!(translated.contains("let frame = timeline.frame(step)?;"), "{translated}");
    assert!(translated.contains("render(frame);"), "{translated}");
}

#[test]
fn a_new_command_completes_in_every_shell_without_anyone_writing_a_script() {
    // The completions are generated from the command table, so a command that
    // is in the table completes for free. Asserted rather than assumed, because
    // "for free" is exactly the kind of claim that stops being true quietly.
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let script = run(&format!("completions {shell}")).stdout;

        assert!(script.contains("i18n"), "{shell} does not complete `i18n`");
        assert!(script.contains("extract"), "{shell} does not complete `i18n extract`");

        // fish names a flag without its dashes — `-l catalogue` — and the other
        // three write it out. Both are the same flag.
        assert!(
            script.contains("--catalogue") || script.contains("-l catalogue"),
            "{shell} does not complete `--catalogue`"
        );
    }
}
