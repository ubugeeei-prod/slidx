//! The rule set.
//!
//! Every rule has the same shape — read the input, push diagnostics — so the
//! engine is a list rather than a framework. Adding a rule is adding a module
//! and one line in [`ALL`].
//!
//! Rules never filter their own output. Suppression is applied centrally in
//! [`crate::lint`], so a suppressed code behaves identically no matter which
//! rule produced it.

pub mod budget;
pub mod contrast;
pub mod demo;
pub mod legibility;
pub mod motion;
pub mod offline;
pub mod overflow;
pub mod resolution;
pub mod structure;

use slidx_core::Diagnostics;

use crate::{LintInput, LintOptions};

/// A rule: everything it needs in, diagnostics out.
pub type RuleFn = fn(&LintInput<'_>, &LintOptions, &mut Diagnostics);

/// Every rule, in the order their diagnostics are reported.
///
/// Ordered by how early in authoring the problem is cheapest to fix: content
/// shape first, then legibility, then presentation-day concerns.
pub const ALL: &[(&str, RuleFn)] = &[
    ("structure", structure::check),
    // Also read from the Markdown body, and cheapest to fix at the moment a
    // remote URL is pasted rather than at the venue where it fails.
    ("offline", offline::check),
    // Also a content rule, and reported after `offline` because a reference
    // that has to be bundled first has no file to measure yet.
    ("resolution", resolution::check),
    ("legibility", legibility::check),
    ("contrast", contrast::check),
    // Reported after the type rules because the fix for a slide the room eats
    // into is usually to move content, and moving content is cheapest once the
    // type it is set in has stopped changing.
    ("overflow", overflow::check),
    // Registered apart from the geometry it shares a group with because it is
    // the only rule in the set whose evidence comes from a browser, and it is
    // the only one that reports nothing at all where none ran.
    ("overflow-clipped", overflow::check_measured),
    ("motion", motion::check),
    ("budget", budget::check),
    ("demo", demo::check),
    ("budget-slides", budget::check_slides),
];

/// Names of every rule group, for `--help` and for documentation.
pub fn names() -> Vec<&'static str> {
    ALL.iter().map(|(name, _)| *name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_is_registered_once() {
        let mut names = names();
        let total = names.len();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), total, "a rule is registered twice");
    }

    #[test]
    fn the_registry_is_not_empty() {
        assert!(!ALL.is_empty());
    }
}
