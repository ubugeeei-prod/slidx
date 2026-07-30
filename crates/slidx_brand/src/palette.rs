//! The brand's colours, and the three jobs they do.
//!
//! Three roles carry the palette, and naming them by job rather than by hue is
//! what stops the fourth blue being added next year:
//!
//! - **paper** is what the brand is drawn *on*.
//! - **ink** is what words are set *in*.
//! - **signal** is the only colour allowed to *mean* something — a link, the
//!   accent rule, the pages in the mark. Nothing decorative may use it, because
//!   a colour used for decoration cannot also be used for emphasis.
//!
//! Two supporting roles exist because a page needs them and inventing them at
//! each call site is how a palette grows: **muted** for secondary text, and
//! **line** for a hairline.
//!
//! Both schemes are declared, for the same reason every built-in theme declares
//! both: the room is not knowable from here.
//!
//! # Why these hexes and not ones that look the same
//!
//! Every pair here is run through [`slidx_lint`] — the deck linter, including
//! its projector-washout model — in [`crate::audit`]. That is not ceremony. The
//! obvious blue for this palette was the default theme's accent, `#1d4ed8`, and
//! it measures 4.46:1 on this paper in a bright room: a fail, by four
//! hundredths, on the exact check slidx exists to run. The signal here is a
//! stop deeper for that reason and nothing else.

use serde::{Deserialize, Serialize};
use slidx_lint::{Rgba, Surface, TextRole, TextSample};

/// Which variant of the brand is in use.
///
/// Deliberately the same two-value shape as [`slidx_theme::Scheme`] rather than
/// a reuse of it: a brand asset is not a slide, and coupling the two would mean
/// a deck theme could not gain a third scheme without changing the brand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scheme {
    Light,
    Dark,
}

impl Scheme {
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];
}

/// The five colours the brand is built from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    /// The background the brand is drawn on.
    ///
    /// Not pure white, and not pure black. `contrast()` in [`slidx_theme`]
    /// documents the reason and it applies here too: at full separation the
    /// edges of a glyph halate, which makes text harder to read rather than
    /// easier.
    pub paper: Rgba,
    /// Words.
    pub ink: Rgba,
    /// Secondary words: captions, attributions, the line under a heading.
    pub muted: Rgba,
    /// The one colour that carries meaning.
    pub signal: Rgba,
    /// A hairline. Never text, so it is the one role with no contrast floor —
    /// a rule that met one would be a border loud enough to read as a bar.
    pub line: Rgba,
}

/// Parses a hex colour from this crate's own source.
///
/// Panics on a malformed literal, which is right: these are constants in this
/// file, so a bad one is a bug rather than user input.
pub(crate) fn hex(text: &str) -> Rgba {
    slidx_lint::color::parse(text).unwrap_or_else(|| panic!("brand colour `{text}` is malformed"))
}

/// Ink on paper, and a signal deep enough to survive a bright room.
pub fn light() -> Palette {
    Palette {
        paper: hex("#fbfbfc"),
        ink: hex("#101014"),
        muted: hex("#4b4b55"),
        signal: hex("#1b3bc9"),
        line: hex("#d8d8de"),
    }
}

/// The same palette with paper and ink exchanged, and the signal lifted.
///
/// A dark scheme cannot simply invert: `#1b3bc9` on near-black measures 1.9:1,
/// so the signal has to move to the other side of the paper it now sits on.
pub fn dark() -> Palette {
    Palette {
        paper: hex("#0b0b0d"),
        ink: hex("#f2f2f5"),
        muted: hex("#a6a6b2"),
        signal: hex("#b8ccff"),
        line: hex("#2b2b33"),
    }
}

pub fn of(scheme: Scheme) -> Palette {
    match scheme {
        Scheme::Light => light(),
        Scheme::Dark => dark(),
    }
}

