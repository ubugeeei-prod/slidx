//! Renders a directory of slide files into a directory of HTML pages.
//!
//! A stand-in for the Vite plugin until it exists, and the thing that produces
//! the screenshots in the README. It runs the real pipeline — parse, lint,
//! theme, render — so what it emits is what a built deck will be, not a mockup.
//!
//! ```sh
//! cargo run -p slidx_render --example preview -- examples/deck/slides dist/preview
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use slidx_core::{parse_deck, Deck, DeckParseOptions, Diagnostic, Slide};
use slidx_lint::{lint, LintInput, LintOptions};
use slidx_render::{render_slide, ShellOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source_dir = PathBuf::from(args.next().unwrap_or_else(|| "examples/deck/slides".into()));
    let out_dir = PathBuf::from(args.next().unwrap_or_else(|| "dist/preview".into()));

    let deck = read_deck(&source_dir)?;
    let theme = deck
        .meta
        .theme
        .as_deref()
        .and_then(slidx_theme::resolve)
        .unwrap_or_else(slidx_theme::default_theme);

    report(&deck, &theme, &source_dir);

    fs::create_dir_all(&out_dir)?;
    let options = ShellOptions { theme, ..ShellOptions::default() };

    for slide in &deck.slides {
        let html = render_slide(&deck, slide, &options);
        let path = out_dir.join(format!("{:04}-{}.html", slide.index + 1, slide.id));
        fs::write(&path, html)?;
        println!("  {}", path.display());
    }

    println!(
        "\n{} slide(s), {} stop(s) -> {}",
        deck.slides.len(),
        deck.stop_count(),
        out_dir.display()
    );
    Ok(())
}

/// Reads one deck from a directory of numbered slide files.
///
/// One file per slide is the recommended layout: it keeps diffs small, makes
/// reordering a rename, and lets two people edit different slides without
/// touching the same file. The files are concatenated with separators so the
/// existing parser stays the only thing that knows the deck format.
fn read_deck(dir: &Path) -> Result<Deck, Box<dyn std::error::Error>> {
    let mut sources: BTreeMap<String, String> = BTreeMap::new();

    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "md") {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            sources.insert(name, fs::read_to_string(&path)?);
        }
    }

    if sources.is_empty() {
        return Err(format!("no .md files in {}", dir.display()).into());
    }

    let joined = sources
        .values()
        .map(|source| source.trim().to_string())
        .collect::<Vec<_>>()
        .join("\n\n---\n");

    Ok(parse_deck(&joined, &DeckParseOptions::default()))
}

/// Prints the deck's diagnostics the way a build should.
///
/// `assets` is the slide directory, because that is what a relative image path
/// in a slide is relative to. Without it the rules that open a file have
/// nothing to open and stay quiet, which is the right behaviour in an editor
/// and the wrong one here.
fn report(deck: &Deck, theme: &slidx_theme::Theme, assets: &Path) {
    let surfaces = theme.surfaces();
    let findings =
        lint(&LintInput::new(deck, &surfaces).with_assets(assets), &LintOptions::default());

    for diagnostic in deck.diagnostics.iter().chain(findings.iter()) {
        println!("{}", format_diagnostic(diagnostic, deck));
    }

    if deck.diagnostics.is_empty() && findings.is_empty() {
        println!("no diagnostics\n");
    } else {
        println!();
    }
}

fn format_diagnostic(diagnostic: &Diagnostic, deck: &Deck) -> String {
    let location = diagnostic
        .span
        .slide_index
        .and_then(|index| deck.slides.get(index as usize))
        .map(Slide::display_title)
        .unwrap_or_else(|| "deck".to_string());

    let help =
        diagnostic.help.as_ref().map(|help| format!("\n    help: {help}")).unwrap_or_default();

    format!(
        "{}: [{}] {} ({location}){help}",
        diagnostic.severity.as_token(),
        diagnostic.code,
        diagnostic.message
    )
}
