//! A theme: two palettes, a type scale, and the fonts to draw them with.

use serde::{Deserialize, Serialize};
use slidx_lint::{Surface, TextRole, TextSample};

use crate::palette::{Palette, Scheme};
use crate::scale::{TypeScale, REFERENCE_HEIGHT_PX};

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

            if background == palette.code_surface && name == "codeText" {
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
fn role_of(name: &str) -> TextRole {
    match name {
        "heading" => TextRole::Heading,
        "muted" => TextRole::Caption,
        "codeText" => TextRole::Code,
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
        assert_eq!(code.text.len(), 1);
        assert_eq!(code.text[0].role, TextRole::Code);
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
}
