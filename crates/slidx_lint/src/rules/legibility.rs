//! Font-size rules.
//!
//! Sizes are judged by angular size at the back row rather than by a pixel
//! threshold, so the rule is portable across canvas sizes and rooms. See
//! [`crate::typography`] for the model and its calibration.

use slidx_core::{Diagnostic, Diagnostics, Severity, SourceSpan};

use crate::surface::{Surface, TextSample};
use crate::typography::{classify, min_font_px, Legibility};
use crate::{LintInput, LintOptions};

pub fn check(input: &LintInput<'_>, options: &LintOptions, sink: &mut Diagnostics) {
    let canvas = input.target.height_px;

    for surface in input.surfaces {
        for sample in &surface.text {
            let verdict = classify(sample.role, sample.font_px, canvas, options.viewing);
            if verdict == Legibility::Comfortable {
                continue;
            }

            sink.push(report(surface, sample, verdict, canvas, options));
        }
    }
}

fn report(
    surface: &Surface,
    sample: &TextSample,
    verdict: Legibility,
    canvas: f64,
    options: &LintOptions,
) -> Diagnostic {
    let target = min_font_px(sample.role, canvas, options.viewing);
    let severity = match verdict {
        Legibility::Unreadable => Severity::Error,
        _ => Severity::Warning,
    };

    let mut span = SourceSpan::line(surface.line);
    span.slide_index = surface.slide_index;

    Diagnostic::new(
        "legibility/font-size",
        severity,
        format!(
            "{} on {} is {:.0}px on a {canvas:.0}px canvas, {} from the back row",
            sample.origin,
            surface.name,
            sample.font_px,
            match verdict {
                Legibility::Unreadable => "unreadable",
                _ => "hard to read",
            }
        ),
    )
    .at(span)
    .with_help(format!(
        "use at least {target:.0}px for {} text, or scale the whole slide rather than the type",
        sample.role.as_token()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;
    use crate::surface::TextSample;
    use crate::test_support::lint_surfaces;
    use crate::typography::{TextRole, ViewingProfile};

    fn sized(role: TextRole, font_px: f64) -> Surface {
        Surface::new("test surface", Rgba::WHITE).with_text(TextSample::new(
            role,
            Rgba::BLACK,
            font_px,
            "theme.fontSizeBody",
        ))
    }

    #[test]
    fn comfortable_text_produces_nothing() {
        assert!(lint_surfaces(vec![sized(TextRole::Body, 32.0)], |_| {}).is_empty());
    }

    #[test]
    fn text_below_the_floor_but_close_is_a_warning() {
        let diagnostics = lint_surfaces(vec![sized(TextRole::Body, 24.0)], |_| {});
        let first = &diagnostics.as_slice()[0];

        assert_eq!(first.code, "legibility/font-size");
        assert_eq!(first.severity, Severity::Warning);
        assert!(first.message.contains("hard to read"));
    }

    #[test]
    fn text_far_below_the_floor_is_an_error() {
        let diagnostics = lint_surfaces(vec![sized(TextRole::Body, 16.0)], |_| {});
        let first = &diagnostics.as_slice()[0];

        assert_eq!(first.severity, Severity::Error);
        assert!(first.message.contains("unreadable"));
    }

    #[test]
    fn the_help_names_a_concrete_target_size() {
        let diagnostics = lint_surfaces(vec![sized(TextRole::Body, 16.0)], |_| {});
        let help = diagnostics.as_slice()[0].help.clone().unwrap();

        assert!(help.contains("28px"), "expected the calibrated body floor, got: {help}");
    }

    #[test]
    fn the_help_steers_away_from_shrinking_type_to_fix_overflow() {
        let diagnostics = lint_surfaces(vec![sized(TextRole::Body, 16.0)], |_| {});
        assert!(diagnostics.as_slice()[0].help.clone().unwrap().contains("scale the whole slide"));
    }

    #[test]
    fn code_is_held_to_a_stricter_floor_than_body_text() {
        // 30px passes as body text and fails as code, because code is denser.
        assert!(lint_surfaces(vec![sized(TextRole::Body, 30.0)], |_| {}).is_empty());
        assert!(!lint_surfaces(vec![sized(TextRole::Code, 30.0)], |_| {}).is_empty());
    }

    #[test]
    fn a_larger_room_flags_sizes_a_smaller_room_accepts() {
        let surfaces = vec![sized(TextRole::Body, 30.0)];

        let meeting = lint_surfaces(surfaces.clone(), |options| {
            options.viewing = ViewingProfile::MEETING_ROOM;
        });
        let hall = lint_surfaces(surfaces, |options| options.viewing = ViewingProfile::HALL);

        assert!(meeting.is_empty());
        assert!(!hall.is_empty(), "a hall needs more than 30px on a 1080 canvas");
    }

    #[test]
    fn the_canvas_size_is_taken_from_the_render_target_not_assumed() {
        // 14px is unreadable on a 1080 canvas and fine on a 540 one.
        let on_1080 = lint_surfaces(vec![sized(TextRole::Body, 14.0)], |_| {});
        assert!(!on_1080.is_empty());

        let on_540 = crate::test_support::lint_surfaces_with_target(
            vec![sized(TextRole::Body, 14.0)],
            crate::surface::RenderTarget { width_px: 960.0, height_px: 540.0 },
            |_| {},
        );
        assert!(on_540.is_empty(), "the same ratio must give the same verdict");
    }

    #[test]
    fn diagnostics_point_at_the_slide_they_came_from() {
        let surface = sized(TextRole::Body, 12.0).on_slide(2).at_line(9);
        let diagnostics = lint_surfaces(vec![surface], |_| {});

        assert_eq!(diagnostics.as_slice()[0].span.slide_index, Some(2));
        assert_eq!(diagnostics.as_slice()[0].span.line, 9);
    }
}
