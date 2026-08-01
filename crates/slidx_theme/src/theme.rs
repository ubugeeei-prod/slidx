//! A theme: two palettes, a type scale, and the fonts to draw them with.

use serde::{Deserialize, Serialize};
use slidx_core::Easing;
use slidx_lint::{Surface, TextRole, TextSample};

use crate::palette::{Palette, Scheme};
use crate::scale::{TypeScale, REFERENCE_HEIGHT_PX};
use crate::typography::Typography;

/// Spacing tokens, in pixels at the reference canvas.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spacing {
    /// Slide padding. Doubles as the safe area the linter checks against,
    /// because content inside the padding is content a projector will not crop.
    pub padding_px: f64,
    /// Vertical rhythm between blocks.
    pub block_px: f64,
    /// Corner radius. Flat themes leave this at zero.
    pub radius_px: f64,
    /// Hairline width for rules and borders.
    pub hairline_px: f64,
}

impl Default for Spacing {
    fn default() -> Self {
        Self { padding_px: 96.0, block_px: 28.0, radius_px: 0.0, hairline_px: 1.0 }
    }
}

/// Longest a transition may run once the viewer has asked for less motion.
///
/// A ceiling rather than a duration: a theme that already moves faster keeps
/// its own timing. Expressing it this way removes the failure where a theme
/// sets a leisurely reduced-motion duration and the accessible path ends up
/// *slower* than the one it replaces.
pub const REDUCED_MOTION_CEILING_MS: u32 = 120;

/// Motion tokens.
///
/// Timing is a theme decision for the same reason type size is: a deck that
/// hard-codes 300ms cannot be made calmer or snappier without editing every
/// slide. Nothing downstream writes a duration of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Motion {
    /// How long a slide-to-slide transition runs, in milliseconds.
    ///
    /// Long enough to read as a change of place, short enough that a presenter
    /// clicking through ten slides does not wait for the tool. Past roughly
    /// 400ms an audience notices the transition instead of the slide.
    pub transition_ms: u32,
    /// Curve the transition runs on.
    ///
    /// Shared with the step vocabulary in [`slidx_core`] deliberately: a deck
    /// whose slide changes ease differently from its reveals reads as two
    /// tools stapled together.
    pub transition_easing: Easing,
}

impl Default for Motion {
    fn default() -> Self {
        Self { transition_ms: 240, transition_easing: Easing::EaseOut }
    }
}

impl Motion {
    /// Duration to use when the viewer prefers reduced motion.
    pub fn reduced_ms(self) -> u32 {
        self.transition_ms.min(REDUCED_MOTION_CEILING_MS)
    }
}

/// Everything that decides how a deck looks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    /// Stable identifier used by `theme:` in frontmatter.
    pub id: String,
    /// Human-readable name, shown in the editor's theme picker.
    pub name: String,
    /// One line on when to reach for this theme.
    pub description: String,
    pub light: Palette,
    pub dark: Palette,
    pub scale: TypeScale,
    pub spacing: Spacing,
    /// Timing for slide-to-slide transitions.
    ///
    /// Defaulted on read so a theme package published before transitions
    /// existed keeps loading. A third-party theme is a JSON file someone else
    /// owns; adding a required field to it would break decks that never asked
    /// for the feature.
    #[serde(default)]
    pub motion: Motion,
    /// Leading, tracking, and measure.
    ///
    /// Defaulted on read for the same reason [`Motion`] is: these were
    /// constants in the shell stylesheet before they were theme tokens, so
    /// every theme package published without them keeps rendering exactly as it
    /// did — the defaults are calibrated to reproduce the constants they
    /// replaced.
    #[serde(default)]
    pub typography: Typography,
    /// Font stack for prose. Must resolve locally: a theme that names a font
    /// only available from a CDN fails the offline check at build time.
    pub font_sans: String,
    pub font_mono: String,
}

impl Theme {
    pub fn palette(&self, scheme: Scheme) -> &Palette {
        match scheme {
            Scheme::Light => &self.light,
            Scheme::Dark => &self.dark,
        }
    }

    /// Describes this theme to the linter.
    ///
    /// Every colour role is paired with the background it is drawn on and the
    /// size it is drawn at, for both schemes. This is what makes a theme
    /// checkable by the same rules that check a deck — including third-party
    /// themes, which get audited without this crate knowing anything about
    /// them.
    pub fn surfaces(&self) -> Vec<Surface> {
        Scheme::ALL.iter().flat_map(|scheme| self.surfaces_for(*scheme)).collect()
    }

    fn surfaces_for(&self, scheme: Scheme) -> Vec<Surface> {
        let palette = self.palette(scheme);
        let mut slide =
            Surface::new(format!("{} / {}", self.id, scheme.as_token()), palette.surface);
        let mut code = Surface::new(
            format!("{} / {} / code", self.id, scheme.as_token()),
            palette.code_surface,
        );

        for (name, color, background) in palette.pairs() {
            let role = role_of(name);
            let sample =
                TextSample::new(role, color, self.scale.role_px(role), format!("theme.{name}"));

            // Routed by the background the role is drawn on rather than by
            // name: a syntax colour checked against the slide would pass a
            // comment that is invisible everywhere it is actually shown.
            if background == palette.code_surface {
                code = code.with_text(sample);
            } else {
                slide = slide.with_text(sample);
            }
        }

        vec![slide, code]
    }

