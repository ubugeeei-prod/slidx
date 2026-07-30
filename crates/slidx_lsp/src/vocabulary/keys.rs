//! Every frontmatter key slidx reads.
//!
//! Split from the value vocabularies next door because it is a different kind of
//! list. A value set is *derived*: `Transition::ALL` already exists, and a copy
//! of it here would be a copy that goes wrong silently. This table cannot be
//! derived at all, because frontmatter is deliberately open — a theme or a plugin
//! may read a key slidx has never heard of, and the editor keeps it rather than
//! dropping it. So what holds this table honest is the tests below, each of which
//! pins a documented key to the parser behaviour it claims.

use super::{Term, Values};

/// Where a key means anything.
///
/// The first block in a file is both the deck's configuration and the first
/// slide's, so everything is offered there; a later block configures one slide
/// and `title:` in it would silently do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Deck,
    Slide,
}

/// One frontmatter key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub name: &'static str,
    pub scope: Scope,
    pub summary: &'static str,
    pub values: Values,
}

impl Key {
    /// True when this key belongs in a block that may or may not be the deck's.
    pub fn applies(&self, is_deck_block: bool) -> bool {
        is_deck_block || self.scope == Scope::Slide
    }

    pub fn as_term(&self) -> Term {
        Term::new(self.name, self.values.hint(), self.summary)
    }
}

/// Every frontmatter key slidx reads.
///
/// Unlike the value vocabularies there is no enum upstream to derive this
/// from: frontmatter is deliberately open, so that a theme or a plugin can
/// read a key this crate has never heard of and the editor keeps it. The
/// tests below pin each documented key to the parser behaviour it claims.
pub const KEYS: &[Key] = &[
    Key {
        name: "title",
        scope: Scope::Deck,
        summary: "The talk's title. Used on the title slide, the social card, and everywhere \
                  the deck is quoted.",
        values: Values::Text("the talk's title"),
    },
    Key {
        name: "description",
        scope: Scope::Deck,
        summary: "One or two sentences. Becomes the published description and the social card \
                  text.",
        values: Values::Text("a sentence or two"),
    },
    Key {
        name: "author",
        scope: Scope::Deck,
        summary: "Who is giving the talk.",
        values: Values::Text("your name"),
    },
    Key {
        name: "lang",
        scope: Scope::Deck,
        summary: "BCP 47 tag for the language the slides are written in. Becomes the page's \
                  `<html lang>`, which is what a screen reader picks a voice from.",
        values: Values::Text("ja"),
    },
    Key {
        name: "theme",
        scope: Scope::Deck,
        summary: "A built-in theme id, or the name of a theme package. An unknown name is \
                  reported rather than silently falling back.",
        values: Values::Themes,
    },
    Key {
        name: "aspect",
        scope: Scope::Deck,
        summary: "Slide geometry. Getting this wrong is how slides arrive cropped on stage.",
        values: Values::Aspects,
    },
    Key {
        name: "duration",
        scope: Scope::Deck,
        summary: "How long the speaking slot is. Drives the presenter countdown and the check \
                  that catches a 40-minute deck booked into a 20-minute slot.",
        values: Values::Duration,
    },
    Key {
        name: "translationOf",
        scope: Scope::Deck,
        summary: "The deck this one is a translation of. Written by `slidx i18n apply`, and what \
                  makes two decks knowably the same talk rather than two talks.",
        values: Values::Text("../slides"),
    },
    Key {
        name: "event",
        scope: Scope::Deck,
        summary: "The conference or meetup this deck is for.",
        values: Values::Text("the event name"),
    },
    Key {
        name: "date",
        scope: Scope::Deck,
        summary: "ISO-8601 date of the talk, kept as text so a deck never depends on a clock.",
        values: Values::Text("YYYY-MM-DD"),
    },
    Key {
        name: "venue",
        scope: Scope::Deck,
        summary: "Where the talk is given.",
        values: Values::Text("the venue"),
    },
    Key {
        name: "hashtag",
        scope: Scope::Deck,
        summary: "Event hashtag. A leading `#` is stripped, so either spelling works.",
        values: Values::Text("the event hashtag"),
    },
    Key {
        name: "url",
        scope: Scope::Deck,
        summary: "Canonical URL of the published deck.",
        values: Values::Text("https://…"),
    },
    Key {
        name: "repo",
        scope: Scope::Deck,
        summary: "Repository shown on the closing slide and in the resources page.",
        values: Values::Text("https://…"),
    },
    Key {
        name: "transition",
        scope: Scope::Slide,
        summary: "How this slide arrives. A slide that names one decides for itself, including \
                  when it says `none`; only silence inherits the deck's.",
        values: Values::Transitions,
    },
    Key {
        name: "layout",
        scope: Scope::Slide,
        summary: "The named set of regions this slide's blocks are placed into. A block picks \
                  one with a class on a line of its own, such as `{.side}`.",
        values: Values::Layouts,
    },
    Key {
        name: "budget",
        scope: Scope::Slide,
        summary: "How long this slide is budgeted. Summed across the deck and checked against \
                  the slot length.",
        values: Values::Duration,
    },
    Key {
        name: "optional",
        scope: Scope::Slide,
        summary: "Safe to skip when running behind. Presenter view marks these.",
        values: Values::Boolean,
    },
    Key {
        name: "autoSteps",
        scope: Scope::Slide,
        summary: "Stages the slide from its own structure, without touching the prose.",
        values: Values::AutoSteps,
    },
    Key {
        name: "steps",
        scope: Scope::Slide,
        summary: "The slide's step pipeline, written out. Takes precedence over step markers \
                  in the body.",
        values: Values::Steps,
    },
];

