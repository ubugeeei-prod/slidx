//! How a room is written down.
//!
//! A caption strip is not a design decision and not a property of the deck: it
//! belongs to one room on one day. So it is neither a theme token — a theme
//! travels between venues, and forking one per conference is absurd — nor a
//! flag on the build, which nobody would remember to pass twice.
//!
//! It is deck frontmatter, next to `aspect:`, because it is the same kind of
//! fact: it changes the shape of the box the author has to work in. Writing it
//! there also puts it in the diff, where a reviewer can see that this deck was
//! authored for a room that eats the bottom of every slide.
//!
//! ```yaml
//! ---
//! title: Making Decks Fast
//! safeArea:
//!   bottom: 15%
//! ---
//! ```
//!
//! A single value applies to all four edges, which is what an overscanning
//! projector does:
//!
//! ```yaml
//! safeArea: 3%
//! ```
//!
//! Every value carries its unit. `15%` is a share of the slide, `120px` is
//! measured against the deck's own canvas, and a bare `15` is refused rather
//! than guessed at — the two readings differ by an order of magnitude, and the
//! wrong one would be silently wrong.

use serde_json::Value as JsonValue;

use crate::geometry::{Insets, Side};
use crate::surface::RenderTarget;

/// The frontmatter key a room is declared under.
pub const KEY: &str = "safeArea";

/// A declaration, and anything in it that could not be read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Declaration {
    pub insets: Option<Insets>,
    /// Values that were written and not understood, as `key: value`.
    ///
    /// Collected rather than skipped: a typo in a safe-area declaration
    /// produces silence in exactly the place the author believed they had
    /// asked for a check, which is the worst outcome available.
    pub unreadable: Vec<String>,
}

/// Reads a `safeArea:` declaration out of deck frontmatter.
pub fn read(raw: &JsonValue, target: RenderTarget) -> Declaration {
    let Some(value) = raw.get(KEY).or_else(|| raw.get("safe-area")) else {
        return Declaration::default();
    };

    match value {
        JsonValue::String(text) => match share(text, target.height_px) {
            Some(found) => Declaration { insets: Some(Insets::uniform(found)), unreadable: vec![] },
            None => unreadable(KEY, text),
        },
        JsonValue::Object(_) => sides(value, target),
        other => unreadable(KEY, &render(other)),
    }
}

/// Reads the per-side form, keeping every side that parsed.
///
/// One bad side does not discard the others: a deck that declared a bottom
/// strip and mistyped its left crop should still be checked against the strip.
fn sides(value: &JsonValue, target: RenderTarget) -> Declaration {
    let mut insets = Insets::NONE;
    let mut unreadable = Vec::new();
    let mut found_any = false;

    for side in Side::ALL {
        let Some(written) = value.get(side.as_token()) else { continue };

        match written.as_str().and_then(|text| share(text, side.extent_px(target))) {
            Some(parsed) => {
                insets = insets.with_side(side, parsed);
                found_any = true;
            }
            None => unreadable.push(format!("{}: {}", side.as_token(), render(written))),
        }
    }

    Declaration { insets: found_any.then_some(insets), unreadable }
}

/// Parses one length into a share of `extent_px`.
pub fn share(text: &str, extent_px: f64) -> Option<f64> {
    let text = text.trim();

    let parsed = if let Some(number) = text.strip_suffix('%') {
        number.trim().parse::<f64>().ok()? / 100.0
    } else {
        // Anything that is not a percentage has to be pixels, and pixels only
        // mean something against an extent that exists to divide by.
        let number = text.strip_suffix("px")?;
        if extent_px <= 0.0 {
            return None;
        }
        number.trim().parse::<f64>().ok()? / extent_px
    };

    // A negative band is not a smaller one, and a band larger than the slide
    // means the declaration says something other than what its author meant.
    (0.0..=1.0).contains(&parsed).then_some(parsed)
}

