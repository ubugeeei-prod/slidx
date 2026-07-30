//! # slidx dialect
//!
//! Whether a deck's dialect is well formed.
//!
//! ## Not the same question as the linter's
//!
//! `slidx_lint` asks whether a slide **survives a projector**: is the contrast
//! there after washout, is the type big enough from row fifteen, does the
//! content fit. Every one of its findings is about a room.
//!
//! This asks whether the deck **says something slidx can carry out**: does
//! `20 minutes` parse as a duration, does `editoral` name a theme, does a
//! `steps:` entry address a mark that exists. Those are not room problems. They
//! are the deck asking for something that will not happen.
//!
//! The two are kept apart because the failures they represent are different
//! sizes. A contrast warning is advice. A `steps:` entry pointing at nothing is
//! an animation that will simply not play, on stage, with nothing said — so an
//! author who has turned the room rules down to concentrate on writing must not
//! lose these at the same time. Every code here is under `dialect/`, and a group
//! name suppresses everything below it, so `--allow dialect` and
//! `--allow contrast` are independent decisions.
//!
//! ## What is checked, and why each one is silent today
//!
//! The parser's job is to never fail, so a value it cannot use it *drops*. That
//! is right for the parser and it is why every check here exists: the dropped
//! value is not reported anywhere.
//!
//! | Code | Written | Happens today |
//! |---|---|---|
//! | `dialect/invalid-duration` | `duration: fast` | no countdown, no warning |
//! | `dialect/invalid-aspect` | `aspect: 16` | falls back to 16:9 silently |
//! | `dialect/unknown-theme` | `theme: editoral` | falls back to `minimal` |
//! | `dialect/unknown-transition` | `transition: cube` | an instant cut |
//! //! | `dialect/unknown-target` | `reveal: "#hreo"` | the step never plays |
//!
//! Nothing here re-reports what the parser already says. `aspect: 16x9` is a
//! string that is not a ratio and the parser calls that `deck/unknown-aspect`;
//! `aspect: 16` is a number, which no reader ever sees, and that is this crate's.
//! A duplicated mark key is `mark/ambiguous-key`, already. Two codes for one
//! mistake would be two answers, and the author would fix it twice.
//!
//! ## Where the vocabularies come from
//!
//! `slidx_theme::builtin`, `Transition::ALL`, `Layout::ALL`,
//! `AspectRatio::parse`, and `frontmatter::parse_duration` — the code that
//! decides, never a list restated here. A checker with a vocabulary of its own
//! would report a typo on the day somebody adds a transition.
//!
//! One vocabulary is not the code's to know. A theme package adds a name this
//! crate cannot work out from anything it can reach, so the caller that read
//! the project hands it over in [`Installed`]. Without it, `theme: workshop`
//! would be reported as a typo on every build of a deck that had installed
//! exactly the theme it named.
//!
//! ```
//! use slidx_core::{parse_deck, DeckParseOptions};
//! use slidx_dialect::Installed;
//!
//! let deck = parse_deck("---\ntheme: editoral\n---\n\n# Hello\n", &DeckParseOptions::default());
//! let found = slidx_dialect::check(&deck, &[], &Installed::default());
//!
//! assert_eq!(found.as_slice()[0].code, "dialect/unknown-theme");
//! assert!(!found.has_blocking(), "a typo must not stop a deck rendering");
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod names;
pub mod shape;
pub mod targets;

use slidx_core::{Deck, Diagnostics};

/// What a project adds to the vocabulary this crate can work out on its own.
///
/// A theme package's id is a name no amount of reading the deck reveals: it is
/// on disk, in a dependency, and only a caller with a filesystem has seen it. A
/// checker that guessed would report `theme: workshop` as a typo on the build
/// of a deck that had installed exactly that theme.
///
/// One field and a struct rather than a bare slice, deliberately. The next
/// vocabulary a package extends — a layout it declares — would otherwise be a
/// second `&[String]` beside the first, in a call that already takes one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    /// Theme ids that resolve because a package provides them.
    pub themes: Vec<String>,
}

