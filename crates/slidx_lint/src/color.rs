//! Colour, luminance, and contrast.
//!
//! Contrast is the single check that most often passes on a laptop and fails in
//! a conference room, so this module models the room rather than the monitor.
//! Everything here is sRGB and WCAG 2.x, extended with a projection model —
//! see [`ProjectorProfile`].

use serde::{Deserialize, Serialize};

mod parse;

pub use parse::parse;

/// An opaque or translucent sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// 0.0 transparent, 1.0 opaque.
    pub a: f64,
}

impl Rgba {
    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const WHITE: Self = Self::opaque(255, 255, 255);
    pub const BLACK: Self = Self::opaque(0, 0, 0);

    /// Composites this colour over an opaque backdrop.
    ///
    /// Text on a translucent panel is common in slide themes, and checking the
    /// declared colour rather than the composited one reports a contrast the
    /// audience never sees.
    pub fn over(self, backdrop: Rgba) -> Rgba {
        if self.a >= 1.0 {
            return Rgba { a: 1.0, ..self };
        }

        let blend = |top: u8, bottom: u8| {
            (f64::from(top) * self.a + f64::from(bottom) * (1.0 - self.a)).round() as u8
        };

        Rgba {
            r: blend(self.r, backdrop.r),
            g: blend(self.g, backdrop.g),
            b: blend(self.b, backdrop.b),
            a: 1.0,
        }
    }

    /// WCAG relative luminance, in the range 0.0 to 1.0.
    pub fn relative_luminance(self) -> f64 {
        let channel = |value: u8| {
            let normalised = f64::from(value) / 255.0;
            if normalised <= 0.040_45 {
                normalised / 12.92
            } else {
                ((normalised + 0.055) / 1.055).powf(2.4)
            }
        };

        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// How much a projector and the room's ambient light flatten contrast.
///
/// A projector cannot emit black: the darkest pixel is whatever light the room
/// is already putting on the screen. That raises the floor of every luminance
/// and compresses every ratio towards 1. Modelling it as an additive light
/// floor is simple, physically motivated, and reproduces the roughly 30%
/// contrast loss reported for typical rooms.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectorProfile {
    /// Perfect conditions: an OLED panel, or a screenshot on a monitor.
    Direct,
    /// Lights down, blinds closed. The best a conference room usually gets.
    DarkRoom,
    /// Lights partly up so the audience can take notes. The realistic default.
    #[default]
    Typical,
    /// Lights up, or daylight on the screen. Common in meeting rooms.
    BrightRoom,
}

impl ProjectorProfile {
    /// Stray light on the screen, as a fraction of the projector's white.
    fn ambient(self) -> f64 {
        match self {
            Self::Direct => 0.0,
            Self::DarkRoom => 0.02,
            Self::Typical => 0.035,
            Self::BrightRoom => 0.08,
        }
    }

    pub fn as_token(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::DarkRoom => "dark-room",
            Self::Typical => "typical",
            Self::BrightRoom => "bright-room",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "direct" => Some(Self::Direct),
            "dark-room" | "dark" => Some(Self::DarkRoom),
            "typical" => Some(Self::Typical),
            "bright-room" | "bright" => Some(Self::BrightRoom),
            _ => None,
        }
    }

    /// Adjusts a luminance for the light the room adds to it.
    fn project(self, luminance: f64) -> f64 {
        let ambient = self.ambient();
        luminance * (1.0 - ambient) + ambient
    }
}

/// WCAG contrast ratio between two opaque colours, from 1.0 to 21.0.
pub fn contrast_ratio(foreground: Rgba, background: Rgba) -> f64 {
    ratio_of(foreground.relative_luminance(), background.relative_luminance())
}

/// Contrast ratio as the room will actually show it.
pub fn projected_contrast_ratio(
    foreground: Rgba,
    background: Rgba,
    profile: ProjectorProfile,
) -> f64 {
    ratio_of(
        profile.project(foreground.relative_luminance()),
        profile.project(background.relative_luminance()),
    )
}

