//! How a built-in theme's colours are arrived at.
//!
//! # What this replaces
//!
//! The four built-in themes used to be hex literals, and they were a framework's
//! default scales pasted in: `#18181b`, `#09090b`, `#52525b`, `#e4e4e7`,
//! `#f4f4f5` are one popular CSS framework's `zinc` ramp in order, `#1d4ed8` is
//! its `blue-700`, and `#5b21b6` is its `violet-800`. Nothing about those values
//! said anything about slidx, which is exactly why the result looked generated: a
//! palette every machine reaches for carries no information about what it is on.
//!
//! There was a second, worse consequence. Because the values were pasted, the
//! *syntax* palettes were pasted between themes too — `minimal` and `terminal`
//! shipped byte-identical light syntax colours, and three of the four themes
//! shipped the same `#5b21b6` for a type name. A theme system whose headline
//! feature is code on slides was not theming code at all.
//!
//! # The recipe
//!
//! A theme now declares **two hues and two lightnesses**, and everything else is
//! solved:
//!
//! - the **accent hue**, which is the theme's identity, and how strong it is;
//! - a **wash**, the fraction of that chroma the neutrals keep, so a theme's greys
//!   belong to its accent rather than standing beside it;
//! - the lightness of the **sheet** the slide is drawn on, and of the **ink** on
//!   it.
//!
//! Every other surface role is a stated offset from those two, and every role that
//! has a contrast floor is walked until [`slidx_lint`] says it clears the floor in
//! a bright room. That is the same arrangement the brand uses and the same one
//! `TypeScale::code_factor` documents: the audit decides, not a constant.
//!
//! # Syntax colours keep their meanings and lose their values
//!
//! The six roles keep the same *hues* in every theme — comments neutral, strings
//! green, numbers warm, keywords blue, types plum — because a reader who has
//! learned one slidx deck should be able to read the next one. What changes per
//! theme is the **lightness each hue lands at**, solved against that theme's own
//! code surface. That is what the old doc comment claimed and the old hexes did
//! not do.

use slidx_lint::{projected_contrast_ratio, ProjectorProfile, Rgba};

use crate::mix::Oklch;
use crate::palette::{Palette, SyntaxPalette};

/// How much better than its floor a solved colour has to be.
///
/// A colour landing exactly on 4.5:1 is one rounding away from failing its own
/// audit, which would make the theme's test suite depend on the order of two
/// floating-point operations.
const MARGIN: f64 = 1.04;

/// Lightness the solver moves by, well below what 8 bits resolve.
const STEP: f64 = 0.004;

/// The room a theme's colours have to survive.
///
/// The brightest slidx models. A theme that clears this clears the others, so
/// solving here means the audit's other profiles are confirmations rather than
/// separate constraints.
const ROOM: ProjectorProfile = ProjectorProfile::BrightRoom;

/// The four *categorical* hues, shared by every theme.
///
/// Shared on purpose: a reader who has learned that green is a string in one
/// slidx deck should not have to learn it again in the next. Only the lightness
/// moves per theme, solved against that theme's own code surface.
///
/// Comments and punctuation are deliberately **not** here. Neither is a category
/// the reader looks up — they are the material the other four sit in — so both
/// are mixed from the theme's own hue and therefore differ between themes.
///
/// The type hue is a **plum at 330°** rather than the violet it used to be. A
/// violet is the conventional choice and it was also the borrowed value that
/// started this rewrite; 330 keeps a type distinguishable from a keyword at a
/// glance while being a colour somebody chose.
mod hue {
    pub const STRING: f64 = 150.0;
    pub const NUMBER: f64 = 65.0;
    pub const KEYWORD: f64 = 258.0;
    pub const TYPE: f64 = 330.0;
}

/// How strong a syntax colour is.
///
/// Enough to tell six roles apart, not enough to turn a code block into a
/// rainbow. Comments and punctuation are washes of the theme's own hue instead,
/// because neither is a category the reader looks up — they are the material the
/// other four sit in.
const SYNTAX_CHROMA: f64 = 0.11;

