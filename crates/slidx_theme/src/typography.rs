//! Leading, tracking, and measure — the three numbers a type scale does not give.
//!
//! [`TypeScale`](crate::TypeScale) answers how large. It does not answer how far
//! apart the lines sit, how tightly the letters do, or how long a line is
//! allowed to get, and those three decide whether a slide reads as set or as
//! typed. They were constants in the shell stylesheet: `line-height: 1.5` on the
//! body, `1.15` on every heading, `letter-spacing: -0.015em` on every heading,
//! `max-width: 22ch`. One set of numbers, applied to every theme.
//!
//! That is the shape of a near-miss. It is right for one size on one scale in
//! one script, and every theme that moves the base or the ratio inherits
//! numbers fitted to a theme it is not.
//!
//! # The two curves
//!
//! Both quantities move with size, in opposite directions, and both are
//! logarithmic for the same reason the type scale is geometric: type is
//! compared optically, and equal-looking steps need a constant ratio.
//!
//! **Tracking closes as type grows.** Sidebearings scale with the glyph; the
//! eye's tolerance for the gap between two letters does not. A face set at
//! display size therefore looks loose at the tracking that suits it at text
//! size, and one set small looks cramped. So tracking is zero at the scale's
//! own base and moves from there — negative above, *positive below*. The
//! positive half is the part a constant cannot express and the part a caption
//! needs.
//!
//! **Leading closes as type grows,** for the same reason and one more: leading
//! is set against the line length below it, and a heading is short. The curve
//! is anchored at the base too, so a theme states one leading and gets a
//! coherent set.
//!
//! Both response constants are calibrated so the default scale reproduces the
//! numbers the stylesheet had been using at display size — the same move
//! [`slidx_lint`]'s arcmin floors make against the published 28px rule of
//! thumb. Where the old constants were right, this agrees with them; where a
//! theme moved its base or its ratio, this keeps working and they did not.
//!
//! # Measure, and why it is one number for two scripts
//!
//! A line is too long when the eye loses its place returning to the next one,
//! and the readable range is conventionally quoted in characters: 45–75 for
//! Latin prose, fewer for display type, which is scanned rather than read.
//!
//! Quoted in characters it is two numbers, because a CJK line does not hold as
//! many: a Han glyph or a kana occupies a full em where a Latin lowercase
//! averages about half of one. But it also *carries* about twice as much —
//! Japanese renders into roughly twice the character count in English. Those
//! two ratios are both about two, and that is not a coincidence: a denser glyph
//! is denser in both senses.
//!
//! So they cancel, and the measure is **one length in `em`** rather than two
//! counts in characters. Thirty em is sixty Latin characters or thirty Japanese
//! ones, and those are the same sentence.
//!
//! Stating it in `em` earns the rest for free. `em` resolves against the
//! element's own size, so one declaration is the right measure on a heading and
//! on a caption without either naming a size; `ch` — the advance of `0` — would
//! have been a Latin metric imposed on both scripts, which is precisely how
//! `22ch` came to break a Japanese heading after ten characters.
//!
//! The honest bound: the 2:1 expansion figure is an average over prose, and a
//! line of Japanese that is mostly katakana loanwords carries less per glyph
//! than one that is mostly kanji. This is a measure, not a guarantee, and it is
//! a cap rather than a target — a line shorter than the cap is never made
//! longer.
//!
//! # Script
//!
//! Two things stay script-dependent after that, and both are physical.
//!
//! **Tracking cannot be negative on CJK.** A kanji is drawn to fill its em box
//! with almost no sidebearing, so tracking that a Latin face has room to give
//! up comes directly out of the space between strokes. The mechanism that does
//! for CJK what negative tracking does for Latin is `palt`, which asks the font
//! for the proportional advances it already contains, and that is a different
//! declaration rather than a smaller number.
//!
//! **CJK needs more leading at the same ratio.** Latin line separation is
//! partly done by the whitespace above the x-height and below the baseline,
//! which is why 1.5 looks generous there. A CJK line is a run of filled em
//! boxes with no such margin, so the same ratio yields visually tighter lines;
//! Japanese practice puts 行送り at 1.5–2.0em for text set to be read.

use serde::{Deserialize, Serialize};

