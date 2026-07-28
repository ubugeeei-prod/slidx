//! Animation rules.
//!
//! Venue hardware is usually older and weaker than the machine a deck was
//! authored on. An effect that animates only transform and opacity stays on
//! the compositor and holds frame rate anywhere; one that animates layout or
//! paint properties will judder on stage even though it was smooth at a desk.

use slidx_core::{Diagnostic, Diagnostics, Severity, SourceSpan};

use crate::{LintInput, LintOptions};

/// Stops on one slide beyond which the slide is probably two slides.
const BUSY_STOP_COUNT: usize = 8;

pub fn check(input: &LintInput<'_>, _options: &LintOptions, sink: &mut Diagnostics) {
    for slide in &input.deck.slides {
        let span = SourceSpan::line(slide.source_line).on_slide(slide.index);

        for frame in slide.timeline.frames() {
            for state in &frame.states {
                let Some(effect) = &state.effect else { continue };
                if effect.preset.is_compositor_only() {
                    continue;
                }

                sink.push(
                    Diagnostic::new(
                        "motion/paint-heavy",
                        Severity::Warning,
                        format!(
                            "`{}` on {} animates paint rather than transform, and will judder on \
                             venue hardware",
                            effect.preset.as_token(),
                            state.target
                        ),
                    )
                    .at(span)
                    .with_help("prefer `fade`, `fly-in`, `zoom`, or `wipe` for anything on stage"),
                );
            }
        }

        if slide.timeline.len() > BUSY_STOP_COUNT {
            sink.push(
                Diagnostic::new(
                    "motion/busy-slide",
                    Severity::Info,
                    format!(
                        "{} stops on one slide ({})",
                        slide.timeline.len(),
                        slide.display_title()
                    ),
                )
                .at(span)
                .with_help("a slide needing this many clicks is usually two slides"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::lint_deck;

    #[test]
    fn a_deck_with_no_animation_produces_nothing() {
        assert!(lint_deck("# One\n\n# Two\n").is_empty());
    }

    #[test]
    fn compositor_friendly_presets_pass() {
        let diagnostics = lint_deck(
            "---\nsteps:\n  - reveal: { target: \".a\", preset: fly-in }\n  - reveal: { target: \".b\", preset: zoom }\n---\n\n# One\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn a_paint_heavy_preset_is_flagged_with_its_target() {
        let diagnostics = lint_deck(
            "---\nsteps:\n  - reveal: { target: \".code\", preset: typewriter }\n---\n\n# One\n",
        );

        let first = &diagnostics.as_slice()[0];
        assert_eq!(first.code, "motion/paint-heavy");
        assert!(first.message.contains("typewriter"));
        assert!(first.message.contains(".code"));
    }

    #[test]
    fn a_paint_heavy_preset_is_reported_once_per_occurrence_not_once_per_frame() {
        // Effects live only on the frame that triggers them, so a three-step
        // slide with one typewriter reveal must report exactly one problem.
        let diagnostics = lint_deck(
            "---\nsteps:\n  - reveal: \".a\"\n  - reveal: { target: \".b\", preset: draw }\n  - reveal: \".c\"\n---\n\n# One\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn a_slide_with_too_many_stops_is_noted() {
        let steps: String = (1..=10).map(|n| format!("  - reveal: \".s{n}\"\n")).collect();
        let diagnostics = lint_deck(&format!("---\nsteps:\n{steps}---\n\n# Busy\n"));

        let busy = diagnostics.iter().find(|d| d.code == "motion/busy-slide").unwrap();
        assert_eq!(busy.severity, Severity::Info);
        assert!(busy.message.contains("Busy"));
    }

    #[test]
    fn a_slide_at_the_stop_limit_is_not_flagged() {
        let steps: String = (1..=7).map(|n| format!("  - reveal: \".s{n}\"\n")).collect();
        let diagnostics = lint_deck(&format!("---\nsteps:\n{steps}---\n\n# Fine\n"));

        assert!(diagnostics.iter().all(|d| d.code != "motion/busy-slide"));
    }

    #[test]
    fn diagnostics_carry_the_slide_index() {
        let diagnostics = lint_deck(
            "# One\n\n---\nsteps:\n  - reveal: { target: \".a\", preset: typewriter }\n---\n\n# Two\n",
        );
        assert_eq!(diagnostics.as_slice()[0].span.slide_index, Some(1));
    }
}