/// A check: read the deck, push diagnostics.
///
/// The same shape `slidx_lint::rules::RuleFn` has, and for the same reason — the
/// engine is a list rather than a framework.
type CheckFn = fn(&Deck, &Installed, &mut Diagnostics);

/// Every check, in the order their findings are reported.
///
/// Ordered by how far from the author's intent the mistake is: a value that does
/// not parse first, then a name that resolves to nothing, then a reference to
/// something that is not there.
///
/// Unnamed, unlike `slidx_lint::rules::ALL`. There a rule's name *is* the group
/// its codes fall under, so the list is worth publishing; here every code is
/// under `dialect/` and these three are only a way of splitting one file into
/// three. A public list of names nothing could act on would be a second
/// vocabulary to keep true.
const ALL: &[CheckFn] = &[shape::check, names::check, targets::check];

/// Runs every check and returns the findings that survive suppression.
///
/// `allow` holds codes and group names, and what a group name means is
/// [`slidx_core::Diagnostic::is_suppressed_by`]'s to say — the same rule the
/// visual linter applies, so `--allow` means one thing across both.
pub fn check(deck: &Deck, allow: &[String], installed: &Installed) -> Diagnostics {
    let mut sink = Diagnostics::default();

    for run in ALL {
        run(deck, installed, &mut sink);
    }

    if allow.is_empty() {
        return sink;
    }

    sink.into_iter().filter(|diagnostic| !diagnostic.is_suppressed_by(allow)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn deck(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    const BROKEN: &str = "---\nduration: fast\ntheme: editoral\ntransition: cube\n---\n\n# One\n";

    #[test]
    fn a_deck_that_says_only_things_slidx_can_carry_out_produces_nothing() {
        let clean = "---\nduration: 20m\ntheme: minimal\naspect: \"16:9\"\n\
                     transition: fade\n---\n\n# One\n\n---\nlayout: split\nbudget: 90s\n---\n\n\
                     ## Two\n\nThe [result]{#result}.\n";

        assert!(
            check(&deck(clean), &[], &Installed::default()).is_empty(),
            "{:?}",
            check(&deck(clean), &[], &Installed::default())
        );
    }

    #[test]
    fn the_group_name_suppresses_every_check_at_once() {
        // The reason these are their own group: an author concentrating on
        // writing turns the room rules down, and must not lose an animation that
        // will silently never play at the same time.
        assert!(!check(&deck(BROKEN), &[], &Installed::default()).is_empty());
        assert!(check(&deck(BROKEN), &["dialect".to_string()], &Installed::default()).is_empty());
    }

    #[test]
    fn one_code_can_be_suppressed_without_the_others() {
        let allow = vec!["dialect/unknown-theme".to_string()];
        let found = check(&deck(BROKEN), &allow, &Installed::default());

        assert!(found.iter().all(|d| d.code != "dialect/unknown-theme"));
        assert!(found.iter().any(|d| d.code == "dialect/unknown-transition"));
    }

    #[test]
    fn every_code_belongs_to_the_group_that_suppresses_it() {
        // A code outside `dialect/` would survive `--allow dialect` and there
        // would be no way to switch it off short of naming it.
        for diagnostic in check(&deck(BROKEN), &[], &Installed::default()).iter() {
            assert!(
                diagnostic.code.starts_with("dialect/"),
                "`{}` is not under `dialect/`",
                diagnostic.code
            );
        }
    }

    #[test]
    fn no_finding_here_stops_a_deck_rendering() {
        // Decks get edited minutes before a talk. Every one of these is a
        // warning: the slide still goes up, without the animation.
        assert!(!check(&deck(BROKEN), &[], &Installed::default()).has_blocking());
    }

    #[test]
    fn every_finding_says_what_to_do_about_it() {
        // A diagnostic an author cannot act on is noise, and these are all
        // typos — so there is always an obvious next action.
        for diagnostic in check(&deck(BROKEN), &[], &Installed::default()).iter() {
            assert!(diagnostic.help.is_some(), "`{}` says nothing to do", diagnostic.code);
        }
    }
}
