//! Mixing a colour instead of writing one down.
//!
//! # Why a colour space instead of a list of hexes
//!
//! A palette written as hex literals is a palette nobody can argue with, because
//! there is no argument in it — only results. Worse, it is how a borrowed palette
//! gets in: a framework's default scale *is* a list of hex literals, and pasted
//! into a theme it is indistinguishable from a decision. This repository shipped
//! one for a while without noticing, which is why `scripts/check-borrowed.mjs`
//! now fails the build on a colour that matches a framework default.
//!
//! So the colours here are not written down. They are *mixed*, from a hue, a
//! chroma and a lightness, and the hexes that reach a slide are output. Every
//! number that goes in has a reason beside it where it is declared, and changing
//! one changes the whole family the way changing a pigment would.
//!
//! Shared by the deck themes in [`crate::builtin`] and by the brand in
//! `slidx_brand`, so there is one conversion in the workspace rather than two
//! that eventually round differently.
//!
//! # Why OKLCh
//!
//! Because a palette needs a *lightness ladder*, and lightness has to mean the
//! same thing at both ends of it. In HSL it does not: `hsl(258 60% 50%)` and
//! `hsl(60 60% 50%)` are nowhere near equally light, so a ladder built in HSL is
//! even by arithmetic and uneven to the eye. OKLab was fitted to perceptual data
//! for exactly this, and its polar form gives the three knobs a palette wants:
//! which hue, how strong, how light.
//!
//! # The gamut clamp
//!
//! sRGB cannot hold every OKLCh coordinate — there is no very light, very
//! saturated blue. Asking for one and letting the channels clip would silently
//! change the *hue*, which is the one thing the palette holds constant. So a
//! colour that does not fit has its chroma reduced until it does, and its hue and
//! lightness are kept. A wash that comes out fainter than asked for is the
//! correct failure; a wash that comes out a different colour is not.

use slidx_lint::Rgba;

/// A colour as this repository thinks about it: how light, how strong, what hue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// Perceptual lightness, 0.0 black to 1.0 white.
    pub l: f64,
    /// Chroma. 0.0 is a neutral; roughly 0.16 is as strong as a mid blue gets in
    /// sRGB.
    pub c: f64,
    /// Hue angle in degrees.
    pub h: f64,
}

impl Oklch {
    pub const fn new(l: f64, c: f64, h: f64) -> Self {
        Self { l, c, h }
    }

    /// The same colour with a different chroma.
    pub fn with_chroma(self, c: f64) -> Self {
        Self { c, ..self }
    }

    /// The same colour at a different lightness.
    pub fn with_lightness(self, l: f64) -> Self {
        Self { l, ..self }
    }

    /// Converts to sRGB, reducing chroma until the result fits.
    ///
    /// Hue and lightness survive; only strength gives way. See the module note.
    pub fn to_rgba(self) -> Rgba {
        // Lightness outside 0…1 is not a colour sRGB has a darker or brighter
        // version of, so it is clamped rather than searched for. Without this a
        // lightness above 1 leaves every chroma out of gamut and the search
        // falls off its own end.
        let mut candidate = Self { l: self.l.clamp(0.0, 1.0), ..self };

        // A binary search would be shorter and wrong: the sRGB boundary is not
        // monotonic in chroma at every hue, so "in gamut" is not a predicate a
        // bisection can rely on. Stepping down from what was asked for returns
        // the strongest chroma that fits rather than one on the far side of a
        // hole in the boundary.
        while candidate.c > 0.0 {
            if let Some(rgba) = candidate.exact() {
                return rgba;
            }
            candidate.c = (candidate.c - CHROMA_STEP).max(0.0);
        }

        // Chroma zero always fits: it is a grey, and every lightness has one.
        candidate.exact().unwrap_or(Rgba::BLACK)
    }

    /// The sRGB value, or `None` when it falls outside the gamut.
    fn exact(self) -> Option<Rgba> {
        let (r, g, b) = self.to_linear_srgb();

        Some(Rgba::opaque(encode(r)?, encode(g)?, encode(b)?))
    }

