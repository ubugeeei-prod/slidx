//! Contrast rules.
//!
//! # Why the large-text exemption does not apply
//!
//! WCAG relaxes the contrast floor from 4.5:1 to 3:1 for large text, because
//! large glyphs are easier to resolve *at a fixed viewing distance*. A slide
//! breaks that assumption: everything on it is large, and the audience is
//! correspondingly further away. Applying the exemption would exempt an entire
//! deck and leave the rule checking nothing.
//!
//! So slidx holds every piece of readable text to 4.5:1, and checks it against
//! the ratio the *room* will show rather than the one the laptop shows.

use slidx_core::{Diagnostic, Diagnostics, Severity, SourceSpan};

use crate::color::{contrast_ratio, projected_contrast_ratio};
use crate::surface::Surface;
use crate::typography::TextRole;
use crate::{LintInput, LintOptions};

/// Minimum contrast ratio for a role.
fn floor(role: TextRole) -> f64 {
    match role {
        // Not required reading, so the AA large-text floor is defensible here.
        TextRole::Caption => 3.0,
        _ => 4.5,
    }
}

/// Enhanced floor, checked when the deck opts into strict mode.
const ENHANCED_FLOOR: f64 = 7.0;

pub fn check(input: &LintInput<'_>, options: &LintOptions, sink: &mut Diagnostics) {
    for surface in input.surfaces {
        for sample in &surface.text {
            let resolved = surface.composited(sample);
            let direct = contrast_ratio(resolved, surface.background);
            let projected =
                projected_contrast_ratio(resolved, surface.background, options.projector);
            let floor = floor(sample.role);

            if direct < floor {
                sink.push(too_low(surface, sample, direct, floor));
            } else if projected < floor {
                sink.push(projector_only(surface, sample, direct, projected, floor, options));
            } else if options.strict && direct < ENHANCED_FLOOR {
                sink.push(
                    diagnostic(
                        surface,
                        "contrast/below-enhanced",
                        Severity::Info,
                        format!(
                            "{} on {} is {direct:.1}:1, below the {ENHANCED_FLOOR:.0}:1 enhanced floor",
                            sample.origin, surface.name
                        ),
                    )
                    .with_help("dark rooms reward more contrast than the AA minimum"),
                );
            }
        }
    }
}

fn too_low(
    surface: &Surface,
    sample: &crate::surface::TextSample,
    direct: f64,
    floor: f64,
) -> Diagnostic {
    // Far below the floor is a mistake; just below it is a judgement call.
    let severity = if direct < floor * 0.66 { Severity::Error } else { Severity::Warning };

    diagnostic(
        surface,
        "contrast/too-low",
        severity,
        format!(
            "{} on {} is {direct:.1}:1, below the {floor:.1}:1 floor for {} text",
            sample.origin,
            surface.name,
            sample.role.as_token()
        ),
    )
    .with_help("darken the text, lighten the background, or use the `contrast` theme")
}

fn projector_only(
    surface: &Surface,
    sample: &crate::surface::TextSample,
    direct: f64,
    projected: f64,
    floor: f64,
    options: &LintOptions,
) -> Diagnostic {
    diagnostic(
        surface,
        "contrast/projector",
        Severity::Warning,
        format!(
            "{} on {} passes at {direct:.1}:1 on a monitor but drops to {projected:.1}:1 \
             under {} projection, below the {floor:.1}:1 floor",
            sample.origin,
            surface.name,
            options.projector.as_token()
        ),
    )
    .with_help(
        "ambient light raises the screen's black level; this pair will look flat from the back row",
    )
}

