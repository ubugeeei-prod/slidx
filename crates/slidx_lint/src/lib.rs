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
pub mod geometry;
pub mod image;
pub mod rules;
pub mod surface;
pub mod typography;

mod markup;

#[cfg(test)]
mod test_support;

pub use color::{contrast_ratio, projected_contrast_ratio, ProjectorProfile, Rgba};
pub use geometry::{Insets, Side};
pub use image::{
    probe as probe_image, Format as ImageFormat, Intrinsic, Tolerance as ImageTolerance,
};
pub use surface::{Measurement, RenderTarget, Surface, TextSample};
pub use typography::{min_font_px, Legibility, TextRole, ViewingProfile};

use std::collections::BTreeMap;
use std::path::Path;

use slidx_core::{Deck, Diagnostics};

/// Where the linter learns an image's intrinsic size.
///
/// Two shapes because two callers, and the split is the same one
/// `slidx_doctor` makes: a rule reads, and something else does the IO. The CLI
/// has a filesystem and hands over a directory. Everything reached through
/// WebAssembly does not — a browser cannot open `./logo.png`, and pushing the
/// file's bytes across the boundary to learn two integers would be a strange
/// trade for a rule that runs on every build.
///
/// So the plugin reads the headers, calls `probeImage`, and passes the sizes.
/// One header parser either way, which is the point: two would eventually
/// disagree about a truncated JPEG.
#[derive(Debug, Clone, Copy)]
pub enum Assets<'a> {
    /// A directory the deck's relative paths resolve against.
    Directory(&'a Path),
    /// Sizes the caller already read, keyed by the path a slide writes.
    Measured(&'a BTreeMap<String, Intrinsic>),
}

/// Everything the rules read.
#[derive(Debug, Clone)]
pub struct LintInput<'a> {
    pub deck: &'a Deck,
    /// Resolved backgrounds and text, produced by whatever rendered the deck.
    pub surfaces: &'a [Surface],
    pub target: RenderTarget,
    /// Where the linter learns how big an image really is.
    ///
    /// `None` switches off every check that needs to know. That is the editor
    /// as the author types — a rule with nothing to measure says nothing
    /// rather than guessing.
    pub assets: Option<Assets<'a>>,
    /// The padding the renderer keeps content inside — the safe area it
    /// guarantees.
    ///
    /// `None` the same way `assets` is: the editor has rendered nothing yet, so
    /// there is no safe area to measure a room against, and a padding invented
    /// here would report bleed on a theme that has none.
    pub padding: Option<Insets>,
    /// What a browser found when it laid the built pages out.
    ///
    /// Empty everywhere no browser ran, which is most places. The rules that
    /// read it report nothing rather than approximating what it would have
    /// said.
    pub measured: &'a [Measurement],
}

impl<'a> LintInput<'a> {
    /// Reads the sizes itself, from a directory the deck's paths resolve
    /// against. For callers that have a filesystem.
    pub fn with_asset_directory(mut self, root: &'a Path) -> Self {
        self.assets = Some(Assets::Directory(root));
        self
    }

    /// Takes sizes the caller already read. For callers that do not.
    pub fn with_asset_sizes(mut self, sizes: &'a BTreeMap<String, Intrinsic>) -> Self {
        self.assets = Some(Assets::Measured(sizes));
        self
    }

    /// Builds an input at the deck's own aspect ratio.
    pub fn new(deck: &'a Deck, surfaces: &'a [Surface]) -> Self {
        Self {
            deck,
            surfaces,
            target: RenderTarget::from_dimensions(deck.meta.aspect.dimensions()),
            assets: None,
            padding: None,
            measured: &[],
        }
    }

    pub fn with_target(mut self, target: RenderTarget) -> Self {
        self.target = target;
        self
    }

    /// States the renderer's padding as a share of the slide's height.
    ///
    /// One number rather than four because a renderer that scales the slide as
    /// one piece has one: the shell resolves `--slidx-space-padding` in `cqh`
    /// and applies it to every side at once.
    pub fn with_padding(mut self, share_of_height: f64) -> Self {
        self.padding = Some(Insets::from_padding(share_of_height, self.target));
        self
    }

    pub fn with_measurements(mut self, measured: &'a [Measurement]) -> Self {
        self.measured = measured;
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
    /// What the room takes off the projected image, when the caller knows.
    ///
    /// Overrides the deck's own `safeArea:`, because whoever passes this is
    /// standing in the room and the deck was written before anyone had seen it.
    pub safe_area: Option<Insets>,
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

    surviving(sink, options)
}

/// Runs only the rules whose evidence is a browser measurement.
///
/// A separate entry point because a measurement arrives *after* the build that
/// already called [`lint`] — the pages have to exist before anything can open
/// them — and re-running the whole set there would report every other finding a
/// second time.
pub fn lint_measured(deck: &Deck, measured: &[Measurement], options: &LintOptions) -> Diagnostics {
    let surfaces: [Surface; 0] = [];
    let input = LintInput::new(deck, &surfaces).with_measurements(measured);

    let mut sink = Diagnostics::default();
    rules::overflow::check_measured(&input, options, &mut sink);

    surviving(sink, options)
}

fn surviving(sink: Diagnostics, options: &LintOptions) -> Diagnostics {
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
    fn a_measurement_pass_reports_only_what_it_measured() {
        // It runs after a build that already linted everything else, so a
        // second copy of those findings is the failure to avoid.
        let deck = deck_with_problems();
        let measured = [Measurement::new(0, 0).over(0.2, 0.0)];
        let found = lint_measured(&deck, &measured, &LintOptions::default());

        assert!(found.iter().all(|d| d.code == "overflow/clipped"), "got: {found:?}");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn a_measurement_pass_honours_the_same_suppression_as_a_build() {
        let deck = deck_with_problems();
        let measured = [Measurement::new(0, 0).over(0.2, 0.0)];
        let options = LintOptions { allow: vec!["overflow".into()], ..LintOptions::default() };

        assert!(lint_measured(&deck, &measured, &options).is_empty());
    }

    #[test]
    fn a_clean_deck_produces_nothing() {
        let deck = parse_deck("# One\n\n- a\n- b\n", &DeckParseOptions::default());
        let surfaces: Vec<Surface> = Vec::new();

        assert!(lint(&LintInput::new(&deck, &surfaces), &LintOptions::default()).is_empty());
    }
}
