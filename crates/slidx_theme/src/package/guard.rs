//! Holding a published theme to what the shell can defend.
//!
//! A theme document arrives from a registry and its values are written into a
//! page. Two things follow, and neither is negotiable.
//!
//! **Nothing a theme says may leave the declaration it was written into.**
//! Colours and sizes cross as numbers and cannot carry syntax, but the font
//! stacks are strings interpolated straight into `--slidx-font-sans:`. A stack
//! holding `; } </style><script>` closes the declaration, closes the rule,
//! closes the element, and is running script on the slide. So a stack is
//! checked against a grammar rather than escaped: escaping asks what the
//! character means to a parser, and a font family has no legitimate use for any
//! of the characters that matter.
//!
//! The same grammar is what keeps the offline guarantee. `url(` and `http:`
//! both need characters a family name never contains, so a theme cannot reach
//! for a webfont — the rule `slidx_render` cannot see, because by then the
//! stack is a string in a stylesheet.
//!
//! **Nothing a theme says may break the two things `slidx_render::layout`
//! owns.** The slide scales as one piece, and the safe area is real padding.
//! Both are enforced in the shell stylesheet, which a theme cannot touch — but
//! the shell reads `var(--slidx-space-padding)` for the safe area and the type
//! scale for every size in it, and those are the theme's numbers. A padding of
//! zero is a safe area of zero, written in a file the shell has no say over.
//! So every number is clamped into the range where those guarantees still hold,
//! and the clamp is reported rather than silent.
//!
//! Nothing here is about legibility. A theme that clears every bound in this
//! module can still ship text nobody at the back can read, and that is
//! [`crate::audit`]'s question, asked of a package for exactly the same reason
//! it is asked of a built-in.

use slidx_lint::Rgba;

use crate::palette::{Palette, SyntaxPalette};
use crate::theme::{Motion, Spacing, Theme};
use crate::typography::Typography;

/// One value the guard would not pass through.
#[derive(Debug, Clone, PartialEq)]
pub struct Repair {
    /// The field as the theme document spells it.
    pub field: String,
    /// What the document asked for.
    pub asked: String,
    /// What the deck got instead.
    pub given: String,
    pub reason: Reason,
}

/// Why a value did not survive the guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// It could have escaped the declaration it was written into.
    Unsafe,
    /// It is outside the range in which the shell's guarantees hold.
    OutOfRange,
}

/// The narrowest padding that is still a safe area.
///
/// Venue projection crops the edges — commonly two to five percent of each
/// side — and the shell enforces the theme's padding as the margin that
/// absorbs it. A theme setting zero is therefore not a design choice about
/// density; it is a deck whose first and last line are cut off by the room,
/// with nothing left for the shell to defend. Three percent of the reference
/// canvas is the floor.
const MIN_PADDING_PX: f64 = 32.0;

/// The widest padding that still leaves a slide to draw on.
///
/// A fifth of the canvas per edge already spends two fifths of the height.
const MAX_PADDING_PX: f64 = 216.0;

/// The smallest body size a theme may quote.
///
/// Deliberately far below anything legible: this is not the legibility floor,
/// which is [`crate::audit`]'s and depends on the room. All that is being
/// prevented here is a zero or negative length, which is not a very small theme
/// but an invalid declaration the browser drops.
const MIN_BASE_PX: f64 = 1.0;

/// The largest body size a theme may quote.
///
/// A heading is `base × ratio³`, so at the widest ratio the guard allows this
/// puts the largest heading at 768px on a 1080 canvas — big, and still inside
/// the frame. Above it a single heading is taller than the slide it is drawn
/// in, and the safe area stops being one.
const MAX_BASE_PX: f64 = 96.0;

/// The scale ratio must at least not invert.
///
/// Below 1.0 a heading is smaller than the body text under it, which is not a
/// dramatic scale but a broken one — and the linter's heading-order rule reads
/// the markup, so it would never see it.
const MIN_RATIO: f64 = 1.0;

/// Three steps of 2.0 is eight times the body size, which is the most a
/// heading can be and still share a slide with anything.
const MAX_RATIO: f64 = 2.0;