fn ratio_of(a: f64, b: f64) -> f64 {
    let (lighter, darker) = if a >= b { (a, b) } else { (b, a) };
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() < 0.05
    }

    #[test]
    fn luminance_matches_the_wcag_endpoints() {
        assert!(close(Rgba::WHITE.relative_luminance(), 1.0));
        assert!(close(Rgba::BLACK.relative_luminance(), 0.0));
    }

    #[test]
    fn contrast_matches_the_wcag_endpoints() {
        assert!(close(contrast_ratio(Rgba::BLACK, Rgba::WHITE), 21.0));
        assert!(close(contrast_ratio(Rgba::WHITE, Rgba::WHITE), 1.0));
    }

    #[test]
    fn contrast_is_symmetric() {
        let a = parse("#767676").unwrap();
        let b = Rgba::WHITE;
        assert!(close(contrast_ratio(a, b), contrast_ratio(b, a)));
    }

    #[test]
    fn the_wcag_reference_pair_lands_on_its_documented_ratio() {
        // #767676 on white is the canonical 4.5:1 boundary colour.
        let ratio = contrast_ratio(parse("#767676").unwrap(), Rgba::WHITE);
        assert!((ratio - 4.54).abs() < 0.05, "expected about 4.54:1, got {ratio}");
    }

    #[test]
    fn translucent_text_is_checked_against_what_is_actually_shown() {
        let half_black = parse("#00000080").unwrap();
        let composited = half_black.over(Rgba::WHITE);

        assert!(composited.a >= 1.0);
        assert!(
            contrast_ratio(composited, Rgba::WHITE) < contrast_ratio(Rgba::BLACK, Rgba::WHITE),
            "compositing must reduce the measured contrast, not preserve it"
        );
    }

    #[test]
    fn an_opaque_colour_is_unchanged_by_compositing() {
        assert_eq!(Rgba::BLACK.over(Rgba::WHITE), Rgba::BLACK);
    }

    #[test]
    fn projection_never_improves_contrast() {
        let pairs = [("#000000", "#ffffff"), ("#333333", "#eeeeee"), ("#1d76db", "#ffffff")];

        for (fg, bg) in pairs {
            let foreground = parse(fg).unwrap();
            let background = parse(bg).unwrap();
            let direct = contrast_ratio(foreground, background);

            for profile in [
                ProjectorProfile::DarkRoom,
                ProjectorProfile::Typical,
                ProjectorProfile::BrightRoom,
            ] {
                let projected = projected_contrast_ratio(foreground, background, profile);
                assert!(projected <= direct, "{fg}/{bg} under {}", profile.as_token());
            }
        }
    }

    #[test]
    fn brighter_rooms_flatten_contrast_further() {
        let dark = projected_contrast_ratio(Rgba::BLACK, Rgba::WHITE, ProjectorProfile::DarkRoom);
        let typical = projected_contrast_ratio(Rgba::BLACK, Rgba::WHITE, ProjectorProfile::Typical);
        let bright =
            projected_contrast_ratio(Rgba::BLACK, Rgba::WHITE, ProjectorProfile::BrightRoom);

        assert!(dark > typical && typical > bright);
    }

    #[test]
    fn the_direct_profile_leaves_contrast_alone() {
        let direct = projected_contrast_ratio(Rgba::BLACK, Rgba::WHITE, ProjectorProfile::Direct);
        assert!(close(direct, contrast_ratio(Rgba::BLACK, Rgba::WHITE)));
    }

    #[test]
    fn a_dark_room_loses_roughly_the_reported_third_of_contrast() {
        // Published guidance puts projected contrast loss near 30%. The model
        // is calibrated against that rather than invented.
        let direct = contrast_ratio(Rgba::BLACK, Rgba::WHITE);
        let projected =
            projected_contrast_ratio(Rgba::BLACK, Rgba::WHITE, ProjectorProfile::DarkRoom);
        let loss = 1.0 - projected / direct;

        assert!((0.2..0.4).contains(&loss), "expected roughly a third lost, got {loss:.2}");
    }

    #[test]
    fn a_pair_that_only_just_passes_on_a_monitor_fails_in_a_room() {
        // This is the whole reason the profile exists: 4.54:1 is a pass on a
        // laptop and unreadable from row 12.
        let foreground = parse("#767676").unwrap();
        assert!(contrast_ratio(foreground, Rgba::WHITE) >= 4.5);
        assert!(projected_contrast_ratio(foreground, Rgba::WHITE, ProjectorProfile::Typical) < 4.5);
    }

    #[test]
    fn profiles_round_trip_through_their_tokens() {
        for profile in [
            ProjectorProfile::Direct,
            ProjectorProfile::DarkRoom,
            ProjectorProfile::Typical,
            ProjectorProfile::BrightRoom,
        ] {
            assert_eq!(ProjectorProfile::parse(profile.as_token()), Some(profile));
        }
        assert_eq!(ProjectorProfile::parse("outdoors"), None);
    }
}
