//! Whether text can be read from the back of the room.
//!
//! "Body text must be at least 28px" is the usual advice, and it is only true
//! for one canvas size at one viewing distance. A deck authored on a 980px-wide
//! canvas and one authored at 1920 do not mean the same thing by `28px`.
//!
//! So the real quantity is **angular size**: how large a glyph is on the
//! audience's retina. That is independent of canvas units, screen size, and
//! resolution, and it is what actually determines legibility.

use serde::{Deserialize, Serialize};

/// The room, as far as legibility is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewingProfile {
    /// Physical height of the projected image, in metres.
    pub screen_height_m: f64,
    /// Distance from the screen to the back row, in metres.
    pub back_row_m: f64,
}

impl Default for ViewingProfile {
    /// A mid-size conference track room: a 2.5m screen with the back row at
    /// 15m, which is six screen heights — the conventional upper bound for a
    /// room where text is expected to be readable.
    fn default() -> Self {
        Self { screen_height_m: 2.5, back_row_m: 15.0 }
    }
}

impl ViewingProfile {
    /// A small meeting room: a 1.5m screen with the back row at 6m.
    pub const MEETING_ROOM: Self = Self { screen_height_m: 1.5, back_row_m: 6.0 };

    /// A large hall: a 5m screen with the back row at 35m.
    pub const HALL: Self = Self { screen_height_m: 5.0, back_row_m: 35.0 };

    /// Screen heights between the screen and the back row.
    ///
    /// The single number venue guides quote. Above about six, text-heavy
    /// slides stop working regardless of how they are set.
    pub fn distance_ratio(self) -> f64 {
        self.back_row_m / self.screen_height_m
    }
}

/// What a piece of text is doing, which sets how large it has to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextRole {
    Heading,
    Body,
    /// Denser glyphs and no redundancy from context, so it needs more size
    /// than prose at the same importance.
    Code,
    /// Captions, footers, and attributions: not required reading.
    Caption,
}

impl TextRole {
    /// Minimum angular size, in arcminutes.
    ///
    /// Calibrated so that [`TextRole::Body`] reproduces the familiar
    /// "28px on a 1080-tall canvas" floor in a [`ViewingProfile::default`]
    /// room. The published rule of thumb and this model therefore agree where
    /// the rule of thumb applies, and this model keeps working where it does
    /// not.
    pub fn min_arcmin(self) -> f64 {
        match self {
            Self::Heading => 18.0,
            Self::Body => 14.8,
            Self::Code => 16.0,
            Self::Caption => 11.0,
        }
    }

    pub fn as_token(self) -> &'static str {
        match self {
            Self::Heading => "heading",
            Self::Body => "body",
            Self::Code => "code",
            Self::Caption => "caption",
        }
    }
}

/// Angular size of a glyph, in arcminutes, seen from the back row.
///
/// `font_px` and `canvas_height_px` are both in the deck's design space, so
/// the ratio between them is all that matters — which is exactly why this
/// works across canvas sizes.
pub fn angular_size_arcmin(font_px: f64, canvas_height_px: f64, profile: ViewingProfile) -> f64 {
    if canvas_height_px <= 0.0 || profile.back_row_m <= 0.0 {
        return 0.0;
    }

    let physical_height_m = font_px / canvas_height_px * profile.screen_height_m;
    let radians = 2.0 * (physical_height_m / (2.0 * profile.back_row_m)).atan();

    radians.to_degrees() * 60.0
}

/// Smallest design-space font size that stays readable for this role.
///
/// The inverse of [`angular_size_arcmin`], so the linter can report a target
/// rather than only a failure.
pub fn min_font_px(role: TextRole, canvas_height_px: f64, profile: ViewingProfile) -> f64 {
    if profile.screen_height_m <= 0.0 {
        return f64::INFINITY;
    }

    let radians = (role.min_arcmin() / 60.0).to_radians();
    let physical_height_m = 2.0 * profile.back_row_m * (radians / 2.0).tan();

    physical_height_m / profile.screen_height_m * canvas_height_px
}

/// Verdict for one piece of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Legibility {
    /// Comfortable from the back row.
    Comfortable,
    /// Readable from the back row, but with effort.
    Tight,
    /// Not readable from the back row.
    Unreadable,
}

