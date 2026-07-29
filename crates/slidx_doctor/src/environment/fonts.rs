//! Font stacks a theme names, and the families this machine actually has.
//!
//! A missing font never announces itself. The browser silently substitutes,
//! the substitute is wider, the title wraps onto three lines, and the speaker
//! finds out from the back of the room. So the question this module answers is
//! not "is the font installed" but "which member of the stack will the browser
//! land on" — landing on the second one is a different problem from landing on
//! nothing, and they need different advice.
//!
//! Matching is deliberately loose: family names reach us from directory
//! listings and from `fc-list`, which disagree about spaces, hyphens and case
//! (`HelveticaNeue.ttc` against `Helvetica Neue`). Comparing on letters and
//! digits alone is approximate in the forgiving direction — it will call a
//! present font present, and the cost of that is a check that stays quiet
//! rather than one that cries wolf about a font the speaker can see working.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// CSS generic families. The browser resolves these on every machine, so a
/// stack that ends in one can degrade but can never fail outright — which is
/// the reason every built-in slidx theme ends in one.
const GENERIC_FAMILIES: &[&str] = &[
    "serif",
    "sansserif",
    "monospace",
    "cursive",
    "fantasy",
    "systemui",
    "uiserif",
    "uisansserif",
    "uimonospace",
    "uirounded",
    "math",
    "emoji",
    "fangsong",
];

/// Case, spacing and punctuation removed, so two spellings of one family
/// compare equal.
pub fn normalise(family: &str) -> String {
    family.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// True for a family the browser is guaranteed to resolve.
pub fn is_generic(family: &str) -> bool {
    GENERIC_FAMILIES.contains(&normalise(family).as_str())
}

/// One font stack from a theme, in the order the browser will try it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontStack {
    /// Which text this stack draws — `sans`, `mono`. Named so the finding can
    /// say *what* will look wrong rather than just which family is missing.
    pub role: String,
    pub families: Vec<String>,
}

impl FontStack {
    /// Parses a CSS font stack: comma separated, optionally quoted.
    ///
    /// Takes the string straight off `theme.font_sans` so the theme and the
    /// doctor cannot drift apart by way of a hand-maintained list.
    pub fn parse(role: impl Into<String>, css: &str) -> Self {
        let families = css
            .split(',')
            .map(|family| family.trim().trim_matches(['"', '\'']).trim().to_string())
            .filter(|family| !family.is_empty())
            .collect();

        Self { role: role.into(), families }
    }

    pub fn new(role: impl Into<String>, families: impl IntoIterator<Item: Into<String>>) -> Self {
        Self { role: role.into(), families: families.into_iter().map(Into::into).collect() }
    }

    /// Which family the browser will land on.
    pub fn resolve(&self, installed: &InstalledFonts) -> Resolution {
        let found = self
            .families
            .iter()
            .position(|family| is_generic(family) || installed.contains(family));

        match found {
            Some(0) => Resolution::Preferred(self.families[0].clone()),
            // Reaching a later member means the deck was laid out against
            // metrics the room will not see. It renders; it does not match.
            Some(index) => Resolution::Fallback { index, family: self.families[index].clone() },
            None => Resolution::Missing,
        }
    }

    /// The stack as CSS writes it, for the message.
    pub fn label(&self) -> String {
        self.families.join(", ")
    }
}

/// Where in a stack the browser stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The family the theme actually wanted.
    Preferred(String),
    /// A later member of the stack, with its position.
    Fallback { index: usize, family: String },
    /// Nothing in the stack resolves and it names no generic, so the browser
    /// picks a default nobody designed against.
    Missing,
}

/// Font families this machine can resolve.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstalledFonts {
    /// Stored normalised: the raw spelling is never shown, only matched.
    families: BTreeSet<String>,
}

impl InstalledFonts {
    pub fn contains(&self, family: &str) -> bool {
        self.families.contains(&normalise(family))
    }

    pub fn len(&self) -> usize {
        self.families.len()
    }

    pub fn is_empty(&self) -> bool {
        self.families.is_empty()
    }
}

