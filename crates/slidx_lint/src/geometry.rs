//! The slide's box, and what the room takes off it.
//!
//! Two things shrink the rectangle an audience actually sees, and neither is
//! visible on the machine the deck was written on.
//!
//! **The renderer's padding.** slidx enforces the theme's padding in the shell
//! rather than leaving it to each layout, so that padding *is* the safe area a
//! deck is guaranteed. It is stated by whatever rendered the slide and never
//! assumed here — a linter that invented a padding would report bleed on a
//! theme that has none.
//!
//! **The room.** A projector crops the edges it cannot square up, and a venue
//! that burns subtitles takes a band across the bottom for the whole talk.
//! Neither is something slidx draws, so neither can be measured from the deck;
//! both are declared by the author, who is the only one who was told.
//!
//! Everything here is arithmetic over declared numbers. Nothing is estimated,
//! which is what lets the diagnostics carry an exact pixel count.

pub mod declare;

use serde::{Deserialize, Serialize};

use crate::surface::RenderTarget;

/// A bleed smaller than this is a rounding artefact of the declaration rather
/// than a band anyone in the room can see.
const SUB_PIXEL: f64 = 0.5;

/// Share of the content box a band must take before it hides content.
///
/// A twentieth of the content box is about one line of body text at any type
/// scale that clears the legibility floor — 28px at a line height of 1.5 on a
/// 1080 canvas with the default padding is 4.7% of it. Below that the band eats
/// the frame's own breathing room: the footer's leading, and the gap a centred
/// body leaves under itself. Above it, a line an audience was meant to read is
/// inside the band.
pub(crate) const CUTTING_SHARE: f64 = 0.05;

/// One edge of the slide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    pub const ALL: [Self; 4] = [Self::Top, Self::Right, Self::Bottom, Self::Left];

    pub fn as_token(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
        }
    }

    /// The canvas dimension an inset on this side is measured against.
    pub fn extent_px(self, target: RenderTarget) -> f64 {
        match self {
            Self::Top | Self::Bottom => target.height_px,
            Self::Right | Self::Left => target.width_px,
        }
    }
}

/// How far in from each edge something reaches, as a share of that edge's axis.
///
/// Shares rather than pixels because both contributors are proportional: a
/// theme's padding scales with the slide, and a venue quotes its caption strip
/// as a fraction of the picture. Neither has a fixed pixel value until a canvas
/// is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Insets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Insets {
    pub const NONE: Self = Self { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };

    /// The same share of its own axis on every side.
    ///
    /// What a venue means by "the projector loses 5%": five percent of the
    /// width off each side, five percent of the height off the top and bottom.
    pub fn uniform(share: f64) -> Self {
        Self { top: share, right: share, bottom: share, left: share }
    }

    /// The same *physical* inset on every side, stated as a share of height.
    ///
    /// A renderer that scales the slide as one piece expresses its padding in
    /// units of the slide's height — the shell resolves `--slidx-space-padding`
    /// in `cqh` — so the left and right shares fall out of the aspect ratio
    /// rather than being stated separately.
    pub fn from_padding(share_of_height: f64, target: RenderTarget) -> Self {
        let horizontal = if target.width_px > 0.0 {
            share_of_height * target.height_px / target.width_px
        } else {
            0.0
        };

        Self { top: share_of_height, right: horizontal, bottom: share_of_height, left: horizontal }
    }

    pub fn share(self, side: Side) -> f64 {
        match side {
            Side::Top => self.top,
            Side::Right => self.right,
            Side::Bottom => self.bottom,
            Side::Left => self.left,
        }
    }

    pub fn with_side(mut self, side: Side, share: f64) -> Self {
        match side {
            Side::Top => self.top = share,
            Side::Right => self.right = share,
            Side::Bottom => self.bottom = share,
            Side::Left => self.left = share,
        }
        self
    }

    pub fn is_none(self) -> bool {
        Side::ALL.into_iter().all(|side| self.share(side) <= 0.0)
    }

    /// Extent of the box left inside these insets, along one side's axis.
    pub fn content_px(self, side: Side, target: RenderTarget) -> f64 {
        let (near, far) = match side {
            Side::Top | Side::Bottom => (self.top, self.bottom),
            Side::Right | Side::Left => (self.right, self.left),
        };

        (side.extent_px(target) * (1.0 - near - far)).max(0.0)
    }
}

/// One side where the room reaches inside the safe area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bleed {
    pub side: Side,
    /// How far past the safe area the room reaches, in design-space pixels.
    pub past_px: f64,
    /// That distance as a share of the content box on the same axis.
    pub share_of_content: f64,
}

impl Bleed {
    /// True when the band takes enough of the content box to hide a line.
    ///
    /// The distinction the whole rule turns on: a band that grazes the frame
    /// and a band that swallows a third of it cannot carry the same severity,
    /// because a rule that treats them alike is a rule an author switches off.
    pub fn is_cutting(self) -> bool {
        self.share_of_content >= CUTTING_SHARE
    }
}

