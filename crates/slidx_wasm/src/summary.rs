//! What changed between two versions of a deck, reachable from JavaScript.
//!
//! The editor's history panel shows this against a commit. It is the same
//! [`slidx_core::Summary`] that `slidx save` writes into a commit message, so
//! the sentence describing a change is composed once and read in both places —
//! a talk's history and its own record of itself cannot word the same edit two
//! ways.
//!
//! # Why both sides are sources rather than parsed decks
//!
//! A caller with a filesystem — the dev server — reads the deck at a commit as
//! text and has nowhere to put a parsed model: there is no handle to hold
//! across this boundary and nothing to free. Handing over two strings means the
//! parse happens on the side that owns the parser, with the same options a
//! build uses, so a summary cannot be computed against a differently segmented
//! deck than the one the canvas is showing.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use wasm_bindgen::prelude::*;

use slidx_core::{parse_deck, Summary};

use crate::{parse_options, to_js};

/// Settings a summary needs. The separator is the only one that changes which
/// bytes a slide is, and therefore which slides there are to compare.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SummaryOptions {
    pub separator: Option<String>,
}

/// What changed between two versions of a deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeckSummary {
    /// True when there was nothing to compare against — the deck arriving,
    /// rather than the deck changing.
    pub first: bool,
    /// How many slides the newer of the two decks has.
    pub slides: u32,
    /// The one line, in the deck's own vocabulary.
    ///
    /// Empty when the deck did not change at all, which is an ordinary commit
    /// that touched something else in the repository.
    pub subject: String,
    /// The rest of the changes, one sentence each.
    ///
    /// Empty when the subject already said the only thing that happened, so a
    /// consumer can render both without repeating one of them.
    pub changes: Vec<String>,
}

/// Compares two versions of a deck and says what changed, in slides.
///
/// `before` absent means there is nothing to compare against — the deck's first
/// commit. That is a different answer from an empty deck, not a degenerate case
/// of one: "34 slides added" is a strange way to describe a talk arriving.
#[wasm_bindgen(js_name = deckSummary, unchecked_return_type = "DeckSummary")]
pub fn deck_summary(
    before: Option<String>,
    after: &str,
    options: JsValue,
) -> Result<JsValue, JsError> {
    let options: SummaryOptions = if options.is_undefined() || options.is_null() {
        SummaryOptions::default()
    } else {
        serde_wasm_bindgen::from_value(options)
            .map_err(|error| JsError::new(&format!("invalid options: {error}")))?
    };

    to_js(&summarise(before.as_deref(), after, &options))
}

fn summarise(before: Option<&str>, after: &str, options: &SummaryOptions) -> DeckSummary {
    let parse = parse_options(options.separator.as_deref());
    let now = parse_deck(after, &parse);

    let summary = match before {
        Some(before) => Summary::of(&parse_deck(before, &parse), &now),
        None => Summary::first(&now),
    };

    // A deck that did not change gets no subject rather than a made-up one:
    // `Summary::subject` has to say *something* for a commit message, and
    // "Save the deck" written against a commit that did not touch the deck
    // would be this panel inventing a change.
    if summary.is_empty() {
        return DeckSummary {
            first: false,
            slides: now.slides.len() as u32,
            subject: String::new(),
            changes: Vec::new(),
        };
    }

    DeckSummary {
        first: summary.first,
        slides: summary.slides as u32,
        subject: summary.subject(),
        changes: summary.body(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECK: &str = "---\ntitle: Making decks fast\nduration: 20m\n---\n\n# Making decks fast\n\n---\n\n# What goes wrong\n\nthe wifi\n\n---\n\n# The fix\n";

    fn summary(before: Option<&str>, after: &str) -> DeckSummary {
        summarise(before, after, &SummaryOptions::default())
    }

    #[test]
    fn a_change_crosses_the_boundary_as_slides_rather_than_as_lines() {
        let after = format!("{DECK}\n---\n\n# What it cost\n");

        assert_eq!(summary(Some(DECK), &after).subject, "Add \"What it cost\"");
    }

    #[test]
    fn the_subject_and_the_rest_never_say_the_same_thing_twice() {
        // One change is the whole subject. A panel rendering both would show it
        // to an author twice, which is why the split is decided here and not by
        // whoever is drawing it.
        let after = format!("{DECK}\n---\n\n# What it cost\n");
        assert!(summary(Some(DECK), &after).changes.is_empty());

        let two = format!("{}\n---\n\n# What it cost\n", DECK.replace("20m", "25m"));
        assert!(summary(Some(DECK), &two).changes.len() >= 2);
    }

    #[test]
    fn a_deck_that_did_not_change_says_nothing_rather_than_inventing_a_change() {
        // An ordinary commit that touched a README and no slide.
        let unchanged = summary(Some(DECK), DECK);

        assert_eq!(unchanged.subject, "");
        assert!(unchanged.changes.is_empty());
        assert_eq!(unchanged.slides, 3);
    }

    #[test]
    fn no_earlier_version_is_a_deck_arriving_rather_than_a_deck_changing() {
        // The deck's first commit has no parent to read, and `git show <root>^`
        // has no answer either. Absence is the case, not an error.
        let first = summary(None, DECK);

        assert!(first.first);
        assert_eq!(first.subject, "Add the deck, 3 slides");
        assert_eq!(first.slides, 3);
    }

    #[test]
    fn a_reorder_reads_as_a_reorder_across_the_boundary_too() {
        let after = "---\ntitle: Making decks fast\nduration: 20m\n---\n\n# Making decks fast\n\n---\n\n# The fix\n\n---\n\n# What goes wrong\n\nthe wifi\n";
        let summary = summary(Some(DECK), after);

        assert_eq!(summary.subject, "Reorder 2 slides");
        assert_eq!(summary.slides, 3, "nothing was added or dropped");
    }

    #[test]
    fn the_separator_decides_which_slides_there_are_to_compare() {
        // A deck stored one slide per file is joined with whatever separator
        // the project configured. Summarising with a different one would
        // compare a three-slide deck against a one-slide deck.
        let options = SummaryOptions { separator: Some("***".to_string()) };
        let before = "# One\n\n***\n\n# Two\n";
        let after = "# One\n\n***\n\n# Two\n\n***\n\n# Three\n";

        assert_eq!(summarise(Some(before), after, &options).slides, 3);
        assert_eq!(summarise(Some(before), after, &SummaryOptions::default()).slides, 1);
    }
}