impl Palette {
    /// Describes the palette to the deck linter.
    ///
    /// Two surfaces, because the brand has two backgrounds anything is ever
    /// drawn on: paper, and a signal fill with paper text on it. A filled
    /// button is the second one, and a brand that failed there would fail in
    /// public on the first page of its own documentation.
    ///
    /// `font_px` comes from the brand type scale rather than a slide's, since
    /// that is the size these colours are actually set at.
    pub fn surfaces(&self, scheme: Scheme, base_px: f64) -> Vec<Surface> {
        let name = |part: &str| format!("brand / {} / {part}", scheme.as_token());

        let paper = Surface::new(name("paper"), self.paper)
            .with_text(TextSample::new(TextRole::Body, self.ink, base_px, "brand.ink"))
            .with_text(TextSample::new(TextRole::Caption, self.muted, base_px, "brand.muted"))
            .with_text(TextSample::new(TextRole::Body, self.signal, base_px, "brand.signal"))
            // The mark's pages are signal on paper, and they are judged as body
            // text on purpose. A favicon at 16 pixels is smaller than any glyph
            // the legibility model covers, so the shape needs at least as much
            // separation as a word does — never less.
            .with_text(TextSample::new(TextRole::Body, self.signal, base_px, "brand.mark.pages"));

        let filled = Surface::new(name("signal"), self.signal).with_text(TextSample::new(
            TextRole::Body,
            self.paper,
            base_px,
            "brand.onSignal",
        ));

        vec![paper, filled]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_schemes_declare_every_role() {
        // A role missing from a scheme is a colour a caller resolves from the
        // other one, which is how a dark page ends up with light-mode ink.
        for scheme in Scheme::ALL {
            let palette = of(scheme);
            for value in [palette.paper, palette.ink, palette.muted, palette.signal, palette.line] {
                assert_eq!(value.a, 1.0, "{} declares a translucent role", scheme.as_token());
            }
        }
    }

    #[test]
    fn the_dark_scheme_is_actually_darker() {
        assert!(dark().paper.relative_luminance() < light().paper.relative_luminance());
        assert!(dark().ink.relative_luminance() > light().ink.relative_luminance());
    }

    #[test]
    fn neither_scheme_reaches_full_black_on_full_white() {
        // Halation at full separation makes text harder to read, not easier —
        // the reason the high-contrast deck theme stops short of it too.
        assert_ne!(light().paper, Rgba::WHITE);
        assert_ne!(light().ink, Rgba::BLACK);
        assert_ne!(dark().paper, Rgba::BLACK);
        assert_ne!(dark().ink, Rgba::WHITE);
    }

    #[test]
    fn the_signal_is_the_only_role_that_is_not_a_neutral() {
        // "Which colour carries meaning" has to have exactly one answer, or the
        // answer is none of them.
        let chromatic = |color: Rgba| {
            let [r, g, b] = [color.r, color.g, color.b].map(i16::from);
            (r - g).abs().max((g - b).abs()).max((r - b).abs()) > 24
        };

        for scheme in Scheme::ALL {
            let palette = of(scheme);
            assert!(chromatic(palette.signal), "{} has no signal", scheme.as_token());

            for (role, value) in
                [("paper", palette.paper), ("ink", palette.ink), ("muted", palette.muted)]
            {
                assert!(!chromatic(value), "{}.{role} competes with the signal", scheme.as_token());
            }
        }
    }

    #[test]
    fn every_role_that_carries_words_is_described_to_the_linter() {
        // A role absent from this list is a role nobody checks, which is the
        // gap `slidx_theme::palette::pairs` exists to close on its own side.
        let described: Vec<String> = light()
            .surfaces(Scheme::Light, 17.0)
            .into_iter()
            .flat_map(|surface| surface.text)
            .map(|sample| sample.origin)
            .collect();

        assert_eq!(
            described,
            vec!["brand.ink", "brand.muted", "brand.signal", "brand.mark.pages", "brand.onSignal"]
        );
    }

    #[test]
    fn a_filled_signal_is_checked_as_its_own_background() {
        let surfaces = dark().surfaces(Scheme::Dark, 17.0);
        let filled = surfaces.iter().find(|surface| surface.name.ends_with("signal")).unwrap();

        assert_eq!(filled.background, dark().signal);
    }

    #[test]
    #[should_panic(expected = "malformed")]
    fn a_malformed_brand_colour_fails_loudly() {
        hex("#not-a-colour");
    }
}