impl<S: AsRef<str>> FromIterator<S> for InstalledFonts {
    fn from_iter<T: IntoIterator<Item = S>>(iter: T) -> Self {
        Self {
            families: iter
                .into_iter()
                .map(|family| normalise(family.as_ref()))
                .filter(|family| !family.is_empty())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(families: &[&str]) -> InstalledFonts {
        families.iter().collect()
    }

    #[test]
    fn a_css_stack_is_parsed_in_the_order_the_browser_tries_it() {
        let stack = FontStack::parse("sans", "Inter, 'Helvetica Neue', sans-serif");

        assert_eq!(stack.families, vec!["Inter", "Helvetica Neue", "sans-serif"]);
    }

    #[test]
    fn a_stack_split_across_source_lines_parses_the_same() {
        // Theme constants wrap with a trailing backslash, so families arrive
        // with newlines and runs of spaces embedded in them.
        let stack = FontStack::parse("sans", "system-ui,\n    -apple-system,\n    'Segoe UI'");

        assert_eq!(stack.families, vec!["system-ui", "-apple-system", "Segoe UI"]);
    }

    #[test]
    fn a_family_present_under_a_different_spelling_still_matches() {
        // Directory listings give `HelveticaNeue`, fc-list gives
        // `Helvetica Neue`. Treating those as different fonts would report a
        // failure the speaker can see is not real.
        let fonts = installed(&["HelveticaNeue"]);

        assert!(fonts.contains("Helvetica Neue"));
        assert!(fonts.contains("helvetica neue"));
    }

    #[test]
    fn an_unrelated_family_does_not_match() {
        assert!(!installed(&["Helvetica"]).contains("Helvetica Neue"));
    }

    #[test]
    fn the_preferred_family_resolves_when_it_is_installed() {
        let stack = FontStack::parse("sans", "Inter, sans-serif");

        assert_eq!(stack.resolve(&installed(&["Inter"])), Resolution::Preferred("Inter".into()));
    }

    #[test]
    fn a_missing_first_choice_reports_which_family_takes_over() {
        // The speaker needs to know what they will be looking at, not just
        // that something is absent.
        let stack = FontStack::parse("sans", "Inter, Arial, sans-serif");

        assert_eq!(
            stack.resolve(&installed(&["Arial"])),
            Resolution::Fallback { index: 1, family: "Arial".into() }
        );
    }

    #[test]
    fn a_generic_family_always_resolves_even_on_a_machine_with_no_fonts_listed() {
        // The browser guarantees these. A stack ending in one cannot land on
        // nothing, which is exactly why the built-in themes end in one.
        let stack = FontStack::parse("sans", "Inter, sans-serif");

        assert_eq!(
            stack.resolve(&InstalledFonts::default()),
            Resolution::Fallback { index: 1, family: "sans-serif".into() }
        );
    }

    #[test]
    fn a_stack_with_no_generic_and_nothing_installed_resolves_to_nothing() {
        // This is the case worth failing on: the browser picks its own default
        // and the deck's line lengths stop meaning anything.
        let stack = FontStack::parse("display", "Söhne, 'GT America'");

        assert_eq!(stack.resolve(&installed(&["Arial"])), Resolution::Missing);
    }

    #[test]
    fn the_built_in_slidx_sans_stack_resolves_on_any_machine() {
        // slidx themes lead with `system-ui` precisely so this check stays
        // green at a venue. If that ever changes, this test should be the
        // thing that objects.
        let stack = FontStack::parse(
            "sans",
            "system-ui, -apple-system, 'Segoe UI', 'Helvetica Neue', \
             'Hiragino Sans', 'Noto Sans JP', 'Yu Gothic UI', sans-serif",
        );

        assert_eq!(
            stack.resolve(&InstalledFonts::default()),
            Resolution::Preferred("system-ui".into())
        );
    }

    #[test]
    fn every_generic_family_is_stored_already_normalised() {
        // The list is compared against normalised input, so an entry written
        // as `sans-serif` would never match anything.
        for family in GENERIC_FAMILIES {
            assert_eq!(&normalise(family), family, "{family} is not in normal form");
        }
    }

    #[test]
    fn font_names_that_normalise_to_nothing_are_dropped() {
        // A directory scan turns up `.DS_Store` and `..`; keeping them as
        // empty keys would make every empty lookup succeed.
        let fonts = installed(&["...", "Inter"]);

        assert_eq!(fonts.len(), 1);
        assert!(!fonts.contains(""));
    }

    #[test]
    fn a_family_named_in_a_non_latin_script_survives_normalisation() {
        // CJK families are named in their own script on the machines that ship
        // them; stripping non-ASCII would make every one of them look missing.
        assert!(installed(&["ヒラギノ角ゴシック"]).contains("ヒラギノ角ゴシック"));
    }
}