/// What a theme declares, before anything is solved.
#[derive(Debug, Clone, Copy)]
pub struct Recipe {
    /// The theme's identity, in OKLCh degrees.
    pub hue: f64,
    /// How strong the accent is asked to be.
    pub accent_chroma: f64,
    /// Fraction of `accent_chroma` the neutrals keep.
    ///
    /// Zero is a legitimate answer and `contrast` uses it: a theme whose whole
    /// job is separation should not spend any of its range on a tint.
    pub wash: f64,
    /// Lightness of the sheet a slide is drawn on.
    pub sheet: f64,
    /// Lightness of the ink on it.
    pub ink: f64,
}

impl Recipe {
    fn neutral_chroma(self) -> f64 {
        self.accent_chroma * self.wash
    }

    /// A lightness `offset` from the sheet in the direction of the ink.
    fn toward_ink(self, offset: f64) -> f64 {
        if self.is_dark() {
            self.sheet + offset
        } else {
            self.sheet - offset
        }
    }

    fn neutral(self, lightness: f64) -> Rgba {
        Oklch::new(lightness, self.neutral_chroma(), self.hue).to_rgba()
    }

    /// True when the sheet is darker than the ink, i.e. this is a dark scheme.
    ///
    /// Everything that has to gain contrast moves away from the sheet, and which
    /// direction that is depends only on this.
    fn is_dark(self) -> bool {
        self.ink > self.sheet
    }

    /// Walks a colour away from the sheet until it clears `floor` in a bright
    /// room.
    fn solved(self, sheet: Rgba, start: f64, chroma: f64, hue: f64, floor: f64) -> Rgba {
        let target = floor * MARGIN;
        let step = if self.is_dark() { STEP } else { -STEP };
        let mut lightness = start;

        while (0.0..=1.0).contains(&lightness) {
            let candidate = Oklch::new(lightness, chroma, hue).to_rgba();
            if projected_contrast_ratio(candidate, sheet, ROOM) >= target {
                return candidate;
            }
            lightness += step;
        }

        // The ends of the range, which every floor in use is reachable well
        // inside. Returning a bound rather than panicking keeps a third-party
        // recipe from taking a deck down.
        Oklch::new(if self.is_dark() { 1.0 } else { 0.0 }, chroma, hue).to_rgba()
    }

    /// The whole palette for one scheme.
    pub fn palette(self) -> Palette {
        let sheet = self.neutral(self.sheet);
        // The canvas is what shows as letterboxing when the aspect does not fit,
        // and it is darker than the sheet in *both* schemes rather than a step in
        // some direction. A projected slide is the lit thing in the room and the
        // surround is the unlit wall; a canvas lighter than the slide would read
        // as a hole cut in a bright surface, which is not what anyone is looking
        // at.
        let canvas = self.neutral((self.sheet - CANVAS_OFFSET).max(0.0));

        // A code block is a panel on the sheet: one small step towards the ink,
        // which reads as inset without needing a border to say so.
        let code_surface = self.neutral(self.toward_ink(PANEL_OFFSET));
        let ink = self.neutral(self.ink);

        Palette {
            canvas,
            surface: sheet,
            text: ink,
            // A heading is the same ink pushed one step further from the sheet,
            // not a second colour. Clamped at both ends, because `contrast`
            // already sets its ink at the end of the ladder and has nowhere
            // further to go — there a heading and body text are the same value,
            // which is correct for a theme whose whole job is separation.
            heading: self.neutral(if self.is_dark() {
                (self.ink + HEADING_OFFSET).min(1.0)
            } else {
                (self.ink - HEADING_OFFSET).max(0.0)
            }),
            muted: self.solved(
                sheet,
                self.toward_ink(MUTED_START),
                self.neutral_chroma(),
                self.hue,
                3.0,
            ),
            accent: self.solved(
                sheet,
                self.toward_ink(ACCENT_START),
                self.accent_chroma,
                self.hue,
                4.5,
            ),
            // A hairline, and the only role with no floor: a border held to a text
            // floor is a border loud enough to read as a rule.
            border: self.neutral(self.toward_ink(BORDER_OFFSET)),
            code_surface,
            code_text: self.solved(code_surface, self.ink, self.neutral_chroma(), self.hue, 4.5),
            syntax: Some(self.syntax(code_surface)),
        }
    }

