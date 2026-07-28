//! Helpers shared by the rule tests.
//!
//! Rule tests should read as "given this deck, expect this diagnostic". These
//! helpers absorb the ceremony of building a `LintInput` so a test body stays
//! one setup line and one assertion.

use slidx_core::{parse_deck, DeckParseOptions, Diagnostics};

use crate::surface::{RenderTarget, Surface};
use crate::{lint, LintInput, LintOptions};

/// Lints a deck source with no surfaces, as the editor does while typing.
pub fn lint_deck(source: &str) -> Diagnostics {
    let deck = parse_deck(source, &DeckParseOptions::default());
    let surfaces: Vec<Surface> = Vec::new();

    lint(&LintInput::new(&deck, &surfaces), &LintOptions::default())
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
