//! The speaker's own camera, as the deck asks for it.
//!
//! A talk given remotely, or recorded, wants the speaker's face on the slide.
//! The mechanism is a webcam, and a webcam is the one thing on this list that a
//! published deck must never reach for on its own: a deck is a static page
//! somebody opens from a link, a QR code, or an archive years later, and a page
//! that asks for a camera is a page people close.
//!
//! So the declaration and the stream are two different things, and only the
//! first of them is in the file. What an author writes here says *where* on the
//! slide a camera belongs. Nothing in this crate, and nothing the build emits
//! from it, opens a device — that needs a second opt-in, from the speaker, at
//! presentation time, and it lives in the runtime.
//!
//! # Why a region and not a corner
//!
//! `camera: side` names a region of the slide's layout, for the same reason a
//! block does: four floats would mean a different thing at a different aspect
//! ratio, no reviewer could read them, and no rule could measure them. A region
//! is a share of the slide at every projector size, and it is what lets a talk
//! put the speaker large on the opening slide and small over a diagram without
//! anyone typing a pixel.
//!
//! There is deliberately no `camera: true`. A camera with no region would have
//! to default to the region the slide's content is already in, which is the one
//! placement an author never means.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::diagnostic::{Diagnostic, Diagnostics};

/// Attribute naming the region a camera tile occupies.
///
/// The placement, and nothing about a device. It is written by the build into a
/// tile that starts empty, so a reader who opens the deck from a link gets an
/// element with no stream in it and no code that could ask for one.
pub const CAMERA_ATTRIBUTE: &str = "data-slidx-camera";

/// Attribute carrying what the camera is currently doing.
///
/// Written by the runtime and never by the build, which is what makes the
/// starting state — `idle`, drawn as nothing at all — the state every published
/// page is stuck in.
pub const CAMERA_STATE_ATTRIBUTE: &str = "data-slidx-camera-state";

/// A camera, as the deck declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Camera {
    /// The layout region the tile occupies, as the author wrote it.
    ///
    /// Kept as written rather than resolved: which regions exist belongs to a
    /// theme, and this crate has never heard of one. A name no layout has is
    /// reported by `slidx_theme::layout`, exactly as a misplaced block is.
    pub region: String,
}

/// Reads a `camera:` declaration.
///
/// Two levels of [`Option`], and both are load-bearing. The outer one says
/// whether the block mentioned `camera` at all, and the inner one says whether
/// it asked for one — without that distinction `camera: false` on a slide could
/// not switch off a deck-wide default, because a slide turning the camera off
/// would look exactly like a slide that never mentioned it.
pub fn parse(matter: &JsonValue, diagnostics: &mut Diagnostics) -> Option<Option<Camera>> {
    let field = crate::frontmatter::field(matter, "camera")?;

    // `camera: false` is how YAML reads the switch an author reaches for, and it
    // has to keep meaning "off" rather than falling through to the report below
    // — a slide that lost it would inherit the very camera it was turning off.
    if field == &JsonValue::Bool(false) {
        return Some(None);
    }

    match field.as_str().map(str::trim) {
        Some("none" | "off") => Some(None),
        Some(region) if !region.is_empty() => Some(Some(Camera { region: region.to_string() })),
        // Anything else — `camera: true`, a number, an empty string. Reported
        // rather than dropped: an author looking at a slide with no camera on it
        // needs to be told which line is the reason.
        _ => {
            diagnostics.push(
                Diagnostic::warning("frontmatter/invalid-camera", "`camera` must name a region")
                    .with_help("write `camera: side`, or `camera: false` to switch one off"),
            );

            Some(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read(value: &JsonValue) -> (Option<Option<Camera>>, Diagnostics) {
        let mut diagnostics = Diagnostics::default();
        let camera = parse(value, &mut diagnostics);
        (camera, diagnostics)
    }

    #[test]
    fn a_camera_names_the_region_of_the_slide_it_occupies() {
        let (camera, diagnostics) = read(&json!({ "camera": "side" }));

        assert_eq!(camera, Some(Some(Camera { region: "side".into() })));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn a_slide_that_says_nothing_about_a_camera_is_left_free_to_inherit_one() {
        // The outer `None`. A slide with no opinion takes the deck's, which is
        // how a remote talk declares the camera once instead of forty times.
        assert_eq!(read(&json!({ "title": "Not a camera" })).0, None);
    }

    #[test]
    fn a_slide_can_switch_off_a_deck_wide_camera() {
        // The one slide that is a full-bleed diagram, in a talk that otherwise
        // keeps the speaker on screen.
        for value in [json!({ "camera": false }), json!({ "camera": "none" })] {
            let (camera, diagnostics) = read(&value);

            assert_eq!(camera, Some(None), "{value} did not switch the camera off");
            assert!(diagnostics.is_empty());
        }
    }

    #[test]
    fn a_camera_with_no_region_is_reported_rather_than_placed_over_the_slide() {
        // `camera: true` has only one region left to fall back to, and it is
        // the one the slide's own content is in.
        let (camera, diagnostics) = read(&json!({ "camera": true }));

        assert_eq!(camera, Some(None));
        assert_eq!(diagnostics.as_slice()[0].code, "frontmatter/invalid-camera");
        assert!(!diagnostics.has_blocking(), "the deck still renders");
    }

    #[test]
    fn a_camera_that_is_not_a_name_is_reported() {
        let (camera, diagnostics) = read(&json!({ "camera": 3 }));

        assert_eq!(camera, Some(None));
        assert_eq!(diagnostics.as_slice()[0].code, "frontmatter/invalid-camera");
    }

    #[test]
    fn the_region_name_is_taken_as_written_rather_than_resolved_here() {
        // Which regions exist belongs to a theme. A name this crate filtered out
        // would be a typo nothing downstream could report.
        let (camera, diagnostics) = read(&json!({ "camera": "  nowhere  " }));

        assert_eq!(camera, Some(Some(Camera { region: "nowhere".into() })));
        assert!(diagnostics.is_empty());
    }
}
