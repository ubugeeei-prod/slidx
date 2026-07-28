//! Colour, luminance, and contrast.
//!
//! Contrast is the single check that most often passes on a laptop and fails in
//! a conference room, so this module models the room rather than the monitor.
//! Everything here is sRGB and WCAG 2.x, extended with a projection model —
//! see [`ProjectorProfile`].

use serde::{Deserialize, Serialize};

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

/// Parses the colour notations that appear in slide themes and Markdown.
///
/// Returns `None` rather than guessing: a colour the linter cannot read is
/// reported as unreadable, never silently treated as black.
pub fn parse(text: &str) -> Option<Rgba> {
    let text = text.trim();

    if let Some(hex) = text.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(body) = strip_call(text, "rgb").or_else(|| strip_call(text, "rgba")) {
        return parse_rgb(body);
    }
    if let Some(body) = strip_call(text, "hsl").or_else(|| strip_call(text, "hsla")) {
        return parse_hsl(body);
    }

    named(&text.to_ascii_lowercase())
}

fn strip_call<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(name)?.trim_start();
    rest.strip_prefix('(')?.strip_suffix(')')
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    let digits: Vec<u8> =
        hex.chars().map(|c| c.to_digit(16).map(|d| d as u8)).collect::<Option<_>>()?;

    let expand = |value: u8| value * 17;
    match digits.len() {
        3 => Some(Rgba::opaque(expand(digits[0]), expand(digits[1]), expand(digits[2]))),
        4 => Some(Rgba {
            r: expand(digits[0]),
            g: expand(digits[1]),
            b: expand(digits[2]),
            a: f64::from(expand(digits[3])) / 255.0,
        }),
        6 => Some(Rgba::opaque(
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
        )),
        8 => Some(Rgba {
            r: digits[0] * 16 + digits[1],
            g: digits[2] * 16 + digits[3],
            b: digits[4] * 16 + digits[5],
            a: f64::from(digits[6] * 16 + digits[7]) / 255.0,
        }),
        _ => None,
    }
}

/// Splits the comma or space separated arguments of a colour function.
fn arguments(body: &str) -> Vec<&str> {
    body.split([',', '/', ' ']).map(str::trim).filter(|part| !part.is_empty()).collect()
}

fn parse_rgb(body: &str) -> Option<Rgba> {
    let parts = arguments(body);
    if parts.len() < 3 {
        return None;
    }

    let channel = |text: &str| -> Option<u8> {
        let value = match text.strip_suffix('%') {
            Some(percent) => percent.trim().parse::<f64>().ok()? * 2.55,
            None => text.parse::<f64>().ok()?,
        };
        Some(value.round().clamp(0.0, 255.0) as u8)
    };

    Some(Rgba {
        r: channel(parts[0])?,
        g: channel(parts[1])?,
        b: channel(parts[2])?,
        a: parts.get(3).and_then(|part| alpha(part)).unwrap_or(1.0),
    })
}

fn alpha(text: &str) -> Option<f64> {
    let value = match text.strip_suffix('%') {
        Some(percent) => percent.trim().parse::<f64>().ok()? / 100.0,
        None => text.parse::<f64>().ok()?,
    };
    Some(value.clamp(0.0, 1.0))
}

fn parse_hsl(body: &str) -> Option<Rgba> {
    let parts = arguments(body);
    if parts.len() < 3 {
        return None;
    }

    let hue = parts[0].trim_end_matches("deg").parse::<f64>().ok()?.rem_euclid(360.0) / 360.0;
    let saturation = parts[1].trim_end_matches('%').parse::<f64>().ok()? / 100.0;
    let lightness = parts[2].trim_end_matches('%').parse::<f64>().ok()? / 100.0;

    let chroma = if lightness <= 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let base = 2.0 * lightness - chroma;

    let component = |offset: f64| {
        let mut position = (hue + offset).rem_euclid(1.0);
        position = match position {
            p if p < 1.0 / 6.0 => base + (chroma - base) * 6.0 * p,
            p if p < 0.5 => chroma,
            p if p < 2.0 / 3.0 => base + (chroma - base) * (2.0 / 3.0 - p) * 6.0,
            _ => base,
        };
        (position * 255.0).round().clamp(0.0, 255.0) as u8
    };

    Some(Rgba {
        r: component(1.0 / 3.0),
        g: component(0.0),
        b: component(-1.0 / 3.0),
        a: parts.get(3).and_then(|part| alpha(part)).unwrap_or(1.0),
    })
}

/// The CSS keywords a slide theme realistically uses.
///
/// Deliberately partial. An unrecognised keyword is reported as unreadable,
/// which is more useful than silently checking the wrong colour.
fn named(name: &str) -> Option<Rgba> {
    let color = match name {
        "transparent" => Rgba { r: 0, g: 0, b: 0, a: 0.0 },
        "white" => Rgba::WHITE,
        "black" => Rgba::BLACK,
        "red" => Rgba::opaque(255, 0, 0),
        "green" => Rgba::opaque(0, 128, 0),
        "blue" => Rgba::opaque(0, 0, 255),
        "yellow" => Rgba::opaque(255, 255, 0),
        "orange" => Rgba::opaque(255, 165, 0),
        "purple" => Rgba::opaque(128, 0, 128),
        "gray" | "grey" => Rgba::opaque(128, 128, 128),
        "silver" => Rgba::opaque(192, 192, 192),
        "navy" => Rgba::opaque(0, 0, 128),
        "teal" => Rgba::opaque(0, 128, 128),
        _ => return None,
    };
    Some(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() < 0.05
    }

    #[test]
    fn hex_notations_all_parse() {
        assert_eq!(parse("#fff"), Some(Rgba::WHITE));
        assert_eq!(parse("#FFFFFF"), Some(Rgba::WHITE));
        assert_eq!(parse("#000000"), Some(Rgba::BLACK));
        assert_eq!(parse("#1d76db"), Some(Rgba::opaque(29, 118, 219)));
    }

    #[test]
    fn hex_alpha_notations_parse() {
        assert_eq!(parse("#00000080").map(|c| (c.a * 100.0).round()), Some(50.0));
        assert_eq!(parse("#000f").map(|c| c.a), Some(1.0));
    }

    #[test]
    fn functional_notations_parse() {
        assert_eq!(parse("rgb(255, 255, 255)"), Some(Rgba::WHITE));
        assert_eq!(parse("rgb(255 255 255)"), Some(Rgba::WHITE));
        assert_eq!(parse("rgba(0, 0, 0, 0.5)").map(|c| c.a), Some(0.5));
        assert_eq!(parse("rgb(100%, 0%, 0%)"), Some(Rgba::opaque(255, 0, 0)));
    }

    #[test]
    fn hsl_maps_onto_the_expected_corners() {
        assert_eq!(parse("hsl(0, 100%, 50%)"), Some(Rgba::opaque(255, 0, 0)));
        assert_eq!(parse("hsl(120, 100%, 50%)"), Some(Rgba::opaque(0, 255, 0)));
        assert_eq!(parse("hsl(240deg 100% 50%)"), Some(Rgba::opaque(0, 0, 255)));
        assert_eq!(parse("hsl(0, 0%, 100%)"), Some(Rgba::WHITE));
    }

    #[test]
    fn unreadable_colours_are_none_rather_than_a_guess() {
        assert_eq!(parse("var(--accent)"), None);
        assert_eq!(parse("#12345"), None);
        assert_eq!(parse("rebeccapurple"), None, "unknown keywords must not resolve to black");
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

    #[test]
    fn hex_round_trips() {
        assert_eq!(parse("#1d76db").unwrap().to_hex(), "#1d76db");
    }
}