    /// Canvas height the theme's sizes are quoted against.
    pub fn reference_height_px(&self) -> f64 {
        REFERENCE_HEIGHT_PX
    }
}

/// Maps a palette role name onto the legibility role it is judged as.
///
/// Every `code*` role is code: a comment is set at code size and read at code
/// density, and holding it to the body floor would let a theme ship a comment
/// colour that only works at heading size.
fn role_of(name: &str) -> TextRole {
    match name {
        "heading" => TextRole::Heading,
        "muted" => TextRole::Caption,
        name if name.starts_with("code") => TextRole::Code,
        _ => TextRole::Body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;

    #[test]
    fn a_theme_describes_both_schemes_to_the_linter() {
        let surfaces = builtin::minimal().surfaces();

        assert!(surfaces.iter().any(|surface| surface.name.contains("light")));
        assert!(surfaces.iter().any(|surface| surface.name.contains("dark")));
    }

    #[test]
    fn every_palette_role_is_audited() {
        // A colour added to Palette without being added to `pairs` would be a
        // role nobody checks, which is exactly the gap this guards.
        let theme = builtin::minimal();
        let audited: usize = theme.surfaces().iter().map(|surface| surface.text.len()).sum();

        assert_eq!(audited, theme.light.pairs().len() * Scheme::ALL.len());
    }

    #[test]
    fn code_text_is_audited_against_the_code_surface() {
        let theme = builtin::minimal();
        let code = theme
            .surfaces()
            .into_iter()
            .find(|surface| surface.name.contains("light / code"))
            .unwrap();

        assert_eq!(code.background, theme.light.code_surface);
        assert!(code.text.iter().all(|sample| sample.role == TextRole::Code));
        assert!(code.text.iter().any(|sample| sample.origin == "theme.codeText"));
    }

    #[test]
    fn every_syntax_colour_is_audited_as_code_on_the_code_surface() {
        // The point of putting these colours in the theme at all: a comment
        // colour that is illegible on a projector is the failure this project
        // exists to catch, and it can only be caught here.
        let theme = builtin::minimal();
        let code = theme
            .surfaces()
            .into_iter()
            .find(|surface| surface.name.contains("dark / code"))
            .unwrap();

        for role in ["codeComment", "codeString", "codeKeyword", "codeType", "codePunctuation"] {
            let sample = code
                .text
                .iter()
                .find(|sample| sample.origin == format!("theme.{role}"))
                .unwrap_or_else(|| panic!("{role} is not audited"));

            assert_eq!(sample.role, TextRole::Code);
            assert_eq!(sample.font_px, theme.scale.code_px());
        }
    }

    #[test]
    fn headings_are_audited_at_heading_size() {
        let theme = builtin::minimal();
        let slide = theme.surfaces().into_iter().next().unwrap();
        let heading = slide.text.iter().find(|sample| sample.role == TextRole::Heading).unwrap();

        assert_eq!(heading.font_px, theme.scale.heading_px(1));
    }

    #[test]
    fn origins_name_something_the_author_can_change() {
        let slide = builtin::minimal().surfaces().into_iter().next().unwrap();

        for sample in &slide.text {
            assert!(sample.origin.starts_with("theme."), "unhelpful origin: {}", sample.origin);
        }
    }

    #[test]
    fn palettes_are_selectable_by_scheme() {
        let theme = builtin::minimal();
        assert_eq!(theme.palette(Scheme::Light), &theme.light);
        assert_eq!(theme.palette(Scheme::Dark), &theme.dark);
    }

    #[test]
    fn the_reduced_duration_only_ever_shortens() {
        // The accessible path must never be the slower one.
        let brisk = Motion { transition_ms: 80, transition_easing: Easing::EaseOut };
        assert_eq!(brisk.reduced_ms(), 80, "a theme already under the ceiling keeps its timing");

        let leisurely = Motion { transition_ms: 600, transition_easing: Easing::EaseOut };
        assert_eq!(leisurely.reduced_ms(), REDUCED_MOTION_CEILING_MS);
    }

    #[test]
    fn a_theme_package_written_before_transitions_still_loads() {
        // Third-party themes are JSON files someone else owns and does not
        // republish. A required field here would break decks that never asked
        // for motion.
        let json = serde_json::to_value(builtin::minimal()).unwrap();
        let mut without_motion = json.as_object().unwrap().clone();
        without_motion.remove("motion");

        let loaded: Theme = serde_json::from_value(without_motion.into()).unwrap();
        assert_eq!(loaded.motion, Motion::default());
    }
}