/// Code below half body size is unreadable and above twice it is a fence that
/// owns the slide. The audit decides everything between.
const MIN_CODE_FACTOR: f64 = 0.5;
const MAX_CODE_FACTOR: f64 = 2.0;

/// The rhythm between blocks, capped where one gap is a tenth of the slide.
const MAX_BLOCK_PX: f64 = 120.0;

/// The two lengths that do not scale with the slide.
///
/// Every other size is emitted in `cqh` and is a share of the slide. These
/// two are absolute pixels, so a theme is the one place a value can be written
/// that stays the same size while the slide shrinks — which is precisely the
/// guarantee the shell owns. Small ceilings, for that reason and not for taste.
const MAX_RADIUS_PX: f64 = 16.0;
const MAX_HAIRLINE_PX: f64 = 8.0;

/// How type is set, bounded.
///
/// [`Typography::leading`](crate::Typography::leading) already clamps what it
/// emits, so leading is safe whatever a document says. Tracking and measure are
/// not: a response of five puts a tenth of an em between every letter, and a
/// measure of two hundred is no measure at all. Both reach a stylesheet
/// directly, so both are held here rather than at the point of use.
///
/// The leading pair is bounded anyway, because a value that survives into the
/// editor's theme panel should be a number somebody can read.
const MIN_BASE_LEADING: f64 = 1.0;
const MAX_BASE_LEADING: f64 = 2.5;
const MAX_LEADING_RESPONSE: f64 = 2.0;
const MAX_TRACKING_RESPONSE: f64 = 0.1;
const MIN_MEASURE_EM: f64 = 4.0;
const MAX_MEASURE_EM: f64 = 120.0;

/// Past roughly this an audience watches the transition instead of the slide.
///
/// The same number the built-in themes are held to, applied to a package for
/// the same reason: a presenter clicking through ten slides waits for every one
/// of them.
const MAX_TRANSITION_MS: u32 = 400;

/// Longest a name or a description may be.
///
/// Both are shown in a list beside three other themes. A theme that arrives
/// with a paragraph in its name is not describing itself.
const MAX_NAME_CHARS: usize = 64;
const MAX_DESCRIPTION_CHARS: usize = 200;

/// Longest a font stack may be.
///
/// Ten families is already more fallback than any deck resolves.
const MAX_STACK_CHARS: usize = 200;

/// A theme document, made safe to render.
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    pub theme: Theme,
    /// Everything the guard changed, each named. Empty is the normal case.
    pub repairs: Vec<Repair>,
}

