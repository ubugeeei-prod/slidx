//! Names that resolve to nothing.
//!
//! `theme:` and `transition:` are closed vocabularies, and each one already has
//! exactly one definition in Rust — `slidx_theme::builtin` and
//! `Transition::ALL`. What neither had was a reader that reported a name outside
//! the set:
//!
//! - `slidx_theme::resolve` returns `None` for a typo and every caller falls
//!   back to `minimal`, so `theme: editoral` renders a deck that looks nearly
//!   right.
//! - the shell resolves a transition with `parse().unwrap_or_default()`, so
//!   `transition: cube` is an instant cut and the author assumes their browser
//!   does not support it.
//!
//! Both are silent, both are found on stage, and both are one character away
//! from correct. That is the whole reason this module exists.
//!
//! `layout:` reads like it belongs here and deliberately does not. The layout
//! vocabulary reports a name outside its own set as `layout/unknown`, from the
//! module that knows what a layout's regions are — which is more than a name
//! check could say. Two answers to one question is what this repository has a
//! rule against, so the better-placed one keeps it.
//!
//! # What a theme name is allowed to be
//!
//! The built-ins, plus whatever the project installed — which this crate cannot
//! see, so the caller hands it over in [`Installed`]. Both halves are needed:
//! without the packages, a deck that installed the theme it named is told it
//! made a typo; without the built-ins, the check has no vocabulary at all.
//!
//! An unknown theme stays a warning rather than an error. A deck naming a
//! package that is not installed on *this* machine is not wrong — a colleague's
//! checkout, a fresh CI runner — so the finding says what happened and names
//! both ways out.

use slidx_core::{frontmatter, Deck, Diagnostic, Diagnostics, SourceSpan};
use slidx_theme::{builtin, Transition};

use crate::Installed;

pub fn check(deck: &Deck, installed: &Installed, sink: &mut Diagnostics) {
    check_theme(deck, installed, sink);

    for slide in &deck.slides {
        let span = SourceSpan::line(slide.source_line).on_slide(slide.index);

        check_transition(written(slide, "transition"), span, sink);
    }
}

/// A token as this slide's own frontmatter spells it.
///
/// Deliberately not `Slide::transition`, which is the *resolved* value: a slide
/// that says nothing inherits the deck's, so a typo up top would be reported
/// again on every one of sixty slides. The question here is which slide the
/// author has to open to fix it.
///
/// A value that is not a string at all — `transition: false`, which YAML reads as
/// a boolean and the parser maps to `none` — is not this crate's to comment on.
fn written<'a>(slide: &'a slidx_core::Slide, key: &str) -> Option<&'a str> {
    frontmatter::field(&slide.frontmatter, key)?.as_str()
}

fn check_theme(deck: &Deck, installed: &Installed, sink: &mut Diagnostics) {
    let Some(name) = &deck.meta.theme else {
        return;
    };
    if slidx_theme::resolve(name).is_some() || installed.themes.iter().any(|id| id == name) {
        return;
    }

    let offered =
        builtin::all().into_iter().map(|theme| theme.id).chain(installed.themes.iter().cloned());

    sink.push(
        Diagnostic::warning("dialect/unknown-theme", format!("no theme called `{name}`"))
            .at(SourceSpan::default().on_slide(0))
            .with_help(format!(
                "use one of {}, or install the theme package that provides `{name}`",
                vocabulary(offered)
            )),
    );
}

/// Flags a transition the shell will silently turn into a cut.
fn check_transition(token: Option<&str>, span: SourceSpan, sink: &mut Diagnostics) {
    let Some(token) = token else {
        return;
    };
    if Transition::parse(token).is_some() {
        return;
    }

    sink.push(
        Diagnostic::warning(
            "dialect/unknown-transition",
            format!("no transition called `{token}`"),
        )
        .at(span)
        .with_help(format!(
            "use {}, and note that a deck gains motion by asking for it",
            slidx_theme::transition::vocabulary()
        )),
    );
}