/// Every side where `room` reaches past the safe area `padding` guarantees.
///
/// Sides where the room stays inside the padding produce nothing: the theme
/// already keeps content out of that band, which is the case worth staying
/// quiet about.
pub fn bleed(padding: Insets, room: Insets, target: RenderTarget) -> Vec<Bleed> {
    Side::ALL
        .into_iter()
        .filter_map(|side| {
            let past_px = (room.share(side) - padding.share(side)) * side.extent_px(target);
            if past_px < SUB_PIXEL {
                return None;
            }

            let content = padding.content_px(side, target);
            let share_of_content = if content > 0.0 { past_px / content } else { 1.0 };

            Some(Bleed { side, past_px, share_of_content })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default 16:9 canvas with the built-in themes' 96px padding.
    const TARGET: RenderTarget = RenderTarget { width_px: 1920.0, height_px: 1080.0 };
    const PADDING_SHARE: f64 = 96.0 / 1080.0;

    fn padding() -> Insets {
        Insets::from_padding(PADDING_SHARE, TARGET)
    }

    #[test]
    fn a_uniform_padding_is_the_same_number_of_pixels_on_every_side() {
        // The shell resolves padding in units of the slide's height, so the
        // left and right *shares* differ while the physical inset does not.
        let padding = padding();

        assert!((padding.top * TARGET.height_px - 96.0).abs() < 0.001);
        assert!((padding.left * TARGET.width_px - 96.0).abs() < 0.001);
        assert!(padding.left < padding.top, "a wide canvas needs a smaller horizontal share");
    }

    #[test]
    fn a_room_that_stays_inside_the_padding_takes_nothing() {
        // The case worth being quiet about: the theme already keeps content
        // out of the band the venue eats.
        let room = Insets::NONE.with_side(Side::Bottom, 0.05);
        assert!(bleed(padding(), room, TARGET).is_empty());
    }

    #[test]
    fn a_caption_strip_past_the_padding_is_reported_in_pixels() {
        // A venue that burns subtitles across the bottom 15% of a 1080 canvas
        // takes 162px; the theme's padding covers 96 of them.
        let room = Insets::NONE.with_side(Side::Bottom, 0.15);
        let found = bleed(padding(), room, TARGET);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].side, Side::Bottom);
        assert!((found[0].past_px - 66.0).abs() < 0.001, "got {}", found[0].past_px);
    }

    #[test]
    fn a_band_that_takes_a_line_of_the_content_box_is_cutting() {
        let room = Insets::NONE.with_side(Side::Bottom, 0.15);
        assert!(bleed(padding(), room, TARGET)[0].is_cutting());
    }

    #[test]
    fn a_band_that_only_grazes_the_frame_is_not_cutting() {
        // 10% of 1080 is 108px against 96px of padding: twelve pixels, which
        // is inside the footer's own leading.
        let room = Insets::NONE.with_side(Side::Bottom, 0.10);
        let found = bleed(padding(), room, TARGET);

        assert_eq!(found.len(), 1);
        assert!(!found[0].is_cutting(), "share was {}", found[0].share_of_content);
    }

    #[test]
    fn the_cutting_threshold_is_about_one_line_at_the_legibility_floor() {
        // The calibration the constant rests on: a band under one line of the
        // smallest legible body text must not be an error.
        let content_height = padding().content_px(Side::Bottom, TARGET);
        let line_px = crate::typography::min_font_px(
            crate::typography::TextRole::Body,
            TARGET.height_px,
            crate::typography::ViewingProfile::default(),
        ) * 1.5;

        assert!(line_px / content_height <= CUTTING_SHARE, "{line_px}px is not under the floor");
    }

    #[test]
    fn a_sub_pixel_bleed_is_not_a_band_anyone_sees() {
        let room = Insets::NONE.with_side(Side::Bottom, PADDING_SHARE + 0.0002);
        assert!(bleed(padding(), room, TARGET).is_empty());
    }

    #[test]
    fn every_side_is_checked_independently() {
        let room = Insets::uniform(0.2);
        let found = bleed(padding(), room, TARGET);

        assert_eq!(found.len(), 4);
        for side in Side::ALL {
            assert!(found.iter().any(|bleed| bleed.side == side), "{} missing", side.as_token());
        }
    }

    #[test]
    fn a_crop_is_measured_against_the_axis_it_is_on() {
        // 10% off the left of a 1920 canvas is 192px; 10% off the top of a
        // 1080 one is 108. A model that used one extent for both would report
        // the wrong number on three quarters of the frame.
        let found = bleed(Insets::NONE, Insets::uniform(0.1), TARGET);

        let left = found.iter().find(|bleed| bleed.side == Side::Left).unwrap();
        let top = found.iter().find(|bleed| bleed.side == Side::Top).unwrap();

        assert!((left.past_px - 192.0).abs() < 0.001);
        assert!((top.past_px - 108.0).abs() < 0.001);
    }

    #[test]
    fn the_share_is_of_what_is_left_after_the_padding_not_of_the_canvas() {
        // An author acts on how much of their *content* is gone, and the
        // content box is already smaller than the slide.
        let room = Insets::NONE.with_side(Side::Bottom, 0.15);
        let found = bleed(padding(), room, TARGET)[0];

        assert!((found.share_of_content - 66.0 / 888.0).abs() < 0.001);
    }

    #[test]
    fn a_degenerate_padding_does_not_divide_by_zero() {
        let swallowed = Insets::uniform(0.5);
        let found = bleed(swallowed, Insets::uniform(0.6), TARGET);

        assert_eq!(found.len(), 4);
        assert!(found.iter().all(|bleed| bleed.share_of_content == 1.0));
    }

    #[test]
    fn a_zero_width_target_does_not_produce_a_nonsense_padding() {
        let target = RenderTarget { width_px: 0.0, height_px: 1080.0 };
        let padding = Insets::from_padding(0.1, target);

        assert_eq!(padding.left, 0.0);
        assert_eq!(padding.top, 0.1);
    }

    #[test]
    fn insets_with_nothing_in_them_report_as_empty() {
        assert!(Insets::NONE.is_none());
        assert!(!Insets::uniform(0.01).is_none());
    }
}