    /// Six colours solved against the code surface they will actually sit on.
    ///
    /// Against the code surface rather than the slide, because a comment checked
    /// against the sheet would pass while being invisible everywhere it is shown.
    fn syntax(self, code_surface: Rgba) -> SyntaxPalette {
        // Every syntax colour starts its search from the middle of the range
        // rather than from the ink, and that is not a detail. At the ends of the
        // ladder no hue can hold any chroma — `contrast` sets its dark ink at
        // full white — so a search starting there returns white for all four
        // categorical hues and the theme highlights nothing. Starting mid-range
        // and walking away from the surface finds the first lightness at which
        // each hue *both* carries colour and clears the floor.
        let start = self.toward_ink(SYNTAX_START);
        let coloured = |hue: f64| self.solved(code_surface, start, SYNTAX_CHROMA, hue, 4.5);

        // A comment has to recede from the code around it without dimming, since
        // it is held to the same 4.5:1 floor as everything else. Hue does the
        // work instead of lightness: a comment carries more of the *theme's own*
        // hue than the punctuation beside it, which reads as tinted rather than
        // faint — and, because it is the theme's hue, it differs from theme to
        // theme, which is the point of this rewrite.
        SyntaxPalette {
            comment: self.solved(
                code_surface,
                start,
                self.neutral_chroma().max(COMMENT_CHROMA),
                self.hue,
                4.5,
            ),
            string: coloured(hue::STRING),
            number: coloured(hue::NUMBER),
            keyword: coloured(hue::KEYWORD),
            type_name: coloured(hue::TYPE),
            // Punctuation is the theme's own neutral: it is structure, not a
            // category, and colouring it would be colouring every third glyph.
            punctuation: self.solved(code_surface, start, self.neutral_chroma(), self.hue, 4.5),
        }
    }
}

/// How far the letterboxing sits from the sheet.
const CANVAS_OFFSET: f64 = 0.075;
/// How far a code panel sits from the sheet.
const PANEL_OFFSET: f64 = 0.05;
/// How much stronger a heading is than body text.
const HEADING_OFFSET: f64 = 0.06;
/// Where a hairline sits between the sheet and the ink.
const BORDER_OFFSET: f64 = 0.16;
/// Where the solver starts looking for secondary text.
const MUTED_START: f64 = 0.22;
/// Where the solver starts looking for the accent.
const ACCENT_START: f64 = 0.30;
/// Where every syntax colour begins its search, measured from the sheet.
const SYNTAX_START: f64 = 0.30;
/// Least chroma a comment may carry.
///
/// A comment separates from code by hue rather than by lightness, because the
/// contrast floor leaves nothing to dim into. A theme with a zero wash still
/// needs enough colour here for that to work.
const COMMENT_CHROMA: f64 = 0.045;

#[cfg(test)]
mod tests {
    use super::*;

    fn light() -> Recipe {
        Recipe { hue: 258.0, accent_chroma: 0.154, wash: 0.1, sheet: 0.985, ink: 0.24 }
    }

    fn dark() -> Recipe {
        Recipe { hue: 258.0, accent_chroma: 0.13, wash: 0.1, sheet: 0.21, ink: 0.95 }
    }

    #[test]
    fn a_recipe_knows_which_way_its_own_contrast_runs() {
        assert!(!light().is_dark());
        assert!(dark().is_dark());
    }

    #[test]
    fn every_neutral_is_a_wash_of_the_accent() {
        // One hue per theme. A neutral with a hue of its own would make the
        // palette two families pretending to be one.
        let recipe = light();
        assert_eq!(recipe.neutral_chroma(), recipe.accent_chroma * recipe.wash);
    }

    #[test]
    fn a_zero_wash_produces_true_greys() {
        let flat = Recipe { wash: 0.0, ..light() };
        let grey = flat.neutral(0.5);

        assert_eq!((grey.r, grey.g), (grey.g, grey.b), "got {}", grey.to_hex());
    }