/// Brings a parsed theme inside the bounds the shell can defend.
///
/// `fallback` supplies the replacement for a string that cannot be made safe.
/// It is the default theme rather than an empty value, because a slide drawn
/// with no font family at all is a slide in whatever the browser picked, which
/// is the one outcome nobody chose.
pub fn hold(theme: Theme, fallback: &Theme) -> Held {
    let mut repairs = Vec::new();

    let theme = Theme {
        id: theme.id,
        name: text("name", theme.name, MAX_NAME_CHARS, &mut repairs),
        description: text("description", theme.description, MAX_DESCRIPTION_CHARS, &mut repairs),
        light: palette(theme.light),
        dark: palette(theme.dark),
        scale: crate::TypeScale {
            base_px: bounded(
                "scale.basePx",
                theme.scale.base_px,
                MIN_BASE_PX,
                MAX_BASE_PX,
                &mut repairs,
            ),
            ratio: bounded("scale.ratio", theme.scale.ratio, MIN_RATIO, MAX_RATIO, &mut repairs),
            code_factor: bounded(
                "scale.codeFactor",
                theme.scale.code_factor,
                MIN_CODE_FACTOR,
                MAX_CODE_FACTOR,
                &mut repairs,
            ),
        },
        spacing: Spacing {
            padding_px: bounded(
                "spacing.paddingPx",
                theme.spacing.padding_px,
                MIN_PADDING_PX,
                MAX_PADDING_PX,
                &mut repairs,
            ),
            block_px: bounded(
                "spacing.blockPx",
                theme.spacing.block_px,
                0.0,
                MAX_BLOCK_PX,
                &mut repairs,
            ),
            radius_px: bounded(
                "spacing.radiusPx",
                theme.spacing.radius_px,
                0.0,
                MAX_RADIUS_PX,
                &mut repairs,
            ),
            hairline_px: bounded(
                "spacing.hairlinePx",
                theme.spacing.hairline_px,
                0.0,
                MAX_HAIRLINE_PX,
                &mut repairs,
            ),
        },
        motion: Motion {
            transition_ms: milliseconds(theme.motion.transition_ms, &mut repairs),
            transition_easing: theme.motion.transition_easing,
        },
        typography: Typography {
            base_leading: bounded(
                "typography.baseLeading",
                theme.typography.base_leading,
                MIN_BASE_LEADING,
                MAX_BASE_LEADING,
                &mut repairs,
            ),
            leading_response: bounded(
                "typography.leadingResponse",
                theme.typography.leading_response,
                0.0,
                MAX_LEADING_RESPONSE,
                &mut repairs,
            ),
            tracking_response: bounded(
                "typography.trackingResponse",
                theme.typography.tracking_response,
                0.0,
                MAX_TRACKING_RESPONSE,
                &mut repairs,
            ),
            prose_measure_em: bounded(
                "typography.proseMeasureEm",
                theme.typography.prose_measure_em,
                MIN_MEASURE_EM,
                MAX_MEASURE_EM,
                &mut repairs,
            ),
            heading_measure_em: bounded(
                "typography.headingMeasureEm",
                theme.typography.heading_measure_em,
                MIN_MEASURE_EM,
                MAX_MEASURE_EM,
                &mut repairs,
            ),
        },
        font_sans: stack("fontSans", theme.font_sans, &fallback.font_sans, &mut repairs),
        font_mono: stack("fontMono", theme.font_mono, &fallback.font_mono, &mut repairs),
    };

    Held { theme, repairs }
}

/// True when a font stack may be written into a CSS declaration unchanged.
///
/// An allowlist, because the question "which characters are dangerous here"
/// has to be answered for a CSS value parser, an HTML tokeniser and whatever
/// reads the stylesheet next. The question "which characters does a font family
/// name contain" has one answer, and it excludes every character any of them
/// treats as syntax.
fn is_family_text(value: &str) -> bool {
    value.chars().all(|c| {
        c == ' '
            || c == ','
            || c == '-'
            || c == '_'
            || c == '\''
            || c == '"'
            || c.is_ascii_alphanumeric()
            || (!c.is_ascii() && !c.is_control())
    })
}

/// Every family in a stack is either bare or wholly quoted.
///
/// A stack with one unbalanced quote is a stack whose remainder the CSS parser
/// reads as part of a string, which is how a value that looks like a list of
/// families becomes one long family name and then something else entirely.
fn quotes_balance(stack: &str) -> bool {
    stack.split(',').all(|family| {
        let family = family.trim();
        let quotes = family.chars().filter(|c| *c == '\'' || *c == '"').count();

        match quotes {
            0 => true,
            2 => {
                let first = family.chars().next();
                first == family.chars().last() && matches!(first, Some('\'') | Some('"'))
            }
            _ => false,
        }
    })
}

fn stack(field: &str, value: String, fallback: &str, repairs: &mut Vec<Repair>) -> String {
    let trimmed = value.trim();

    if !trimmed.is_empty()
        && trimmed.chars().count() <= MAX_STACK_CHARS
        && is_family_text(trimmed)
        && quotes_balance(trimmed)
    {
        return trimmed.to_string();
    }

    repairs.push(Repair {
        field: field.to_string(),
        asked: value,
        given: fallback.to_string(),
        reason: Reason::Unsafe,
    });

    fallback.to_string()
}

/// Text a person reads, with control characters removed and a length cap.
fn text(field: &str, value: String, limit: usize, repairs: &mut Vec<Repair>) -> String {
    let cleaned: String = value.chars().filter(|c| !c.is_control()).collect();
    let capped: String = cleaned.chars().take(limit).collect();

    if capped != value {
        repairs.push(Repair {
            field: field.to_string(),
            asked: value,
            given: capped.clone(),
            reason: Reason::Unsafe,
        });
    }

    capped
}