/// The keys offered in a block, in the order they are offered.
pub fn keys_for(is_deck_block: bool) -> Vec<&'static Key> {
    KEYS.iter().filter(|key| key.applies(is_deck_block)).collect()
}

/// Looks up a key by the name an author wrote, in either spelling.
///
/// The parser accepts `autoSteps` and `auto-steps` alike, so hover has to
/// recognise both or it goes blank on the spelling half of them use.
pub fn key(name: &str) -> Option<&'static Key> {
    KEYS.iter().find(|key| key.name == name || kebab_case(key.name) == name)
}

fn kebab_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    for character in name.chars() {
        if character.is_ascii_uppercase() {
            out.push('-');
            out.push(character.to_ascii_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, AspectRatio, AutoSteps};

    fn parse(source: &str) -> slidx_core::Deck {
        parse_deck(source, &slidx_core::DeckParseOptions::default())
    }

    #[test]
    fn deck_only_keys_are_not_offered_inside_a_slide_block() {
        // `title:` on slide four does nothing, and offering it there is how an
        // author learns the wrong thing.
        let slide: Vec<&str> = keys_for(false).iter().map(|key| key.name).collect();

        assert!(!slide.contains(&"title"));
        assert!(slide.contains(&"layout"));
        assert!(slide.contains(&"transition"));
    }

    #[test]
    fn the_first_block_offers_deck_and_slide_keys_alike() {
        // It is both, which is why `transition:` up there sets the default and
        // `budget:` up there budgets slide one.
        let deck: Vec<&str> = keys_for(true).iter().map(|key| key.name).collect();

        assert!(deck.contains(&"title"));
        assert!(deck.contains(&"budget"));
    }

    #[test]
    fn a_key_is_found_under_either_spelling_the_parser_accepts() {
        assert_eq!(key("autoSteps").map(|key| key.name), Some("autoSteps"));
        assert_eq!(key("auto-steps").map(|key| key.name), Some("autoSteps"));
        assert!(key("autosteps").is_none(), "and not one it does not");
    }

    #[test]
    fn every_documented_key_is_actually_read_by_the_parser() {
        // Pins the table to behaviour. A key documented here that the parser
        // ignores is a promise the editor cannot keep.
        let source = "---\ntitle: T\ndescription: D\nauthor: A\ntheme: terminal\naspect: \"4:3\"\n\
                      duration: 25m\nevent: E\ndate: 2026-07-29\nvenue: V\nhashtag: \"#h\"\n\
                      url: https://example.com\nrepo: https://example.com/r\ntransition: fade\n\
                      layout: split\nbudget: 90s\noptional: true\nautoSteps: list\n---\n\n- one\n";
        let deck = parse(source);
        let slide = &deck.slides[0];

        assert!(deck.diagnostics.is_empty(), "{:?}", deck.diagnostics);
        assert_eq!(deck.meta.title.as_deref(), Some("T"));
        assert_eq!(deck.meta.description.as_deref(), Some("D"));
        assert_eq!(deck.meta.author.as_deref(), Some("A"));
        assert_eq!(deck.meta.theme.as_deref(), Some("terminal"));
        assert_eq!(deck.meta.aspect, AspectRatio::Classic);
        assert_eq!(deck.meta.duration_seconds, Some(1500));
        assert_eq!(deck.meta.talk.event.as_deref(), Some("E"));
        assert_eq!(deck.meta.talk.date.as_deref(), Some("2026-07-29"));
        assert_eq!(deck.meta.talk.venue.as_deref(), Some("V"));
        assert_eq!(deck.meta.talk.hashtag.as_deref(), Some("h"));
        assert_eq!(deck.meta.talk.url.as_deref(), Some("https://example.com"));
        assert_eq!(deck.meta.talk.repo.as_deref(), Some("https://example.com/r"));
        assert_eq!(slide.transition.as_deref(), Some("fade"));
        assert_eq!(slide.layout.as_deref(), Some("split"));
        assert_eq!(slide.budget_seconds, Some(90));
        assert!(slide.optional);
        assert_eq!(slide.steps.auto, Some(AutoSteps::List));
    }

    #[test]
    fn the_steps_key_is_read_as_a_pipeline() {
        let deck = parse("---\nsteps:\n  - reveal: \".x\"\n---\n\n# One\n");

        assert_eq!(deck.slides[0].steps.actions.len(), 1);
        assert!(key("steps").is_some_and(|key| key.values == Values::Steps));
    }

    #[test]
    fn every_key_says_what_it_expects() {
        for key in KEYS {
            assert!(!key.summary.is_empty(), "{} says nothing", key.name);
            assert!(!key.values.hint().is_empty(), "{} hints nothing", key.name);
        }
    }
}
