//! Frontmatter keys and the closed sets they accept.
//!
//! Every table here comes out of [`slidx_lsp::vocabulary`], which is the module
//! that already refuses to restate an enum: its preset names come from
//! `EffectPreset::ALL`, its themes from `slidx_theme::builtin::all`, and the
//! prose beside each one is attached with an exhaustive `match`, so adding a
//! variant upstream fails that crate's build until somebody writes the
//! sentence.
//!
//! That makes this the cheapest possible reference page: the language server
//! and the site show a reader the same words, because there is only one set of
//! them.

use slidx_lsp::vocabulary::{self, Key, Scope, Term};

use super::{code, prose, table};

/// Keys that only mean anything in the first frontmatter block.
pub fn deck_keys() -> String {
    keys(Scope::Deck)
}

/// Keys that configure one slide, in any block including the first.
pub fn slide_keys() -> String {
    keys(Scope::Slide)
}

fn keys(scope: Scope) -> String {
    let rows = vocabulary::KEYS
        .iter()
        .filter(|key: &&Key| key.scope == scope)
        .map(|key| vec![code(key.name), prose(key.values.hint()), prose(key.summary)])
        .collect();

    table(&["Key", "Accepts", "What it does"], rows)
}

pub fn themes() -> String {
    terms(&["Theme", "Name", "What it is for"], vocabulary::themes())
}

pub fn transitions() -> String {
    terms(&["Transition", "Motion", "What it does"], vocabulary::transitions())
}

pub fn aspects() -> String {
    terms(&["Ratio", "Canvas", "What it is for"], vocabulary::aspects())
}

pub fn auto_steps() -> String {
    terms(&["Mode", "Effect", "What it stages"], vocabulary::auto_steps())
}

/// Effect presets, with what each one costs a compositor.
///
/// The cost column is not decoration: the motion rule flags a slide whose
/// effects will not stay on the compositor, and the preset a reader picks here
/// is what decides whether that fires.
pub fn step_presets() -> String {
    terms(&["Preset", "Phase", "What it does, and what it costs"], vocabulary::presets())
}

fn terms(headers: &[&str], terms: Vec<Term>) -> String {
    let rows = terms
        .into_iter()
        .map(|term| vec![code(&term.label), prose(&term.detail), prose(&term.documentation)])
        .collect();

    table(headers, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_frontmatter_key_the_language_server_knows_is_on_one_of_the_two_tables() {
        // The reader who cannot find `budget:` here concludes it does not
        // exist, so the split by scope has to be exhaustive rather than tidy.
        let both = format!("{}{}", deck_keys(), slide_keys());

        for key in vocabulary::KEYS {
            assert!(both.contains(key.name), "{} is documented nowhere", key.name);
        }
    }

    #[test]
    fn a_deck_key_and_a_slide_key_land_on_different_tables() {
        // `title:` in a later block silently does nothing, which is exactly the
        // mistake this separation exists to stop a reader making.
        assert!(deck_keys().contains("title"));
        assert!(!slide_keys().contains(">title<"));
        assert!(slide_keys().contains("budget"));
    }

    #[test]
    fn every_effect_preset_the_compiler_accepts_is_listed_with_its_cost() {
        let html = step_presets();

        for preset in slidx_core::EffectPreset::ALL {
            assert!(html.contains(preset.as_token()), "{} is undocumented", preset.as_token());
        }
        assert!(html.contains("compositor"));
    }

    #[test]
    fn every_built_in_theme_describes_itself() {
        let html = themes();

        for theme in slidx_theme::builtin::all() {
            assert!(html.contains(&theme.id), "{} is undocumented", theme.id);
            assert!(html.contains(&theme.description), "{} describes itself nowhere", theme.id);
        }
    }

    #[test]
    fn the_spelling_that_switches_automatic_staging_off_is_offered_too() {
        // `none` is not a mode and would fall out of a list derived from the
        // enum alone. Without it a slide cannot opt out of a deck-wide default.
        assert!(auto_steps().contains("<code>none</code>"));
    }

    #[test]
    fn a_description_written_in_markdown_arrives_as_markup() {
        assert!(transitions().contains("<code>prefers-reduced-motion</code>"));
    }
}