fn diagnostic(surface: &Surface, code: &str, severity: Severity, message: String) -> Diagnostic {
    let mut span = SourceSpan::line(surface.line);
    span.slide_index = surface.slide_index;
    Diagnostic::new(code, severity, message).at(span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{self, ProjectorProfile, Rgba};
    use crate::surface::TextSample;
    use crate::test_support::lint_surfaces;

    fn surface_with(color: &str, background: &str, role: TextRole) -> Surface {
        Surface::new("test surface", color::parse(background).unwrap()).with_text(TextSample::new(
            role,
            color::parse(color).unwrap(),
            28.0,
            "theme.colorText",
        ))
    }

    #[test]
    fn a_strong_pair_produces_nothing() {
        let diagnostics =
            lint_surfaces(vec![surface_with("#000000", "#ffffff", TextRole::Body)], |_| {});
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn a_pair_far_below_the_floor_is_an_error() {
        let diagnostics =
            lint_surfaces(vec![surface_with("#cccccc", "#ffffff", TextRole::Body)], |_| {});

        let first = &diagnostics.as_slice()[0];
        assert_eq!(first.code, "contrast/too-low");
        assert_eq!(first.severity, Severity::Error);
    }

    #[test]
    fn a_pair_just_below_the_floor_is_a_warning() {
        // #949494 on white is about 3.2:1 — under 4.5 but not egregious.
        let diagnostics =
            lint_surfaces(vec![surface_with("#949494", "#ffffff", TextRole::Body)], |_| {});

        let first = &diagnostics.as_slice()[0];
        assert_eq!(first.code, "contrast/too-low");
        assert_eq!(first.severity, Severity::Warning);
    }

    #[test]
    fn a_pair_that_passes_on_a_monitor_and_fails_in_the_room_is_reported_separately() {
        // This is the rule that justifies the whole projector model.
        let diagnostics =
            lint_surfaces(vec![surface_with("#767676", "#ffffff", TextRole::Body)], |_| {});

        let first = &diagnostics.as_slice()[0];
        assert_eq!(first.code, "contrast/projector");
        assert!(first.message.contains("on a monitor"));
        assert!(first.message.contains("typical"));
    }

    #[test]
    fn a_darker_room_forgives_a_pair_a_brighter_room_flags() {
        // #666666 on white clears the floor with a little headroom, which a
        // dark room preserves and a bright one eats.
        let surfaces = vec![surface_with("#666666", "#ffffff", TextRole::Body)];

        let dark = lint_surfaces(surfaces.clone(), |options| {
            options.projector = ProjectorProfile::DarkRoom;
        });
        let bright = lint_surfaces(surfaces, |options| {
            options.projector = ProjectorProfile::BrightRoom;
        });

        assert!(dark.is_empty(), "a dark room preserves enough contrast for this pair");
        assert_eq!(bright.as_slice()[0].code, "contrast/projector");
    }

    #[test]
    fn captions_are_held_to_the_lower_floor() {
        // The precondition the test rests on: this pair sits between the two
        // floors, so the role is the only thing that decides the verdict.
        let foreground = color::parse("#8a8a8a").unwrap();
        let ratio = crate::color::projected_contrast_ratio(
            foreground,
            Rgba::WHITE,
            ProjectorProfile::default(),
        );
        assert!((3.0..4.5).contains(&ratio), "fixture drifted: {ratio:.2}:1");

        let body = lint_surfaces(vec![surface_with("#8a8a8a", "#ffffff", TextRole::Body)], |_| {});
        let caption =
            lint_surfaces(vec![surface_with("#8a8a8a", "#ffffff", TextRole::Caption)], |_| {});

        assert_eq!(body.as_slice()[0].code, "contrast/too-low");
        assert!(caption.is_empty(), "captions are not required reading");
    }

    #[test]
    fn translucent_text_is_judged_on_what_the_audience_sees() {
        // Declared as pure black, but at 25% alpha it is effectively #bfbfbf.
        let surface = Surface::new("panel", Rgba::WHITE).with_text(TextSample::new(
            TextRole::Body,
            color::parse("#00000040").unwrap(),
            28.0,
            "theme.colorTextMuted",
        ));

        let diagnostics = lint_surfaces(vec![surface], |_| {});
        assert_eq!(diagnostics.as_slice()[0].code, "contrast/too-low");
    }

    #[test]
    fn strict_mode_notes_pairs_that_pass_aa_but_not_the_enhanced_floor() {
        // The precondition: comfortably over 4.5:1, comfortably under 7:1.
        let ratio = crate::color::contrast_ratio(color::parse("#666666").unwrap(), Rgba::WHITE);
        assert!((4.5..7.0).contains(&ratio), "fixture drifted: {ratio:.2}:1");

        let surfaces = vec![surface_with("#666666", "#ffffff", TextRole::Body)];

        assert!(lint_surfaces(surfaces.clone(), |_| {}).is_empty());

        let strict = lint_surfaces(surfaces, |options| options.strict = true);
        assert_eq!(strict.as_slice()[0].code, "contrast/below-enhanced");
        assert_eq!(strict.as_slice()[0].severity, Severity::Info);
    }

    #[test]
    fn diagnostics_point_at_the_slide_and_line_they_came_from() {
        let surface = surface_with("#eeeeee", "#ffffff", TextRole::Body).on_slide(4).at_line(17);
        let diagnostics = lint_surfaces(vec![surface], |_| {});

        assert_eq!(diagnostics.as_slice()[0].span.slide_index, Some(4));
        assert_eq!(diagnostics.as_slice()[0].span.line, 17);
    }

    #[test]
    fn every_diagnostic_offers_a_next_action() {
        let diagnostics =
            lint_surfaces(vec![surface_with("#cccccc", "#ffffff", TextRole::Body)], |_| {});
        assert!(diagnostics.as_slice()[0].help.is_some());
    }
}