    /// OKLab's inverse, then the linear sRGB matrix.
    ///
    /// The coefficients are Björn Ottosson's published values. They are copied
    /// rather than derived because deriving them needs the LMS fit they came
    /// from, and a re-derivation here would be a second source of truth for a
    /// constant.
    fn to_linear_srgb(self) -> (f64, f64, f64) {
        let hue = self.h.to_radians();
        let a = self.c * hue.cos();
        let b = self.c * hue.sin();

        let long = (self.l + 0.398_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
        let medium = (self.l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
        let short = (self.l - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);

        (
            4.076_741_662_1 * long - 3.307_711_591_3 * medium + 0.230_969_929_2 * short,
            -1.268_438_004_6 * long + 2.609_757_401_1 * medium - 0.341_319_396_5 * short,
            -0.004_196_086_3 * long - 0.703_418_614_7 * medium + 1.707_614_701_0 * short,
        )
    }
}

/// How finely the clamp steps. Half a thousandth of chroma is well below the
/// smallest difference an 8-bit channel can express, so the clamp never loses a
/// value sRGB could have held.
const CHROMA_STEP: f64 = 0.0005;

/// How far outside the range a channel may sit and still count as in gamut,
/// measured in code values.
///
/// Half of one, because a channel that far out rounds onto the boundary anyway:
/// the colour that comes back is the right one to the precision sRGB has, and
/// rejecting it would mean rejecting a colour sRGB can display. The sRGB blue
/// primary is exactly that case — its published coordinate converts back with
/// red a hair below zero.
///
/// **In code values, not in linear light**, and that distinction is the whole
/// reason this constant has a comment. The transfer function is steep near black:
/// half a code value there is about a seven-thousandth of linear light, while
/// near white it is a two-hundredth. A tolerance stated in linear light is
/// therefore either far too tight at one end or worth several visible code values
/// at the other — which showed up as the green primary coming back with a blue
/// channel of 5.
const CODE_TOLERANCE: f64 = 0.5 / 255.0;

/// Linear light to an 8-bit sRGB channel, or `None` if it is out of range.
fn encode(linear: f64) -> Option<u8> {
    // Odd-extended for negative input, which is what the sRGB specification does
    // for values outside the range. Without it a channel slightly below zero has
    // no measurable distance from the boundary, only an undefined one.
    let encoded = if linear < 0.0 { -transfer(-linear) } else { transfer(linear) };

    if !(-CODE_TOLERANCE..=1.0 + CODE_TOLERANCE).contains(&encoded) {
        return None;
    }

    Some((encoded.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// The sRGB electro-optical transfer function, forward.
fn transfer(linear: f64) -> f64 {
    if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(color: Oklch) -> String {
        color.to_rgba().to_hex()
    }

    #[test]
    fn a_neutral_at_full_lightness_is_white() {
        assert_eq!(hex(Oklch::new(1.0, 0.0, 258.0)), "#ffffff");
    }

    #[test]
    fn a_neutral_at_zero_lightness_is_black() {
        assert_eq!(hex(Oklch::new(0.0, 0.0, 258.0)), "#000000");
    }

    #[test]
    fn the_srgb_primaries_land_back_on_their_own_corners() {
        // Approximate OKLCh coordinates of the three sRGB primaries. What this
        // guards is a transcription error in the matrix, which is the one bug in
        // this module a reader cannot see: a wrong coefficient moves a channel by
        // tens or hundreds of code values, and an early version of this file
        // returned #0032e4 for the blue primary.
        //
        // The tolerance is loose deliberately. Secondary sources quote the polar
        // form to four or five decimals and disagree in the last of them, and the
        // toe of the transfer function turns a thousandth of linear light into
        // ten code values. A tight assertion here would be testing the rounding
        // of the reference figures rather than this conversion, which is why the
        // exact anchors are the grey ladder and the two endpoints instead.
        const SLACK: u8 = 12;

        for (coordinate, expected) in [
            (Oklch::new(0.6280, 0.2577, 29.23), Rgba::opaque(255, 0, 0)),
            (Oklch::new(0.8664, 0.2948, 142.50), Rgba::opaque(0, 255, 0)),
            (Oklch::new(0.4520, 0.3132, 264.05), Rgba::opaque(0, 0, 255)),
        ] {
            let got = coordinate.to_rgba();

            for (channel, (got, want)) in
                [(got.r, expected.r), (got.g, expected.g), (got.b, expected.b)]
                    .into_iter()
                    .enumerate()
            {
                assert!(
                    got.abs_diff(want) <= SLACK,
                    "channel {channel} of {} came back as {}",
                    expected.to_hex(),
                    coordinate.to_rgba().to_hex()
                );
            }
        }
    }

    #[test]
    fn a_chroma_of_zero_is_a_true_grey_at_every_lightness() {
        // A structural check on the matrix rather than on one colour: the three
        // rows have to agree exactly when there is no chroma to separate them.
        // A transcription error in any coefficient shows up here as a tint.
        for lightness in [0.1, 0.3, 0.5, 0.7, 0.9] {
            let grey = Oklch::new(lightness, 0.0, 258.0).to_rgba();
            assert_eq!(
                (grey.r, grey.g),
                (grey.g, grey.b),
                "L={lightness} came out tinted: {}",
                grey.to_hex()
            );
        }
    }

    #[test]
    fn equal_lightness_across_hues_is_equally_light() {
        // The property HSL does not have, and the reason the ladder is built
        // here rather than there. A blue and a yellow at the same OKLCh
        // lightness land within a few percent of the same relative luminance;
        // in HSL they are nowhere near.
        let blue = Oklch::new(0.6, 0.1, 258.0).to_rgba().relative_luminance();
        let yellow = Oklch::new(0.6, 0.1, 100.0).to_rgba().relative_luminance();

        assert!(
            (blue - yellow).abs() < 0.04,
            "blue {blue:.3} and yellow {yellow:.3} are not equally light"
        );
    }

    #[test]
    fn a_lighter_lightness_is_a_lighter_colour() {
        let ladder: Vec<f64> = [0.2, 0.4, 0.6, 0.8]
            .into_iter()
            .map(|l| Oklch::new(l, 0.05, 258.0).to_rgba().relative_luminance())
            .collect();

        for pair in ladder.windows(2) {
            assert!(pair[0] < pair[1], "the ladder is not monotonic: {ladder:?}");
        }
    }

    #[test]
    fn an_out_of_gamut_chroma_gives_up_strength_and_keeps_its_hue() {
        // There is no very light, very saturated blue. Letting the channels clip
        // would change the hue, which is the one thing the palette holds fixed,
        // so the chroma comes down instead.
        let asked = Oklch::new(0.97, 0.3, 258.0);
        let got = asked.to_rgba();

        // Still recognisably the same hue: blue is the largest channel and the
        // ordering of the three is unchanged.
        assert!(got.b >= got.g && got.g >= got.r, "the hue moved: {}", got.to_hex());
        assert!(got.r > 200, "a near-white lightness did not survive: {}", got.to_hex());
    }

    #[test]
    fn a_colour_inside_the_gamut_is_returned_at_the_chroma_it_asked_for() {
        // The clamp must not quietly desaturate everything. A mid blue at a
        // chroma sRGB can hold has to come back exact.
        let inside = Oklch::new(0.42, 0.154, 258.0);

        assert_eq!(inside.exact().map(|rgba| rgba.to_hex()), Some(inside.to_rgba().to_hex()));
    }

    #[test]
    fn the_clamp_terminates_on_a_lightness_no_chroma_can_reach() {
        // Above 1.0 nothing fits at any chroma. The loop has to end rather than
        // stepping chroma down forever.
        assert_eq!(hex(Oklch::new(1.4, 0.3, 258.0)), "#ffffff");
    }

    #[test]
    fn the_builders_change_one_axis_and_leave_the_others() {
        let base = Oklch::new(0.5, 0.1, 258.0);

        assert_eq!(base.with_chroma(0.02), Oklch::new(0.5, 0.02, 258.0));
        assert_eq!(base.with_lightness(0.9), Oklch::new(0.9, 0.1, 258.0));
    }

    #[test]
    fn mixing_is_deterministic() {
        // The tokens are committed, so a colour that differed run to run would
        // rewrite every generated file for nothing.
        let color = Oklch::new(0.42, 0.154, 258.0);
        assert_eq!(color.to_rgba(), color.to_rgba());
    }
}