/// A script, as far as setting type is concerned.
///
/// Two entries rather than a list of writing systems, because only one
/// distinction reaches these numbers: whether a glyph is drawn proportionally
/// inside sidebearings it can give up, or as a filled em box it cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Script {
    /// Latin, Greek, Cyrillic: proportional advances, and a space between
    /// words that is also the only place a line may break.
    Latin,
    /// Han, kana, and Hangul: one em box per glyph, no interword space, and a
    /// break legal at almost every position.
    Cjk,
}

impl Script {
    /// The BCP 47 primary language subtags this crate treats as CJK.
    ///
    /// Matched as a prefix, so `ja`, `ja-JP` and `zh-Hant` all land here. Kept
    /// short deliberately: this list decides which decks get different leading,
    /// and a language added on a guess would change how somebody's deck is set
    /// for a reason nobody could name.
    pub const CJK_TAGS: [&'static str; 3] = ["ja", "zh", "ko"];

    /// Which script a document language belongs to.
    ///
    /// Anything unrecognised is [`Script::Latin`], which is where the numbers
    /// that were already shipping live. An unknown language gets what it got
    /// before rather than an experiment.
    pub fn of_lang(lang: &str) -> Self {
        let tag = lang.split(['-', '_']).next().unwrap_or(lang).to_ascii_lowercase();

        if Self::CJK_TAGS.contains(&tag.as_str()) {
            Self::Cjk
        } else {
            Self::Latin
        }
    }

    pub fn as_token(self) -> &'static str {
        match self {
            Self::Latin => "latin",
            Self::Cjk => "cjk",
        }
    }
}

/// How far a CJK line is opened beyond the Latin leading at the same size.
///
/// Japanese practice puts 行送り at 1.5–2.0em for text meant to be read, against
/// the 1.4–1.6 that reads as generous in Latin. The offset lands the default
/// scale's body at 1.7 and its largest heading at about 1.35, both inside the
/// range each is quoted at.
///
/// An offset rather than a factor: the reason CJK needs more room is the
/// missing ascender and descender whitespace, which is a fixed share of the em
/// and does not grow when the leading does.
pub const CJK_LEADING_OFFSET: f64 = 0.2;

/// Leading, tracking, and measure, as one theme decision.
///
/// Every field is a value at, or a response around, the scale's own base — so a
/// theme states what it wants at body size and the rest of the ladder follows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Typography {
    /// Leading at the scale's base size, as a multiple of the font size.
    pub base_leading: f64,
    /// How much leading changes per natural-log step of size.
    ///
    /// Calibrated so the default scale's largest heading lands on the 1.15 the
    /// shell stylesheet had been using.
    pub leading_response: f64,
    /// How much tracking changes per natural-log step of size, in em.
    ///
    /// Calibrated so the default scale's largest heading lands on the −0.015em
    /// the shell stylesheet had been using.
    pub tracking_response: f64,
    /// Longest a line of prose may get, in em.
    pub prose_measure_em: f64,
    /// Longest a heading line may get, in em.
    ///
    /// Shorter than prose because display type is scanned rather than read, and
    /// a heading that runs to a prose measure stops reading as one line.
    pub heading_measure_em: f64,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            base_leading: 1.5,
            leading_response: 0.52,
            tracking_response: 0.022,
            prose_measure_em: 30.0,
            heading_measure_em: 13.0,
        }
    }
}

/// Tightest leading this crate will emit.
///
/// Below roughly 1.05 the ascenders of one line meet the descenders of the one
/// above at any normal face, which is a collision rather than a tight setting.
/// A theme with an extreme ratio should not be able to reach it by arithmetic.
const MIN_LEADING: f64 = 1.05;

/// Loosest leading this crate will emit.
///
/// Past about two the lines stop reading as one block of text. The ceiling
/// matters for the small end of a wide scale, where the curve keeps opening.
const MAX_LEADING: f64 = 2.0;

impl Typography {
    /// Leading for text set at `size_px`, on a scale whose base is `base_px`.
    pub fn leading(&self, size_px: f64, base_px: f64, script: Script) -> f64 {
        let latin = self.base_leading + self.leading_response * steps(base_px, size_px);

        let opened = match script {
            Script::Latin => latin,
            Script::Cjk => latin + CJK_LEADING_OFFSET,
        };

        opened.clamp(MIN_LEADING, MAX_LEADING)
    }

