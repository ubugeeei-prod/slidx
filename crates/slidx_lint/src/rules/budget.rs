//! Time rules.
//!
//! Overrunning the slot is the most commonly cited delivery failure, and it is
//! decided long before the talk — at the moment someone writes forty minutes of
//! content for a twenty-minute slot. Both signals available before a rehearsal
//! are checked here: declared per-slide budgets, and an estimate derived from
//! speaker notes.

use slidx_core::{Deck, Diagnostic, Diagnostics, Severity, SourceSpan};

use crate::{LintInput, LintOptions};

/// A talk that fills less than this fraction of its slot is probably thin.
const UNDER_FILL: f64 = 0.6;

/// Tolerance before a budget total counts as over the slot.
const OVER_FILL: f64 = 1.0;

pub fn check(input: &LintInput<'_>, _options: &LintOptions, sink: &mut Diagnostics) {
    let deck = input.deck;
    let Some(slot) = deck.meta.duration_seconds.filter(|slot| *slot > 0) else {
        if deck.budgeted_seconds().is_some() {
            sink.push(
                Diagnostic::new(
                    "budget/no-slot",
                    Severity::Info,
                    "slides declare `budget:` but the deck has no `duration:` to check them against",
                )
                .with_help("add `duration: 20m` to the deck frontmatter"),
            );
        }
        return;
    };

    match deck.budgeted_seconds() {
        Some(total) => check_total(total, slot, "budget", sink),
        None => check_estimate(deck, slot, sink),
    }
}

fn check_total(total: u32, slot: u32, source: &str, sink: &mut Diagnostics) {
    let ratio = f64::from(total) / f64::from(slot);

    if ratio > OVER_FILL {
        let over = total.saturating_sub(slot);
        sink.push(
            Diagnostic::new(
                format!("{source}/over-slot"),
                Severity::Warning,
                format!(
                    "the deck runs {} against a {} slot, {} over",
                    minutes(total),
                    minutes(slot),
                    minutes(over)
                ),
            )
            .with_help("cut content, or mark the slides you can drop with `optional: true`"),
        );
    } else if ratio < UNDER_FILL {
        sink.push(
            Diagnostic::new(
                format!("{source}/under-slot"),
                Severity::Info,
                format!(
                    "the deck runs {} against a {} slot, filling {:.0}%",
                    minutes(total),
                    minutes(slot),
                    ratio * 100.0
                ),
            )
            .with_help("running short is recoverable, but plan what to expand"),
        );
    }
}

/// Falls back to the length of the speaker notes when no budgets are declared.
///
/// Rough, and available before the first rehearsal, which is exactly when the
/// content is still cheap to change.
fn check_estimate(deck: &Deck, slot: u32, sink: &mut Diagnostics) {
    let estimate = deck.estimated_seconds();
    if estimate == 0 {
        return;
    }

    if f64::from(estimate) / f64::from(slot) > OVER_FILL {
        sink.push(
            Diagnostic::new(
                "budget/estimate-over",
                Severity::Info,
                format!(
                    "speaker notes read as about {} against a {} slot",
                    minutes(estimate),
                    minutes(slot)
                ),
            )
            .with_help("an estimate, not a measurement — confirm it with `slidx rehearse`"),
        );
    }
}

/// Flags slides whose notes are far longer than their declared budget.
pub fn check_slides(input: &LintInput<'_>, _options: &LintOptions, sink: &mut Diagnostics) {
    for slide in &input.deck.slides {
        let Some(budget) = slide.budget_seconds.filter(|budget| *budget > 0) else { continue };
        let estimate = slide.estimated_seconds();

        if f64::from(estimate) > f64::from(budget) * 1.5 {
            sink.push(
                Diagnostic::new(
                    "budget/slide-over",
                    Severity::Info,
                    format!(
                        "\"{}\" is budgeted {} but its notes read as about {}",
                        slide.display_title(),
                        minutes(budget),
                        minutes(estimate)
                    ),
                )
                .at(SourceSpan::line(slide.source_line).on_slide(slide.index)),
            );
        }
    }
}

