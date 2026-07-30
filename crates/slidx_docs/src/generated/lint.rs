//! The rule groups, and what each one is looking for.
//!
//! # Groups rather than codes
//!
//! The linter has a registry of groups and no registry of individual codes —
//! a code is a string literal inside the rule that raises it, and there are
//! nearly thirty. Listing them here would mean maintaining a second copy of
//! every message, and a diagnostic already says more than a table can: it
//! carries the code, the slide, and a concrete next action. So this documents
//! the groups, which is also the granularity `--allow` works at.
//!
//! # The sentences are held to the registry
//!
//! [`slidx_lint::rules::ALL`] is the source of truth for which groups exist,
//! and the prose below cannot be derived from it. So it is a second list, and a
//! second list is only safe if something fails when the two disagree — which is
//! what the tests at the bottom do, in both directions: a group with no
//! sentence, and a sentence naming a group that no longer exists.

use super::{code, prose, table};

/// What each rule group looks for, in one sentence.
///
/// Kept beside the generator rather than in the page, because the page is
/// Markdown and cannot be held to the registry by a test.
const DESCRIPTIONS: &[(&str, &str)] = &[
    ("structure", "Missing alt text, heading order, bare URLs, and how much a slide is asking an audience to read at once."),
    ("offline", "Anything the deck would fetch at the venue — an image, a font, an embed. The rule the whole offline guarantee rests on."),
    ("resolution", "An image asked to render larger than its own pixels, or stretched out of its aspect ratio."),
    ("legibility", "Rendered type against the angular size a glyph subtends from the back row, rather than against a pixel threshold."),
    ("contrast", "The WCAG ratio, and then the same pair again through a model of what a projector's washout does to it."),
    ("overflow", "Content against the declared safe area and the venue's caption strip. Arithmetic on numbers you gave it, so it runs everywhere."),
    ("overflow-clipped", "Content that does not fit, measured in a real browser. The only rule whose evidence is a measurement, and the only one that reports *unchecked* rather than clean when there is none."),
    ("motion", "Effects that will not stay on the compositor, and slides asking for too many at once."),
    ("budget", "The per-slide budgets you wrote, summed against the slot the deck declared."),
    ("demo", "A demo slide with no declared fallback, or one whose fallback would have to be fetched when the demo dies."),
    ("budget-slides", "One slide's budget against how long its content would take to say."),
];

/// The rule groups, in the order the linter reports them.
pub fn groups() -> String {
    let rows = slidx_lint::rules::names()
        .into_iter()
        .map(|name| vec![code(name), prose(description(name))])
        .collect();

    table(&["Group", "What it looks for"], rows)
}

fn description(name: &str) -> &'static str {
    DESCRIPTIONS
        .iter()
        .find(|(group, _)| *group == name)
        .map(|(_, text)| *text)
        .unwrap_or("Undescribed.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_group_the_linter_runs_is_described() {
        // A group added upstream and not described here would render as
        // "Undescribed." rather than quietly vanishing, but it would still be a
        // reference page that failed the reader. This fails the build instead.
        for name in slidx_lint::rules::names() {
            assert!(
                DESCRIPTIONS.iter().any(|(group, _)| *group == name),
                "the {name} rule group has no sentence in slidx_docs"
            );
        }
    }

    #[test]
    fn no_sentence_describes_a_group_that_no_longer_exists() {
        // The other direction, and the one that rots silently: a rule is
        // renamed, the page keeps describing the old name, and every reader
        // who searches for it finds documentation for nothing.
        let names = slidx_lint::rules::names();

        for (group, _) in DESCRIPTIONS {
            assert!(names.contains(group), "{group} is described but is not a rule group");
        }
    }

    #[test]
    fn the_table_is_in_the_order_the_linter_reports_findings() {
        // Ordered by how early in authoring the problem is cheapest to fix,
        // which is a decision the linter made and this page should not re-make.
        let html = groups();
        let position = |name: &str| html.find(name).expect("a listed group");

        assert!(position("structure") < position("legibility"));
        assert!(position("legibility") < position("budget"));
    }
}
