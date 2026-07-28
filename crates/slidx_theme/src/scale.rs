//! Type scale.
//!
//! Sizes come from a modular scale rather than a list of magic numbers, for
//! one reason that matters on stage: it makes "shrink the text until it fits"
//! impossible to do by accident. There is no arbitrary size to reach for — a
//! theme picks a base and a ratio, every role derives from those, and a slide
//! that overflows has to be scaled or split instead.
//!
//! Sizes are expressed against a reference canvas height, so a theme is
//! resolution-independent and the linter's angular-size model applies directly.

use serde::{Deserialize, Serialize};
use slidx_lint::TextRole;

/// The canvas the theme's numbers are quoted against.
///
/// 1080 is the resolution published decks are graded at, so a theme's body
/// size reads as the number an author expects.
pub const REFERENCE_HEIGHT_PX: f64 = 1080.0;

/// A modular type scale.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeScale {
    /// Body size, in pixels at [`REFERENCE_HEIGHT_PX`].
    pub base_px: f64,
    /// Step between adjacent sizes. 1.25 (major third) is calm; 1.414
    /// (augmented fourth) is dramatic.
    pub ratio: f64,
    /// Code size relative to body.
    ///
    /// Code is denser than prose and carries no redundancy from context, so
    /// the linter holds it to a stricter floor. A theme with a small base has
    /// no room to set code below body size and still pass the audit; one with
    /// a generous base does. The audit decides, not a fixed limit here.
    pub code_factor: f64,
}

impl Default for TypeScale {
    fn default() -> Self {
        Self { base_px: 32.0, ratio: 1.25, code_factor: 1.0 }
    }
}

impl TypeScale {
    /// Size for a heading level, where 1 is the largest.
    ///
    /// Levels below 1 clamp rather than growing without bound, so a stray `#`
    /// cannot produce a glyph taller than the slide.
    pub fn heading_px(&self, level: u8) -> f64 {
        let steps = match level {
            0 | 1 => 3,
            2 => 2,
            3 => 1,
            _ => 0,
        };
        self.base_px * self.ratio.powi(steps)
    }

    pub fn body_px(&self) -> f64 {
        self.base_px
    }

    pub fn code_px(&self) -> f64 {
        self.base_px * self.code_factor
    }

    pub fn caption_px(&self) -> f64 {
        self.base_px / self.ratio
    }

    /// Size for a role, using the largest heading.
    pub fn role_px(&self, role: TextRole) -> f64 {
        match role {
            TextRole::Heading => self.heading_px(1),
            TextRole::Body => self.body_px(),
            TextRole::Code => self.code_px(),
            TextRole::Caption => self.caption_px(),
        }
    }

    /// Scales every size for a canvas of a different height.
    ///
    /// A deck authored at 4:3 has a 1080 canvas too, so this is mostly for
    /// themes quoting their numbers against something else.
    pub fn at_canvas(&self, height_px: f64) -> Self {
        let factor = height_px / REFERENCE_HEIGHT_PX;
        Self { base_px: self.base_px * factor, ..*self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_descend_in_size() {
        let scale = TypeScale::default();
        let sizes: Vec<f64> = (1..=4).map(|level| scale.heading_px(level)).collect();

        for pair in sizes.windows(2) {
            assert!(pair[0] > pair[1], "heading sizes must descend: {sizes:?}");
        }
    }

    #[test]
    fn the_smallest_heading_is_still_at_least_body_size() {
        let scale = TypeScale::default();
        assert!(scale.heading_px(6) >= scale.body_px());
    }

    #[test]
    fn a_stray_level_zero_does_not_grow_without_bound() {
        let scale = TypeScale::default();
        assert_eq!(scale.heading_px(0), scale.heading_px(1));
    }

    #[test]
    fn captions_are_smaller_than_body_text() {
        let scale = TypeScale::default();
        assert!(scale.caption_px() < scale.body_px());
    }

    #[test]
    fn a_wider_ratio_spreads_the_scale_further() {
        let calm = TypeScale { ratio: 1.2, ..TypeScale::default() };
        let dramatic = TypeScale { ratio: 1.5, ..TypeScale::default() };

        assert!(dramatic.heading_px(1) > calm.heading_px(1));
        assert_eq!(dramatic.body_px(), calm.body_px(), "the base is unchanged");
    }

    #[test]
    fn roles_map_onto_the_scale() {
        let scale = TypeScale::default();

        assert_eq!(scale.role_px(TextRole::Heading), scale.heading_px(1));
        assert_eq!(scale.role_px(TextRole::Body), scale.base_px);
        assert_eq!(scale.role_px(TextRole::Code), scale.code_px());
        assert_eq!(scale.role_px(TextRole::Caption), scale.caption_px());
    }

    #[test]
    fn scaling_to_a_canvas_preserves_every_ratio() {
        let scale = TypeScale::default();
        let half = scale.at_canvas(REFERENCE_HEIGHT_PX / 2.0);

        assert!((half.base_px - scale.base_px / 2.0).abs() < 0.001);
        assert!((half.heading_px(1) - scale.heading_px(1) / 2.0).abs() < 0.001);
    }

    #[test]
    fn the_default_base_clears_the_legibility_floor_with_room_to_spare() {
        // A theme whose default body size only just passes would leave authors
        // no headroom to set anything smaller.
        let floor =
            slidx_lint::min_font_px(TextRole::Body, REFERENCE_HEIGHT_PX, Default::default());

        assert!(
            TypeScale::default().base_px > floor * 1.1,
            "default base {} is too close to the {floor:.0}px floor",
            TypeScale::default().base_px
        );
    }
}