    /// Tracking for text set at `size_px`, in em, on a scale whose base is
    /// `base_px`.
    ///
    /// Zero on CJK at every size, rather than a smaller negative number. The
    /// space a Latin face gives up to negative tracking is sidebearing a kanji
    /// does not have, so any amount of it is stroke-to-stroke collision — see
    /// [`palt`](self#script) for the declaration that does this job instead.
    pub fn tracking_em(&self, size_px: f64, base_px: f64, script: Script) -> f64 {
        match script {
            Script::Cjk => 0.0,
            Script::Latin => self.tracking_response * steps(base_px, size_px),
        }
    }
}

/// Natural-log steps from `base_px` up to `size_px`, negated.
///
/// Negative above the base and positive below, so both curves read the same
/// way: multiply by a response and add. Degenerate sizes give zero rather than
/// an infinity, because a theme is data and a theme package is data somebody
/// else wrote.
fn steps(base_px: f64, size_px: f64) -> f64 {
    if base_px <= 0.0 || size_px <= 0.0 {
        return 0.0;
    }

    (base_px / size_px).ln()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::TypeScale;

    fn scale() -> TypeScale {
        TypeScale::default()
    }

    #[test]
    fn the_default_leading_reproduces_the_constant_the_stylesheet_shipped() {
        // The calibration the whole module rests on: on the default scale, the
        // largest heading must land on the 1.15 that was hard-coded, so this
        // model agrees with the old numbers exactly where they were right.
        let scale = scale();
        let leading =
            Typography::default().leading(scale.heading_px(1), scale.base_px, Script::Latin);

        assert!((leading - 1.15).abs() < 0.01, "expected about 1.15, got {leading:.3}");
    }

    #[test]
    fn the_default_tracking_reproduces_the_constant_the_stylesheet_shipped() {
        let scale = scale();
        let tracking =
            Typography::default().tracking_em(scale.heading_px(1), scale.base_px, Script::Latin);

        assert!((tracking + 0.015).abs() < 0.001, "expected about -0.015em, got {tracking:.4}");
    }

    #[test]
    fn body_text_sits_exactly_on_the_theme_s_stated_leading() {
        // The base is where the theme's own number applies unmodified. If it
        // did not, `base_leading` would be a parameter of a curve rather than
        // a decision anybody could read.
        let scale = scale();
        let typography = Typography::default();

        assert_eq!(typography.leading(scale.base_px, scale.base_px, Script::Latin), 1.5);
        assert_eq!(typography.tracking_em(scale.base_px, scale.base_px, Script::Latin), 0.0);
    }

    #[test]
    fn leading_closes_as_type_grows() {
        let scale = scale();
        let typography = Typography::default();
        let mut previous = f64::INFINITY;

        for size in [scale.caption_px(), scale.base_px, scale.heading_px(3), scale.heading_px(1)] {
            let leading = typography.leading(size, scale.base_px, Script::Latin);
            assert!(leading < previous, "leading did not close at {size}px: {leading}");
            previous = leading;
        }
    }

    #[test]
    fn tracking_closes_as_type_grows_and_opens_below_the_base() {
        // The half a constant cannot express: a caption wants *more* tracking
        // than body text, not the same amount and not less.
        let scale = scale();
        let typography = Typography::default();

        assert!(typography.tracking_em(scale.heading_px(1), scale.base_px, Script::Latin) < 0.0);
        assert!(typography.tracking_em(scale.caption_px(), scale.base_px, Script::Latin) > 0.0);
    }

    #[test]
    fn a_theme_that_moves_its_base_gets_different_numbers() {
        // The defect this module exists to fix. `editorial` sets a 34px base
        // and a 1.333 ratio; sharing a constant with a 32/1.25 theme means one
        // of the two is set to numbers fitted to the other.
        let calm = TypeScale { base_px: 32.0, ratio: 1.25, code_factor: 1.0 };
        let dramatic = TypeScale { base_px: 34.0, ratio: 1.333, code_factor: 0.95 };
        let typography = Typography::default();

        let calm_h1 = typography.leading(calm.heading_px(1), calm.base_px, Script::Latin);
        let dramatic_h1 =
            typography.leading(dramatic.heading_px(1), dramatic.base_px, Script::Latin);

        assert!(
            dramatic_h1 < calm_h1,
            "a wider scale reaches further from its base and must close further: \
             {dramatic_h1:.3} vs {calm_h1:.3}"
        );
    }

    #[test]
    fn cjk_is_set_more_open_at_every_size() {
        let scale = scale();
        let typography = Typography::default();

        for size in [scale.caption_px(), scale.base_px, scale.heading_px(1)] {
            let latin = typography.leading(size, scale.base_px, Script::Latin);
            let cjk = typography.leading(size, scale.base_px, Script::Cjk);
            assert!(cjk > latin, "CJK must open at {size}px: {cjk:.3} vs {latin:.3}");
        }
    }

    #[test]
    fn cjk_body_leading_lands_inside_the_range_japanese_practice_quotes() {
        let scale = scale();
        let leading = Typography::default().leading(scale.base_px, scale.base_px, Script::Cjk);

        assert!((1.5..=2.0).contains(&leading), "行送り out of range: {leading:.3}");
    }

    #[test]
    fn cjk_tracking_is_never_negative() {
        // Sidebearing a Latin face can give up is stroke-to-stroke space on a
        // kanji. Any negative value here is a collision on a projector.
        let scale = scale();
        let typography = Typography::default();

        for size in [scale.caption_px(), scale.base_px, scale.heading_px(1), 400.0] {
            assert_eq!(typography.tracking_em(size, scale.base_px, Script::Cjk), 0.0);
        }
    }

    #[test]
    fn leading_is_clamped_before_lines_could_collide() {
        // A theme package is JSON somebody else wrote, and an extreme response
        // must not be able to produce a setting that overlaps.
        let wild = Typography { leading_response: 5.0, ..Typography::default() };

        assert_eq!(wild.leading(400.0, 32.0, Script::Latin), MIN_LEADING);
        assert_eq!(wild.leading(1.0, 32.0, Script::Latin), MAX_LEADING);
    }

    #[test]
    fn the_measure_is_shorter_for_a_heading_than_for_prose() {
        let typography = Typography::default();
        assert!(typography.heading_measure_em < typography.prose_measure_em);
    }

    #[test]
    fn the_prose_measure_holds_both_scripts_inside_their_readable_range() {
        // The claim the single number rests on. A Latin lowercase averages
        // about half an em and a CJK glyph exactly one, so the same length is
        // two character counts — and both have to be readable, or this should
        // have been two numbers.
        let em = Typography::default().prose_measure_em;

        let latin_chars = em / 0.5;
        let cjk_chars = em;

        assert!((45.0..=75.0).contains(&latin_chars), "Latin measure: {latin_chars} characters");
        assert!((20.0..=40.0).contains(&cjk_chars), "CJK measure: {cjk_chars} characters");
    }

    #[test]
    fn a_language_tag_resolves_to_a_script() {
        assert_eq!(Script::of_lang("ja"), Script::Cjk);
        assert_eq!(Script::of_lang("ja-JP"), Script::Cjk);
        assert_eq!(Script::of_lang("zh-Hant"), Script::Cjk);
        assert_eq!(Script::of_lang("ko"), Script::Cjk);
        assert_eq!(Script::of_lang("en"), Script::Latin);
        assert_eq!(Script::of_lang("en-GB"), Script::Latin);
        assert_eq!(Script::of_lang("de"), Script::Latin);
    }

    #[test]
    fn an_unknown_language_is_set_the_way_it_was_set_before() {
        // Unrecognised is Latin, which is where the shipping numbers live. A
        // language nobody has thought about gets what it already got.
        assert_eq!(Script::of_lang(""), Script::Latin);
        assert_eq!(Script::of_lang("xx-YY"), Script::Latin);
        assert_eq!(Script::of_lang("JA"), Script::Cjk, "a tag is case-insensitive");
    }

    #[test]
    fn degenerate_sizes_do_not_produce_an_infinity() {
        let typography = Typography::default();

        assert_eq!(typography.leading(0.0, 32.0, Script::Latin), typography.base_leading);
        assert_eq!(typography.leading(32.0, 0.0, Script::Latin), typography.base_leading);
        assert_eq!(typography.tracking_em(0.0, 32.0, Script::Latin), 0.0);
        assert_eq!(typography.tracking_em(32.0, -1.0, Script::Latin), 0.0);
    }

    #[test]
    fn a_theme_package_written_before_typography_existed_still_loads() {
        // Same guarantee `motion` has: a third-party theme is a JSON file
        // somebody else owns and does not republish.
        let json = serde_json::to_value(Typography::default()).unwrap();
        let round_tripped: Typography = serde_json::from_value(json).unwrap();

        assert_eq!(round_tripped, Typography::default());
    }
}