    #[test]
    fn the_canvas_is_darker_than_the_slide_in_both_schemes() {
        // A projected slide is the lit thing in the room. Letterboxing lighter
        // than the slide would read as a hole cut in a bright surface.
        for recipe in [light(), dark()] {
            let palette = recipe.palette();

            assert!(
                palette.canvas.relative_luminance() < palette.surface.relative_luminance(),
                "the canvas is not darker than the slide"
            );
        }
    }

    #[test]
    fn a_heading_is_the_body_ink_pushed_one_step_further_from_the_sheet() {
        for recipe in [light(), dark()] {
            let palette = recipe.palette();
            let text = palette.text.relative_luminance();
            let heading = palette.heading.relative_luminance();

            if recipe.is_dark() {
                assert!(heading >= text, "a dark heading must be at least as bright as its text");
            } else {
                assert!(heading <= text, "a light heading must be at least as dark as its text");
            }
            assert_ne!(palette.heading, palette.surface);
        }
    }

    #[test]
    fn a_code_panel_sits_towards_the_ink_from_the_sheet() {
        // Which is what makes it read as inset without a border saying so.
        for recipe in [light(), dark()] {
            let palette = recipe.palette();
            let sheet = palette.surface.relative_luminance();
            let panel = palette.code_surface.relative_luminance();

            if recipe.is_dark() {
                assert!(panel > sheet);
            } else {
                assert!(panel < sheet);
            }
        }
    }

    #[test]
    fn every_solved_role_clears_its_floor_in_the_room_it_was_solved_for() {
        for recipe in [light(), dark()] {
            let palette = recipe.palette();

            for (name, color, background, floor) in [
                ("text", palette.text, palette.surface, 4.5),
                ("muted", palette.muted, palette.surface, 3.0),
                ("accent", palette.accent, palette.surface, 4.5),
                ("codeText", palette.code_text, palette.code_surface, 4.5),
            ] {
                let ratio = projected_contrast_ratio(color, background, ROOM);
                assert!(ratio >= floor, "{name} is {ratio:.2}:1 against its own background");
            }
        }
    }

    #[test]
    fn every_syntax_role_clears_the_code_surface_it_sits_on() {
        for recipe in [light(), dark()] {
            let palette = recipe.palette();
            let syntax = palette.syntax();

            for token in slidx_highlight::Token::COLOURED {
                let ratio = projected_contrast_ratio(syntax.get(token), palette.code_surface, ROOM);
                assert!(ratio >= 4.5, "{} is {ratio:.2}:1", token.as_token());
            }
        }
    }

    #[test]
    fn no_two_syntax_roles_come_out_the_same_colour() {
        // Two roles one colour is highlighting that says less than it appears to.
        for recipe in [light(), dark()] {
            let syntax = recipe.palette().syntax();
            let mut used: Vec<String> = slidx_highlight::Token::COLOURED
                .iter()
                .map(|&token| syntax.get(token).to_hex())
                .collect();

            let total = used.len();
            used.sort();
            used.dedup();

            assert_eq!(used.len(), total, "a role repeats a colour");
        }
    }

    #[test]
    fn two_themes_with_different_hues_do_not_share_a_syntax_colour() {
        // The defect this module exists to fix: the old themes shipped identical
        // syntax hexes, so the theme system did not theme code at all.
        let one = Recipe { hue: 258.0, ..light() }.palette().syntax();
        let other = Recipe { hue: 65.0, accent_chroma: 0.11, ..light() }.palette().syntax();

        assert_ne!(one.comment, other.comment, "comments are not themed");
        assert_ne!(one.punctuation, other.punctuation, "punctuation is not themed");
    }

    #[test]
    fn a_comment_separates_from_code_by_hue_when_it_cannot_by_lightness() {
        // The floor leaves almost nothing to dim into, so a comment carries
        // colour instead. A comment with the neutral's chroma on a zero-wash
        // theme would be the same colour as the punctuation beside it.
        let flat = Recipe { wash: 0.0, ..light() };
        let syntax = flat.palette().syntax();

        assert_ne!(syntax.comment, syntax.punctuation);
    }

    #[test]
    fn a_recipe_is_deterministic() {
        assert_eq!(light().palette(), light().palette());
    }
}
