//! Helpers shared by the rule tests.
//!
//! Rule tests should read as "given this deck, expect this diagnostic". These
//! helpers absorb the ceremony of building a `LintInput` so a test body stays
//! one setup line and one assertion.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use slidx_core::{parse_deck, DeckParseOptions, Diagnostics};

use crate::surface::{RenderTarget, Surface};
use crate::{lint, LintInput, LintOptions};

/// Lints a deck source with no surfaces, as the editor does while typing.
pub fn lint_deck(source: &str) -> Diagnostics {
    let deck = parse_deck(source, &DeckParseOptions::default());
    let surfaces: Vec<Surface> = Vec::new();

    lint(&LintInput::new(&deck, &surfaces), &LintOptions::default())
}

/// Lints a deck the way a renderer would, having stated its padding.
///
/// `padding` is a share of the slide's height, matching what a theme resolves
/// `--slidx-space-padding` to.
pub fn lint_deck_rendered(source: &str, padding: f64) -> Diagnostics {
    lint_deck_in_room(source, padding, |_| {})
}

/// The same, with the room configured.
pub fn lint_deck_in_room(
    source: &str,
    padding: f64,
    configure: impl FnOnce(&mut LintOptions),
) -> Diagnostics {
    let deck = parse_deck(source, &DeckParseOptions::default());
    let surfaces: Vec<Surface> = Vec::new();
    let mut options = LintOptions::default();
    configure(&mut options);

    lint(&LintInput::new(&deck, &surfaces).with_padding(padding), &options)
}

/// Lints a deck against what a browser measured of the built pages.
pub fn lint_deck_measured(source: &str, measured: &[crate::Measurement]) -> Diagnostics {
    let deck = parse_deck(source, &DeckParseOptions::default());
    let surfaces: Vec<Surface> = Vec::new();

    lint(&LintInput::new(&deck, &surfaces).with_measurements(measured), &LintOptions::default())
}

/// Lints surfaces against an empty deck, at the default render target.
pub fn lint_surfaces(
    surfaces: Vec<Surface>,
    configure: impl FnOnce(&mut LintOptions),
) -> Diagnostics {
    lint_surfaces_with_target(surfaces, RenderTarget::default(), configure)
}

/// Lints surfaces at a specific render target.
pub fn lint_surfaces_with_target(
    surfaces: Vec<Surface>,
    target: RenderTarget,
    configure: impl FnOnce(&mut LintOptions),
) -> Diagnostics {
    let deck = parse_deck("", &DeckParseOptions::default());
    let mut options = LintOptions::default();
    configure(&mut options);

    lint(&LintInput::new(&deck, &surfaces).with_target(target), &options)
}

/// A directory of real files on disk, and decks linted against it.
///
/// The rules that read images can only be tested against a filesystem, because
/// what they assert is that a header on disk disagrees with a slide. Each
/// instance owns a directory of its own so the suite can keep running in
/// parallel, and removes it on drop so a failed test leaves nothing behind.
#[derive(Debug)]
pub struct Assets {
    root: PathBuf,
}

impl Assets {
    pub fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);

        let name =
            format!("slidx-lint-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
        let root = std::env::temp_dir().join(name);
        fs::create_dir_all(&root).expect("a scratch directory");

        Self { root }
    }

    /// Writes one file, creating any directories its name implies.
    pub fn with(self, name: &str, bytes: &[u8]) -> Self {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("a scratch directory");
        }
        fs::write(path, bytes).expect("a scratch file");

        self
    }

    /// Lints a deck whose assets are this directory, the way a build does.
    pub fn lint(&self, source: &str) -> Diagnostics {
        self.lint_with(source, |_| {})
    }

    pub fn lint_with(&self, source: &str, configure: impl FnOnce(&mut LintOptions)) -> Diagnostics {
        let deck = parse_deck(source, &DeckParseOptions::default());
        let surfaces: Vec<Surface> = Vec::new();
        let mut options = LintOptions::default();
        configure(&mut options);

        lint(&LintInput::new(&deck, &surfaces).with_asset_directory(&self.root), &options)
    }
}

impl Drop for Assets {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