fn unreadable(key: &str, written: &str) -> Declaration {
    Declaration { insets: None, unreadable: vec![format!("{key}: {written}")] }
}

/// A value as it would read back to the author, for quoting in a diagnostic.
fn render(value: &JsonValue) -> String {
    value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    const TARGET: RenderTarget = RenderTarget { width_px: 1920.0, height_px: 1080.0 };

    fn parse(declaration: JsonValue) -> Declaration {
        read(&json!({ "title": "One", "safeArea": declaration }), TARGET)
    }

    #[test]
    fn a_deck_that_declares_nothing_declares_nothing() {
        assert_eq!(read(&json!({ "title": "One" }), TARGET), Declaration::default());
    }

    #[test]
    fn a_venue_that_eats_the_bottom_fifteen_percent_is_declared_on_one_line() {
        let found = parse(json!({ "bottom": "15%" }));

        assert_eq!(found.insets.unwrap().bottom, 0.15);
        assert_eq!(found.insets.unwrap().top, 0.0);
    }

    #[test]
    fn a_single_value_applies_to_every_edge() {
        assert_eq!(parse(json!("3%")).insets.unwrap(), Insets::uniform(0.03));
    }

    #[test]
    fn a_kebab_case_key_reads_the_same_as_a_camel_case_one() {
        // Authors write both, and rejecting one is a papercut that reads as a
        // bug in the deck.
        let kebab = read(&json!({ "safe-area": { "bottom": "10%" } }), TARGET);
        assert_eq!(kebab, parse(json!({ "bottom": "10%" })));
    }

    #[test]
    fn a_pixel_value_is_measured_against_the_axis_it_is_on() {
        let insets = parse(json!({ "bottom": "108px", "left": "192px" })).insets.unwrap();

        assert!((insets.bottom - 0.1).abs() < 0.001);
        assert!((insets.left - 0.1).abs() < 0.001);
    }

    #[test]
    fn a_value_with_no_unit_is_refused_rather_than_guessed_at() {
        // `15` could be fifteen percent or fifteen pixels, and the readings
        // differ by an order of magnitude.
        let found = parse(json!({ "bottom": 15 }));

        assert!(found.insets.is_none());
        assert_eq!(found.unreadable, vec!["bottom: 15".to_string()]);
    }

    #[test]
    fn one_bad_side_does_not_discard_the_others() {
        let found = parse(json!({ "bottom": "15%", "left": "wide" }));

        assert_eq!(found.insets.unwrap().bottom, 0.15);
        assert_eq!(found.unreadable, vec!["left: wide".to_string()]);
    }

    #[test]
    fn a_declaration_that_is_not_a_mapping_is_reported_not_ignored() {
        let found = parse(json!(["15%"]));

        assert!(found.insets.is_none());
        assert!(found.unreadable[0].starts_with("safeArea:"), "got {:?}", found.unreadable);
    }

    #[test]
    fn a_band_larger_than_the_slide_is_refused() {
        assert!(parse(json!({ "bottom": "140%" })).insets.is_none());
        assert!(parse(json!({ "bottom": "-5%" })).insets.is_none());
    }

    #[test]
    fn unknown_keys_inside_the_declaration_are_left_alone() {
        // Frontmatter carries theme and plugin options; a key this rule has
        // never heard of belongs to somebody else.
        let found = parse(json!({ "bottom": "5%", "note": "the projector overscans" }));

        assert_eq!(found.insets.unwrap().bottom, 0.05);
        assert!(found.unreadable.is_empty());
    }

    #[test]
    fn whitespace_around_a_value_does_not_change_it() {
        assert_eq!(share("  15%  ", 1080.0), Some(0.15));
        assert_eq!(share("108 px", 1080.0), Some(0.1));
    }

    #[test]
    fn a_zero_extent_cannot_turn_a_pixel_value_into_a_share() {
        assert_eq!(share("100px", 0.0), None);
    }
}
