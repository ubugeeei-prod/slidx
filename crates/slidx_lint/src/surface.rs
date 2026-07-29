//! The contract between whatever renders a slide and the linter.
//!
//! The linter must not know about themes, CSS, or a specific renderer — it
//! checks *resolved* text on *resolved* backgrounds. A surface is that
//! resolution: one background, and every piece of text drawn on it.
//!
//! Keeping this narrow is what lets the same rules run over a built-in theme,
//! a third-party theme package, a React island, and a browser measurement pass
//! without any of them being special-cased.

use serde::{Deserialize, Serialize};

use crate::color::Rgba;
use crate::typography::TextRole;

/// Where a slide is being drawn, in design-space pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTarget {
    pub width_px: f64,
    pub height_px: f64,
}

impl Default for RenderTarget {
    fn default() -> Self {
        Self { width_px: 1920.0, height_px: 1080.0 }
    }
}

impl RenderTarget {
    pub fn from_dimensions((width, height): (u32, u32)) -> Self {
        Self { width_px: f64::from(width), height_px: f64::from(height) }
    }
}

/// What a browser found when it laid one stop out for real.
///
/// The other half of this contract describes what a renderer *intends*. This
/// one describes what actually happened, and it exists because one question in
/// the rule set cannot be answered any other way: whether a slide's content
/// fits its box depends on line breaking, and line breaking depends on font
/// metrics no build-time model has.
///
/// Shares of the box rather than pixel counts, because the measuring browser
/// laid the page out at whatever size it chose. A ratio survives that; a pixel
/// figure would be true only at the width it was taken at.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    pub slide_index: u32,
    /// Which stop of the slide was measured, zero-based. A slide with no steps
    /// has one, and a slide that only overflows on its last reveal is the whole
    /// reason this is per stop rather than per slide.
    pub stop: u32,
    /// How far the content exceeded the box downwards, as a share of the box.
    /// Zero when it fitted.
    pub over_height: f64,
    /// The same across.
    pub over_width: f64,
}

impl Measurement {
    pub fn new(slide_index: u32, stop: u32) -> Self {
        Self { slide_index, stop, over_height: 0.0, over_width: 0.0 }
    }

    pub fn over(mut self, height: f64, width: f64) -> Self {
        self.over_height = height;
        self.over_width = width;
        self
    }
}

/// One piece of text, already resolved to a colour and a size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSample {
    pub role: TextRole,
    pub color: Rgba,
    /// Size in the deck's design space, before the slide is scaled to fit.
    pub font_px: f64,
    /// Human-readable source, such as `theme.colorTextMuted` or
    /// `slide 4 accent`. Shown verbatim in diagnostics, so it should name
    /// something the author can go and change.
    pub origin: String,
}

impl TextSample {
    pub fn new(role: TextRole, color: Rgba, font_px: f64, origin: impl Into<String>) -> Self {
        Self { role, color, font_px, origin: origin.into() }
    }
}

/// A background and everything drawn on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Surface {
    /// Human-readable name, such as `editorial / dark / code block`.
    pub name: String,
    pub background: Rgba,
    pub text: Vec<TextSample>,
    /// Set when the surface comes from one slide rather than from the theme.
    pub slide_index: Option<u32>,
    /// One-based source line, when the surface has one.
    pub line: u32,
}

impl Surface {
    pub fn new(name: impl Into<String>, background: Rgba) -> Self {
        Self { name: name.into(), background, text: Vec::new(), slide_index: None, line: 0 }
    }

    pub fn with_text(mut self, sample: TextSample) -> Self {
        self.text.push(sample);
        self
    }

    pub fn on_slide(mut self, index: u32) -> Self {
        self.slide_index = Some(index);
        self
    }

    pub fn at_line(mut self, line: u32) -> Self {
        self.line = line;
        self
    }

    /// Composites a text colour against this surface's background.
    ///
    /// Themes routinely set muted text with an alpha rather than a separate
    /// colour, and checking the declared value would report a contrast the
    /// audience never sees.
    pub fn composited(&self, sample: &TextSample) -> Rgba {
        sample.color.over(self.background)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color;

    fn sample() -> TextSample {
        TextSample::new(TextRole::Body, Rgba::BLACK, 28.0, "theme.colorText")
    }

    #[test]
    fn a_surface_collects_its_text() {
        let surface = Surface::new("editorial / light", Rgba::WHITE)
            .with_text(sample())
            .on_slide(3)
            .at_line(12);

        assert_eq!(surface.text.len(), 1);
        assert_eq!(surface.slide_index, Some(3));
        assert_eq!(surface.line, 12);
    }

    #[test]
    fn compositing_resolves_translucent_text_against_the_background() {
        let surface = Surface::new("panel", Rgba::WHITE);
        let muted = TextSample::new(
            TextRole::Body,
            color::parse("#00000099").unwrap(),
            28.0,
            "theme.colorTextMuted",
        );

        let resolved = surface.composited(&muted);
        assert_eq!(resolved.a, 1.0);
        assert!(resolved.r > 0, "compositing over white lightens the text");
    }

    #[test]
    fn opaque_text_is_unchanged_by_compositing() {
        let surface = Surface::new("panel", Rgba::WHITE);
        assert_eq!(surface.composited(&sample()), Rgba::BLACK);
    }

    #[test]
    fn the_default_render_target_is_publication_resolution() {
        let target = RenderTarget::default();
        assert_eq!((target.width_px, target.height_px), (1920.0, 1080.0));
    }

    #[test]
    fn a_render_target_can_be_built_from_an_aspect_ratio() {
        let target = RenderTarget::from_dimensions(slidx_core::AspectRatio::Classic.dimensions());
        assert_eq!((target.width_px, target.height_px), (1440.0, 1080.0));
    }
}