/// A list of names, as help text. Derived from whatever it is given, so it
/// cannot drift from the vocabulary the check itself consulted.
fn vocabulary(names: impl Iterator<Item = String>) -> String {
    let quoted: Vec<String> = names.map(|name| format!("`{name}`")).collect();

    match quoted.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{}, or {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn found(source: &str) -> Diagnostics {
        let deck = parse_deck(source, &DeckParseOptions::default());
        let mut sink = Diagnostics::default();
        check(&deck, &Installed::default(), &mut sink);
        sink
    }

    fn codes(source: &str) -> Vec<String> {
        found(source).iter().map(|diagnostic| diagnostic.code.clone()).collect()
    }

    #[test]
    fn a_theme_nothing_answers_to_is_reported() {
        // One character from `editorial`, and today it renders as `minimal`
        // without a word.
        let sink = found("---\ntheme: editoral\n---\n\n# One\n");

        assert_eq!(sink.as_slice()[0].code, "dialect/unknown-theme");
        assert!(sink.as_slice()[0].message.contains("editoral"));
    }

    #[test]
    fn the_theme_help_names_every_built_in_and_the_way_out() {
        // A deck may legitimately name a theme package slidx has not loaded, so
        // the help offers that reading rather than calling it a typo.
        let help =
            found("---\ntheme: aurora\n---\n\n# One\n").as_slice()[0].help.clone().expect("help");

        for theme in builtin::all() {
            assert!(help.contains(&theme.id), "help omits `{}`", theme.id);
        }
        assert!(help.contains("theme package"), "{help}");
    }

    #[test]
    fn a_theme_the_project_installed_is_not_a_typo() {
        // The build this crate would otherwise have warned on every time: a
        // deck naming a theme, and the package providing exactly that theme
        // sitting in the project's dependencies.
        let deck = parse_deck("---\ntheme: workshop\n---\n\n# One\n", &DeckParseOptions::default());
        let installed = Installed { themes: vec!["workshop".to_string()] };
        let mut sink = Diagnostics::default();

        check(&deck, &installed, &mut sink);

        assert!(sink.is_empty(), "{sink:?}");
    }

    #[test]
    fn a_typo_near_an_installed_theme_is_offered_that_theme_too() {
        // The help lists what a name could have been, and a theme the author
        // has already installed is the likeliest answer of all.
        let deck =
            parse_deck("---\ntheme: workshopp\n---\n\n# One\n", &DeckParseOptions::default());
        let installed = Installed { themes: vec!["workshop".to_string()] };
        let mut sink = Diagnostics::default();

        check(&deck, &installed, &mut sink);
        let help = sink.as_slice()[0].help.clone().expect("help");

        assert!(help.contains("`workshop`"), "{help}");
    }

    #[test]
    fn every_built_in_theme_passes() {
        for theme in builtin::all() {
            assert!(
                codes(&format!("---\ntheme: {}\n---\n\n# One\n", theme.id)).is_empty(),
                "{} was rejected",
                theme.id
            );
        }
    }

    #[test]
    fn a_transition_nothing_answers_to_is_reported() {
        let sink = found("---\ntransition: cube\n---\n\n# One\n");

        assert_eq!(sink.as_slice()[0].code, "dialect/unknown-transition");
        assert!(sink.as_slice()[0].message.contains("cube"));
    }

    #[test]
    fn the_transition_help_names_every_transition_on_offer() {
        let help =
            found("---\ntransition: cube\n---\n\n# One\n").as_slice()[0].help.clone().unwrap();

        for transition in Transition::ALL {
            assert!(help.contains(transition.as_token()), "help omits `{}`", transition.as_token());
        }
    }

    #[test]
    fn every_transition_on_offer_passes() {
        for transition in Transition::ALL {
            let source = format!("---\ntransition: {}\n---\n\n# One\n", transition.as_token());
            assert!(codes(&source).is_empty(), "{} was rejected", transition.as_token());
        }
    }

    #[test]
    fn switching_a_transition_off_is_not_a_typo() {
        // `false` and `off` are what people write, and the parser normalises
        // both to `none`.
        for spelling in ["none", "off", "false"] {
            let source = format!("---\ntransition: {spelling}\n---\n\n# One\n");
            assert!(codes(&source).is_empty(), "`{spelling}` was rejected");
        }
    }

    #[test]
    fn a_typo_in_a_deck_transition_is_reported_once_not_once_per_slide() {
        // Every slide inherits the deck's transition. Reporting the inherited
        // copy would put the same finding on all sixty slides of a deck.
        let sink = found("---\ntransition: cube\n---\n\n# One\n\n---\n\n# Two\n\n---\n\n# Three\n");

        assert_eq!(sink.len(), 1, "{sink:?}");
        assert_eq!(sink.as_slice()[0].span.slide_index, Some(0));
    }

    #[test]
    fn a_typo_on_one_slide_is_reported_on_that_slide() {
        let sink = found("# One\n\n---\ntransition: disolve\n---\n\n# Two\n");

        assert_eq!(sink.len(), 1);
        assert_eq!(sink.as_slice()[0].span.slide_index, Some(1));
    }

    #[test]
    fn a_transition_that_is_not_a_name_is_left_to_the_parser() {
        // `transition: 3` is already `frontmatter/invalid-transition`, and
        // `transition: false` is a YAML boolean the parser reads as `none`.
        // Neither is this crate's to comment on.
        assert!(codes("---\ntransition: 3\n---\n\n# One\n").is_empty());
        assert!(codes("---\ntransition: false\n---\n\n# One\n").is_empty());
    }

    #[test]
    fn a_deck_that_names_nothing_is_reported_on_for_nothing() {
        assert!(codes("# One\n\n---\n\n# Two\n").is_empty());
    }

    #[test]
    fn a_list_of_one_name_reads_as_one_name() {
        assert_eq!(vocabulary(["a".to_string()].into_iter()), "`a`");
        assert_eq!(vocabulary(["a".to_string(), "b".to_string()].into_iter()), "`a`, or `b`");
    }
}