/// A number brought inside `[low, high]`, reporting when it had to move.
///
/// Non-finite goes to `low` rather than through `clamp`, which returns NaN for
/// NaN and would put it straight into a stylesheet.
fn bounded(field: &str, value: f64, low: f64, high: f64, repairs: &mut Vec<Repair>) -> f64 {
    let held = if value.is_finite() { value.clamp(low, high) } else { low };

    if held != value {
        repairs.push(Repair {
            field: field.to_string(),
            asked: format!("{value}"),
            given: format!("{held}"),
            reason: Reason::OutOfRange,
        });
    }

    held
}

fn milliseconds(value: u32, repairs: &mut Vec<Repair>) -> u32 {
    let held = value.min(MAX_TRANSITION_MS);

    if held != value {
        repairs.push(Repair {
            field: "motion.transitionMs".to_string(),
            asked: value.to_string(),
            given: held.to_string(),
            reason: Reason::OutOfRange,
        });
    }

    held
}

/// Alpha brought into range on every colour in a palette.
///
/// The only number in a palette that is not a `u8`. A NaN there survives every
/// comparison in [`Rgba::over`] and composites to black, which would give the
/// audit a colour the slide never shows.
fn palette(palette: Palette) -> Palette {
    let syntax = palette.syntax.map(|syntax| SyntaxPalette {
        comment: opaque_enough(syntax.comment),
        string: opaque_enough(syntax.string),
        number: opaque_enough(syntax.number),
        keyword: opaque_enough(syntax.keyword),
        type_name: opaque_enough(syntax.type_name),
        punctuation: opaque_enough(syntax.punctuation),
    });

    Palette {
        canvas: opaque_enough(palette.canvas),
        surface: opaque_enough(palette.surface),
        text: opaque_enough(palette.text),
        muted: opaque_enough(palette.muted),
        heading: opaque_enough(palette.heading),
        accent: opaque_enough(palette.accent),
        border: opaque_enough(palette.border),
        code_surface: opaque_enough(palette.code_surface),
        code_text: opaque_enough(palette.code_text),
        syntax,
    }
}