/// Classifies a font size for a role.
///
/// The `Tight` band exists because a hard pass/fail at one threshold turns a
/// judgement call into a build failure. Text a little under the floor is worth
/// a warning; text far under it is worth stopping for.
pub fn classify(
    role: TextRole,
    font_px: f64,
    canvas_height_px: f64,
    profile: ViewingProfile,
) -> Legibility {
    let actual = angular_size_arcmin(font_px, canvas_height_px, profile);
    let floor = role.min_arcmin();

    if actual >= floor {
        Legibility::Comfortable
    } else if actual >= floor * 0.8 {
        Legibility::Tight
    } else {
        Legibility::Unreadable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANVAS: f64 = 1080.0;

    #[test]
    fn the_model_reproduces_the_published_28px_floor() {
        // The whole calibration rests on this: in a default room, 28px on a
        // 1080 canvas must land exactly on the body-text threshold.
        let minimum = min_font_px(TextRole::Body, CANVAS, ViewingProfile::default());
        assert!((minimum - 28.0).abs() < 0.5, "expected about 28px, got {minimum:.1}");
    }

    #[test]
    fn angular_size_and_min_font_px_are_inverses() {
        let profile = ViewingProfile::default();

        for role in [TextRole::Heading, TextRole::Body, TextRole::Code, TextRole::Caption] {
            let minimum = min_font_px(role, CANVAS, profile);
            let arcmin = angular_size_arcmin(minimum, CANVAS, profile);
            assert!(
                (arcmin - role.min_arcmin()).abs() < 0.01,
                "{} round trip: {arcmin} vs {}",
                role.as_token(),
                role.min_arcmin()
            );
        }
    }

    #[test]
    fn the_same_ratio_gives_the_same_verdict_on_any_canvas() {
        // A 980-wide canvas and a 1920-wide canvas do not mean the same thing
        // by "28px", and this is the property that makes the linter portable.
        let profile = ViewingProfile::default();
        let small = angular_size_arcmin(14.0, 540.0, profile);
        let large = angular_size_arcmin(56.0, 2160.0, profile);

        assert!((small - large).abs() < 0.001);
    }

    #[test]
    fn a_deck_authored_on_a_small_canvas_needs_a_smaller_number() {
        let profile = ViewingProfile::default();
        let on_1080 = min_font_px(TextRole::Body, 1080.0, profile);
        let on_552 = min_font_px(TextRole::Body, 552.0, profile);

        assert!(on_552 < on_1080);
        assert!((on_552 / on_1080 - 552.0 / 1080.0).abs() < 0.001);
    }

    #[test]
    fn a_bigger_room_demands_bigger_text() {
        let default_room = min_font_px(TextRole::Body, CANVAS, ViewingProfile::default());
        let hall = min_font_px(TextRole::Body, CANVAS, ViewingProfile::HALL);
        let meeting = min_font_px(TextRole::Body, CANVAS, ViewingProfile::MEETING_ROOM);

        assert!(hall > default_room, "a hall needs more than a track room");
        assert!(meeting < default_room, "a meeting room needs less");
    }

    #[test]
    fn code_and_headings_have_stricter_floors_than_body_text() {
        let profile = ViewingProfile::default();
        let body = min_font_px(TextRole::Body, CANVAS, profile);

        assert!(min_font_px(TextRole::Code, CANVAS, profile) > body, "code is denser");
        assert!(min_font_px(TextRole::Heading, CANVAS, profile) > body);
        assert!(min_font_px(TextRole::Caption, CANVAS, profile) < body);
    }

    #[test]
    fn classification_has_a_warning_band_between_pass_and_fail() {
        let profile = ViewingProfile::default();

        assert_eq!(classify(TextRole::Body, 32.0, CANVAS, profile), Legibility::Comfortable);
        assert_eq!(classify(TextRole::Body, 28.0, CANVAS, profile), Legibility::Comfortable);
        assert_eq!(classify(TextRole::Body, 24.0, CANVAS, profile), Legibility::Tight);
        assert_eq!(classify(TextRole::Body, 18.0, CANVAS, profile), Legibility::Unreadable);
    }

    #[test]
    fn the_distance_ratio_matches_the_number_venue_guides_quote() {
        assert!((ViewingProfile::default().distance_ratio() - 6.0).abs() < 0.001);
        assert!((ViewingProfile::MEETING_ROOM.distance_ratio() - 4.0).abs() < 0.001);
        assert!((ViewingProfile::HALL.distance_ratio() - 7.0).abs() < 0.001);
    }

    #[test]
    fn degenerate_inputs_do_not_panic_or_produce_nonsense() {
        let profile = ViewingProfile::default();

        assert_eq!(angular_size_arcmin(28.0, 0.0, profile), 0.0);
        assert_eq!(angular_size_arcmin(0.0, CANVAS, profile), 0.0);
        assert_eq!(
            angular_size_arcmin(28.0, CANVAS, ViewingProfile { back_row_m: 0.0, ..profile }),
            0.0
        );
        assert!(min_font_px(
            TextRole::Body,
            CANVAS,
            ViewingProfile { screen_height_m: 0.0, ..profile }
        )
        .is_infinite());
    }

    #[test]
    fn angular_size_grows_with_font_size() {
        let profile = ViewingProfile::default();
        let mut previous = 0.0;

        for size in [12.0, 24.0, 36.0, 72.0] {
            let arcmin = angular_size_arcmin(size, CANVAS, profile);
            assert!(arcmin > previous);
            previous = arcmin;
        }
    }
}
