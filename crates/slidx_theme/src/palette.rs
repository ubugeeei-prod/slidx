//! Colour roles.
//!
//! A palette names *roles*, not colours: `text`, `muted`, `accent`. That is
//! what lets the linter check a theme it has never seen, and what lets a deck
//! swap themes without rewriting a slide.
//!
//! Every palette carries both a light and a dark variant, because the room's
//! lighting is usually unknown until the day and switching at the venue must
//! not mean re-authoring anything.

use serde::{Deserialize, Serialize};
use slidx_lint::Rgba;

/// The colours one slide is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    /// Behind the slide, visible as letterboxing when the aspect does not fit.
    pub canvas: Rgba,
    /// The slide itself.
    pub surface: Rgba,
    pub text: Rgba,
    /// Secondary text: captions, footers, attributions.
    pub muted: Rgba,
    pub heading: Rgba,
    /// Links, strong text, and the accent line.
    pub accent: Rgba,
    pub border: Rgba,
    pub code_surface: Rgba,
    pub code_text: Rgba,
}

/// Which variant of a theme is in use.
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

/// Parses a hex colour at compile-time-adjacent call sites.
///
/// Panics on a malformed literal, which is correct here: these are constants
/// in this crate's own source, so a bad one is a bug rather than user input.
pub(crate) fn hex(text: &str) -> Rgba {
    slidx_lint::color::parse(text)
        .unwrap_or_else(|| panic!("built-in theme colour `{text}` is malformed"))
}

impl Palette {
    /// Every text role paired with the background it is drawn on.
    ///
    /// This is the list the linter walks, so a role missing from it is a role
    /// nobody checks. Adding a colour to [`Palette`] without adding it here is
    /// caught by `every_palette_role_is_audited`.
    pub fn pairs(&self) -> Vec<(&'static str, Rgba, Rgba)> {
        vec![
            ("text", self.text, self.surface),
            ("muted", self.muted, self.surface),
            ("heading", self.heading, self.surface),
            ("accent", self.accent, self.surface),
            ("codeText", self.code_text, self.code_surface),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Palette {
        Palette {
            canvas: hex("#f4f4f5"),
            surface: hex("#ffffff"),
            text: hex("#18181b"),
            muted: hex("#52525b"),
            heading: hex("#09090b"),
            accent: hex("#1d4ed8"),
            border: hex("#e4e4e7"),
            code_surface: hex("#f4f4f5"),
            code_text: hex("#18181b"),
        }
    }

    #[test]
    fn every_text_role_is_paired_with_the_background_it_sits_on() {
        let pairs = sample().pairs();
        let names: Vec<&str> = pairs.iter().map(|(name, _, _)| *name).collect();

        assert_eq!(names, vec!["text", "muted", "heading", "accent", "codeText"]);
    }

    #[test]
    fn code_text_is_paired_with_the_code_surface_not_the_slide() {
        let palette = sample();
        let (_, _, background) =
            palette.pairs().into_iter().find(|(name, _, _)| *name == "codeText").unwrap();

        assert_eq!(background, palette.code_surface);
    }

    #[test]
    fn schemes_round_trip_through_their_tokens() {
        assert_eq!(Scheme::Light.as_token(), "light");
        assert_eq!(Scheme::Dark.as_token(), "dark");
        assert_eq!(Scheme::ALL.len(), 2);
    }

    #[test]
    #[should_panic(expected = "malformed")]
    fn a_malformed_built_in_colour_fails_loudly() {
        hex("#not-a-colour");
    }
}