fn opaque_enough(color: Rgba) -> Rgba {
    Rgba { a: if color.a.is_finite() { color.a.clamp(0.0, 1.0) } else { 1.0 }, ..color }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;

    fn held(theme: Theme) -> Held {
        hold(theme, &builtin::minimal())
    }

    #[test]
    fn a_theme_that_asks_for_nothing_unusual_is_passed_through_untouched() {
        let held = held(builtin::editorial());

        assert_eq!(held.repairs, vec![]);
        assert_eq!(held.theme, builtin::editorial());
    }

    #[test]
    fn a_font_stack_that_closes_its_own_declaration_does_not_reach_the_page() {
        // The whole reason this module exists. The stack is written straight
        // into `--slidx-font-sans:`, so a semicolon ends the declaration and
        // the next rule is the theme's to write.
        let mut theme = builtin::minimal();
        theme.font_sans = "sans-serif; } :root { --slidx-color-text: red".into();

        let held = held(theme);

        assert_eq!(held.theme.font_sans, builtin::minimal().font_sans);
        assert_eq!(held.repairs[0].field, "fontSans");
        assert_eq!(held.repairs[0].reason, Reason::Unsafe);
    }

    #[test]
    fn a_font_stack_that_closes_the_style_element_does_not_reach_the_page() {
        // The attack that needs no CSS syntax at all: `<style>` is a raw-text
        // element, so a browser ends it at the first `</style>` whatever the
        // stylesheet parser makes of the characters around it.
        let mut theme = builtin::minimal();
        theme.font_sans = "sans-serif</style><script>alert(1)</script>".into();

        assert_eq!(held(theme).theme.font_sans, builtin::minimal().font_sans);
    }

    #[test]
    fn a_theme_cannot_smuggle_in_a_webfont() {
        // The offline guarantee, held where the linter cannot see: by the time
        // a stack is in a stylesheet it is no longer a deck's asset reference.
        for stack in [
            "url(https://fonts.example/Face.woff2)",
            "local('Face'), url(//cdn.example/f.woff2)",
            "Face, sans-serif; @import url(https://cdn.example/f.css)",
        ] {
            let mut theme = builtin::minimal();
            theme.font_mono = stack.into();

            assert_eq!(held(theme).theme.font_mono, builtin::minimal().font_mono, "{stack}");
        }
    }

    #[test]
    fn an_unbalanced_quote_makes_the_rest_of_the_stack_a_string() {
        let mut theme = builtin::minimal();
        theme.font_sans = "'Face, sans-serif".into();

        assert_eq!(held(theme).repairs[0].reason, Reason::Unsafe);
    }

    #[test]
    fn a_quoted_family_with_a_space_in_its_name_is_ordinary_and_survives() {
        let mut theme = builtin::minimal();
        theme.font_sans = "'Segoe UI', \"Noto Sans JP\", Helvetica-Neue, sans-serif".into();

        let held = held(theme);

        assert_eq!(held.repairs, vec![]);
        assert!(held.theme.font_sans.contains("Segoe UI"));
    }

    #[test]
    fn a_family_named_in_its_own_script_is_not_an_attack() {
        // A Japanese theme naming a Japanese face is the case a byte-level
        // allowlist gets wrong, and getting it wrong means the guard is a
        // reason not to publish a theme outside English.
        let mut theme = builtin::minimal();
        theme.font_sans = "ヒラギノ角ゴ ProN, 游ゴシック, sans-serif".into();

        assert_eq!(held(theme).repairs, vec![]);
    }

    #[test]
    fn an_empty_font_stack_falls_back_rather_than_emitting_an_empty_property() {
        let mut theme = builtin::minimal();
        theme.font_mono = "   ".into();

        assert_eq!(held(theme).theme.font_mono, builtin::minimal().font_mono);
    }

    #[test]
    fn a_theme_cannot_set_the_safe_area_to_nothing() {
        // The shell enforces the theme's padding as the safe area, so zero is
        // not density — it is a deck the room crops.
        let mut theme = builtin::minimal();
        theme.spacing.padding_px = 0.0;

        let held = held(theme);

        assert_eq!(held.theme.spacing.padding_px, MIN_PADDING_PX);
        assert_eq!(held.repairs[0].field, "spacing.paddingPx");
        assert_eq!(held.repairs[0].reason, Reason::OutOfRange);
    }

    #[test]
    fn a_theme_cannot_pad_a_slide_out_of_existence() {
        let mut theme = builtin::minimal();
        theme.spacing.padding_px = 900.0;

        assert_eq!(held(theme).theme.spacing.padding_px, MAX_PADDING_PX);
    }

    #[test]
    fn a_heading_cannot_be_asked_to_be_taller_than_the_slide() {
        // `heading_px(1)` is `base × ratio³`. Left alone, a base of 4000 draws
        // one glyph over the whole frame and every rule about the safe area
        // becomes decorative.
        let mut theme = builtin::minimal();
        theme.scale.base_px = 4000.0;
        theme.scale.ratio = 9.0;

        let scale = held(theme).theme.scale;

        assert!(scale.heading_px(1) <= crate::REFERENCE_HEIGHT_PX, "{}", scale.heading_px(1));
    }

    #[test]
    fn a_scale_that_makes_a_heading_smaller_than_body_text_is_refused() {
        let mut theme = builtin::minimal();
        theme.scale.ratio = 0.5;

        let scale = held(theme).theme.scale;

        assert!(scale.heading_px(1) >= scale.body_px());
    }

    #[test]
    fn the_two_lengths_that_do_not_scale_with_the_slide_are_capped() {
        // Everything else is `cqh` and shrinks with the frame. A 400px hairline
        // is 400px on a phone and on a hall projector alike.
        let mut theme = builtin::minimal();
        theme.spacing.radius_px = 400.0;
        theme.spacing.hairline_px = 400.0;

        let spacing = held(theme).theme.spacing;

        assert_eq!(spacing.radius_px, MAX_RADIUS_PX);
        assert_eq!(spacing.hairline_px, MAX_HAIRLINE_PX);
    }

    #[test]
    fn a_theme_keeps_the_way_it_sets_type() {
        // The guard rebuilds a theme field by field, which is how a field added
        // to `Theme` quietly stops arriving. `editorial` is the one built-in
        // that moves its leading, so it is the fixture that would notice.
        let held = held(builtin::editorial());

        assert_eq!(held.theme.typography, builtin::editorial().typography);
        assert_ne!(held.theme.typography, Typography::default(), "the fixture must differ");
    }

    #[test]
    fn tracking_and_measure_a_document_cannot_be_talked_out_of_are_capped() {
        // Leading is clamped where it is emitted; these two are written into a
        // stylesheet as given. A response of five is a tenth of an em between
        // every letter, and a two-hundred-em measure is no measure at all.
        let mut theme = builtin::minimal();
        theme.typography.tracking_response = 5.0;
        theme.typography.prose_measure_em = 200.0;
        theme.typography.heading_measure_em = 0.0;

        let typography = held(theme).theme.typography;

        assert_eq!(typography.tracking_response, MAX_TRACKING_RESPONSE);
        assert_eq!(typography.prose_measure_em, MAX_MEASURE_EM);
        assert_eq!(typography.heading_measure_em, MIN_MEASURE_EM);
    }

    #[test]
    fn a_transition_long_enough_to_watch_is_shortened() {
        let mut theme = builtin::minimal();
        theme.motion.transition_ms = 5_000;

        assert_eq!(held(theme).theme.motion.transition_ms, MAX_TRANSITION_MS);
    }

    #[test]
    fn a_size_that_is_not_a_number_never_reaches_a_stylesheet() {
        // `f64::clamp` returns NaN for NaN, so the obvious implementation puts
        // `--slidx-size-body: NaNcqh` into the page.
        for broken in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut theme = builtin::minimal();
            theme.scale.base_px = broken;

            assert!(held(theme).theme.scale.base_px.is_finite(), "{broken}");
        }
    }

    #[test]
    fn a_control_character_in_a_name_is_removed() {
        let mut theme = builtin::minimal();
        theme.name = "Aur\u{0}ora\n".into();

        let held = held(theme);

        assert_eq!(held.theme.name, "Aurora");
        assert_eq!(held.repairs[0].field, "name");
    }

    #[test]
    fn a_description_the_length_of_a_page_is_cut_to_a_line() {
        let mut theme = builtin::minimal();
        theme.description = "x".repeat(10_000);

        assert_eq!(held(theme).theme.description.chars().count(), MAX_DESCRIPTION_CHARS);
    }

    #[test]
    fn a_translucent_colour_with_no_number_in_it_composites_to_something_real() {
        let mut theme = builtin::minimal();
        theme.light.text = Rgba { a: f64::NAN, ..theme.light.text };

        assert_eq!(held(theme).theme.light.text.a, 1.0);
    }

    #[test]
    fn every_repair_names_what_was_asked_for_and_what_was_given() {
        // A report that says only "the theme was adjusted" is one nobody can
        // act on, and the person who has to act is the theme's author.
        let mut theme = builtin::minimal();
        theme.spacing.padding_px = -1.0;
        theme.font_sans = "a; b".into();

        for repair in held(theme).repairs {
            assert!(!repair.field.is_empty());
            assert!(!repair.asked.is_empty(), "{}", repair.field);
            assert!(!repair.given.is_empty(), "{}", repair.field);
        }
    }

    #[test]
    fn holding_a_theme_twice_changes_nothing_the_second_time() {
        // The guard has to be a fixed point, or a theme cached after one pass
        // and reloaded would keep drifting.
        let mut theme = builtin::terminal();
        theme.spacing.padding_px = 0.0;
        theme.font_sans = "</style>".into();

        let once = held(theme).theme;
        let twice = held(once.clone());

        assert_eq!(twice.repairs, vec![]);
        assert_eq!(twice.theme, once);
    }
}
