//! # slidx lint
//!
//! Checks for the slide failures that are invisible on a laptop and fatal on a
//! projector.
//!
//! Three ideas run through the rule set:
//!
//! **Check the room, not the monitor.** Contrast is evaluated through a model
//! of projector washout, and font size through the angular size a glyph
//! subtends from the back row. Both are calibrated against published venue
//! guidance, so they agree with the familiar rules of thumb where those apply
//! and keep working where they do not.
//!
//! **Say what to do.** Every diagnostic carries a stable code, a span, and a
//! concrete next action. A warning the author cannot act on is noise.
//!
//! **Run everywhere, unchanged.** The same rules run in the editor as the
//! author types, in `vite build`, and in `slidx doctor` at the venue. The
//! linter knows nothing about themes or CSS — it reads a [`Deck`] and a list
//! of resolved [`Surface`]s, which any renderer can produce.
//!
//! ```
//! use slidx_core::{parse_deck, DeckParseOptions};
//! use slidx_lint::{color, lint, LintInput, LintOptions, Surface, TextSample, TextRole};
//!
//! let deck = parse_deck("---\nduration: 5m\n---\n\n# Hello\n", &DeckParseOptions::default());
//! let surfaces = vec![Surface::new("theme", color::Rgba::WHITE).with_text(TextSample::new(
//!     TextRole::Body,
//!     color::parse("#767676").unwrap(),
//!     28.0,
//!     "theme.colorText",
//! ))];
//!
//! let report = lint(&LintInput::new(&deck, &surfaces), &LintOptions::default());
//!
//! // Passes at 4.5:1 on a monitor; flagged because a room will flatten it.
//! assert!(report.iter().any(|d| d.code == "contrast/projector"));
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod color;
pub mod image;
pub mod rules;
pub mod surface;
pub mod typography;

mod markup;

#[cfg(test)]
mod test_support;

pub use color::{contrast_ratio, projected_contrast_ratio, ProjectorProfile, Rgba};
pub use image::{Intrinsic, Tolerance as ImageTolerance};
pub use surface::{RenderTarget, Surface, TextSample};
pub use typography::{min_font_px, Legibility, TextRole, ViewingProfile};

use std::path::Path;

use slidx_core::{Deck, Diagnostics};

/// Everything the rules read.
#[derive(Debug, Clone)]
pub struct LintInput<'a> {
    pub deck: &'a Deck,
    /// Resolved backgrounds and text, produced by whatever rendered the deck.
    pub surfaces: &'a [Surface],
    pub target: RenderTarget,
    /// Directory the deck's relative asset paths resolve against.
    ///
    /// `None` switches off every check that has to open a file. That is the
    /// editor as the author types, and the browser, where there is no
    /// filesystem to read — a rule with nothing to measure says nothing rather
    /// than guessing.
    pub assets: Option<&'a Path>,
}

impl<'a> LintInput<'a> {
    /// Builds an input at the deck's own aspect ratio.
    pub fn new(deck: &'a Deck, surfaces: &'a [Surface]) -> Self {
        Self {
            deck,
            surfaces,
            target: RenderTarget::from_dimensions(deck.meta.aspect.dimensions()),
            assets: None,
        }
    }

    pub fn with_target(mut self, target: RenderTarget) -> Self {
        self.target = target;
        self
    }

    pub fn with_assets(mut self, root: &'a Path) -> Self {
        self.assets = Some(root);
        self
    }
}

/// How strictly, and for what room.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LintOptions {
    /// The room the deck will be shown in.
    pub projector: ProjectorProfile,
    /// How far away the back row is.
    pub viewing: ViewingProfile,
    /// How soft and how stretched an image may be before it is worth saying so.
    pub images: ImageTolerance,
    /// Codes to suppress. A group name suppresses everything under it, so
    /// `"contrast"` covers `contrast/too-low` and `contrast/projector` alike.
    pub allow: Vec<String>,
    /// Adds advisory checks that are correct but not always worth acting on.
    pub strict: bool,
}

impl LintOptions {
    /// True when `code` is suppressed by this configuration.
    fn suppresses(&self, code: &str) -> bool {
        self.allow.iter().any(|allowed| {
            code == allowed
                || code.strip_prefix(allowed.as_str()).is_some_and(|rest| rest.starts_with('/'))
        })
    }
}

/// Runs every rule and returns the diagnostics that survive suppression.
pub fn lint(input: &LintInput<'_>, options: &LintOptions) -> Diagnostics {
    let mut sink = Diagnostics::default();

    for (_, rule) in rules::ALL {
        rule(input, options, &mut sink);
    }

    if options.allow.is_empty() {
        return sink;
    }

    sink.into_iter().filter(|diagnostic| !options.suppresses(&diagnostic.code)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn deck_with_problems() -> Deck {
        parse_deck("# One\n\n![](./a.png)\n\n---\n\n### Skipped\n", &DeckParseOptions::default())
    }

    fn run(allow: &[&str]) -> Diagnostics {
        let deck = deck_with_problems();
        let surfaces: Vec<Surface> = Vec::new();
        let options = LintOptions {
            allow: allow.iter().map(|code| code.to_string()).collect(),
            ..LintOptions::default()
        };

        lint(&LintInput::new(&deck, &surfaces), &options)
    }

    #[test]
    fn rules_run_over_a_deck_with_no_surfaces() {
        // The editor lints while the author types, before anything is rendered.
        assert!(!run(&[]).is_empty());
    }

    #[test]
    fn an_exact_code_is_suppressed() {
        assert!(run(&[]).iter().any(|d| d.code == "structure/missing-alt"));
        assert!(run(&["structure/missing-alt"]).iter().all(|d| d.code != "structure/missing-alt"));
    }

    #[test]
    fn a_group_name_suppresses_every_code_under_it() {
        assert!(run(&["structure"]).is_empty());
    }

    #[test]
    fn a_prefix_that_is_not_a_group_boundary_does_not_suppress() {
        // `struct` must not swallow `structure/*`.
        assert!(!run(&["struct"]).is_empty());
    }

    #[test]
    fn the_render_target_follows_the_deck_aspect_ratio() {
        let deck = parse_deck("---\naspect: \"4:3\"\n---\n\n# One\n", &DeckParseOptions::default());
        let surfaces: Vec<Surface> = Vec::new();
        let input = LintInput::new(&deck, &surfaces);

        assert_eq!(input.target.width_px, 1440.0);
    }

    #[test]
    fn the_render_target_can_be_overridden() {
        let deck = deck_with_problems();
        let surfaces: Vec<Surface> = Vec::new();
        let input = LintInput::new(&deck, &surfaces)
            .with_target(RenderTarget { width_px: 960.0, height_px: 540.0 });

        assert_eq!(input.target.height_px, 540.0);
    }

    #[test]
    fn a_clean_deck_produces_nothing() {
        let deck = parse_deck("# One\n\n- a\n- b\n", &DeckParseOptions::default());
        let surfaces: Vec<Surface> = Vec::new();

        assert!(lint(&LintInput::new(&deck, &surfaces), &LintOptions::default()).is_empty());
    }
}