fn minutes(seconds: u32) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }

    let (whole, rest) = (seconds / 60, seconds % 60);
    if rest == 0 {
        format!("{whole}m")
    } else {
        format!("{whole}m{rest}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::lint_deck;

    #[test]
    fn a_deck_without_a_slot_is_not_checked() {
        assert!(lint_deck("# One\n").is_empty());
    }

    #[test]
    fn budgets_without_a_slot_are_reported_as_unusable() {
        let diagnostics =
            lint_deck("---\nbudget: 60s\n---\n\n# One\n\n---\nbudget: 60s\n---\n\n# Two\n");

        let first = diagnostics.iter().find(|d| d.code == "budget/no-slot").unwrap();
        assert!(first.help.as_ref().unwrap().contains("duration"));
    }

    #[test]
    fn a_deck_that_fits_its_slot_produces_nothing() {
        let diagnostics = lint_deck(
            "---\nduration: 5m\nbudget: 150s\n---\n\n# One\n\n---\nbudget: 150s\n---\n\n# Two\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn a_deck_over_its_slot_is_warned_with_the_overrun() {
        let diagnostics = lint_deck(
            "---\nduration: 5m\nbudget: 240s\n---\n\n# One\n\n---\nbudget: 240s\n---\n\n# Two\n",
        );

        let first = diagnostics.iter().find(|d| d.code == "budget/over-slot").unwrap();
        assert_eq!(first.severity, Severity::Warning);
        assert!(first.message.contains("3m over"), "got: {}", first.message);
    }

    #[test]
    fn the_over_slot_help_points_at_optional_slides() {
        let diagnostics = lint_deck("---\nduration: 1m\nbudget: 5m\n---\n\n# One\n");
        let first = diagnostics.iter().find(|d| d.code == "budget/over-slot").unwrap();
        assert!(first.help.as_ref().unwrap().contains("optional"));
    }

    #[test]
    fn a_deck_far_under_its_slot_is_noted_rather_than_warned() {
        let diagnostics = lint_deck("---\nduration: 20m\nbudget: 60s\n---\n\n# One\n");

        let first = diagnostics.iter().find(|d| d.code == "budget/under-slot").unwrap();
        assert_eq!(first.severity, Severity::Info);
        assert!(first.message.contains('%'));
    }

    #[test]
    fn a_partially_budgeted_deck_falls_back_to_the_notes_estimate() {
        // One slide budgeted and one not means no usable total, so the
        // estimate path has to take over rather than reporting nothing.
        let notes = "word ".repeat(400);
        let source =
            format!("---\nduration: 1m\nbudget: 30s\n---\n\n# One\n\n---\n\n# Two\n\n<!-- notes: {notes} -->\n");

        let diagnostics = lint_deck(&source);
        assert!(diagnostics.iter().any(|d| d.code == "budget/estimate-over"));
    }

    #[test]
    fn the_estimate_is_labelled_as_an_estimate() {
        let notes = "word ".repeat(400);
        let diagnostics =
            lint_deck(&format!("---\nduration: 1m\n---\n\n# One\n\n<!-- notes: {notes} -->\n"));

        let first = diagnostics.iter().find(|d| d.code == "budget/estimate-over").unwrap();
        assert_eq!(first.severity, Severity::Info);
        assert!(first.help.as_ref().unwrap().contains("rehearse"));
    }

    #[test]
    fn a_deck_with_no_notes_and_no_budgets_produces_nothing() {
        assert!(lint_deck("---\nduration: 20m\n---\n\n# One\n").is_empty());
    }

    #[test]
    fn a_slide_whose_notes_overflow_its_budget_is_noted() {
        let notes = "word ".repeat(200);
        let diagnostics = lint_deck(&format!(
            "---\nduration: 20m\nbudget: 10s\n---\n\n# Dense\n\n<!-- notes: {notes} -->\n"
        ));

        let first = diagnostics.iter().find(|d| d.code == "budget/slide-over").unwrap();
        assert!(first.message.contains("Dense"));
        assert_eq!(first.span.slide_index, Some(0));
    }

    #[test]
    fn durations_are_rendered_the_way_people_say_them() {
        assert_eq!(minutes(45), "45s");
        assert_eq!(minutes(60), "1m");
        assert_eq!(minutes(1200), "20m");
        assert_eq!(minutes(90), "1m30s");
    }
}
