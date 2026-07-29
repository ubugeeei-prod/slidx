//! What an author is allowed to write, derived from the code that decides it.
//!
//! # The rule
//!
//! Every closed set slidx has — themes, transitions, effect presets, auto-step
//! modes, aspect ratios — is already defined in exactly one place in Rust.
//! This module reads those definitions. It never restates them.
//!
//! A list of preset names typed out in a language server is a list that is
//! true on the day it is written and wrong on the day someone adds a preset,
//! and the failure is silent: completion simply stops offering the new one,
//! and no test anywhere notices. So the names come from `EffectPreset::ALL`,
//! `Transition::ALL`, `AutoSteps::ALL`, `AspectRatio::ALL`, and
//! [`slidx_theme::builtin::all`].
//!
//! Prose is the part that cannot be derived, and it is attached with an
//! exhaustive `match` on the enum rather than a lookup table. That makes the
//! compiler the thing that notices: adding a variant upstream fails this
//! crate's build until someone writes the sentence describing it.
//!
//! Theme descriptions are not written here at all — a theme already describes
//! itself, and that description is what the author sees.

use slidx_core::{AspectRatio, AutoSteps, EffectPreset};
use slidx_theme::{builtin, Transition};

/// One thing an author may write, with the prose an editor shows beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub label: String,
    /// Short annotation, shown to the right of the label in a completion list.
    pub detail: String,
    /// Markdown, shown on hover and in the expanded completion item.
    pub documentation: String,
}

impl Term {
    fn new(label: impl Into<String>, detail: impl Into<String>, doc: impl Into<String>) -> Self {
        Self { label: label.into(), detail: detail.into(), documentation: doc.into() }
    }
}

/// Finds the term describing a written token.
pub fn find<'a>(terms: &'a [Term], token: &str) -> Option<&'a Term> {
    terms.iter().find(|term| term.label == token)
}

/// The built-in themes, described in their own words.
pub fn themes() -> Vec<Term> {
    builtin::all()
        .into_iter()
        .map(|theme| Term::new(theme.id, theme.name, theme.description))
        .collect()
}

/// Slide-to-slide transitions.
pub fn transitions() -> Vec<Term> {
    Transition::ALL
        .into_iter()
        .map(|transition| {
            let motion = if transition.moves() {
                "cancelled under `prefers-reduced-motion`"
            } else {
                "safe under `prefers-reduced-motion`"
            };

            Term::new(transition.as_token(), motion, describe_transition(transition))
        })
        .collect()
}

fn describe_transition(transition: Transition) -> &'static str {
    match transition {
        Transition::None => "An instant cut. No animation is emitted at all.",
        Transition::Fade => "The slides cross-fade.",
        Transition::Slide => {
            "The arriving slide slides in over a stationary one, like a card dealt on top of \
             the deck."
        }
        Transition::Push => "Both slides move in lockstep, the arriving one pushing the other off.",
    }
}

/// Effect presets, as `steps:` and `<!-- step: … -->` accept them.
pub fn presets() -> Vec<Term> {
    EffectPreset::ALL
        .into_iter()
        .map(|preset| {
            let phase = format!("{:?}", preset.kind()).to_lowercase();
            let cost = if preset.is_compositor_only() {
                "stays on the compositor"
            } else {
                "repaints — the linter flags it on a heavy slide"
            };

            Term::new(preset.as_token(), phase, format!("{}\n\n{cost}.", describe_preset(preset)))
        })
        .collect()
}

fn describe_preset(preset: EffectPreset) -> &'static str {
    match preset {
        EffectPreset::None => "No motion at all. Respected verbatim when a deck opts out.",
        EffectPreset::Fade => "Fades in. The default entrance.",
        EffectPreset::FlyIn => "Travels in from an edge. Takes an `origin`.",
        EffectPreset::Wipe => "Revealed behind a moving edge.",
        EffectPreset::Zoom => "Scales up into place.",
        EffectPreset::Split => "Opens from the centre outwards.",
        EffectPreset::Grow => "Grows from nothing.",
        EffectPreset::Float => "Drifts up into place.",
        EffectPreset::Typewriter => "Types the text out character by character.",
        EffectPreset::Draw => "Draws a stroke along its path.",
        EffectPreset::Pulse => "Swells once. The default emphasis.",
        EffectPreset::Shake => "Shakes side to side.",
        EffectPreset::Spin => "Turns once about its centre.",
        EffectPreset::ColorPulse => "Flashes to the accent colour and back.",
        EffectPreset::Underline => "Draws an underline beneath the text.",
        EffectPreset::FadeOut => "Fades out. The default exit.",
        EffectPreset::FlyOut => "Travels off towards an edge. Takes an `origin`.",
        EffectPreset::WipeOut => "Hidden behind a moving edge.",
        EffectPreset::ZoomOut => "Scales away.",
        EffectPreset::Shrink => "Shrinks to nothing.",
    }
}

/// Automatic staging modes, plus the spelling that switches one off.
pub fn auto_steps() -> Vec<Term> {
    let mut terms: Vec<Term> = AutoSteps::ALL
        .into_iter()
        .map(|mode| Term::new(mode.as_token(), "staged automatically", describe_auto(mode)))
        .collect();

    // `none` is not a mode, it is the frontmatter spelling that switches a
    // deck-wide default off for one slide. Offering it matters more than most
    // of the modes: without it a slide cannot opt out at all.
    terms.push(Term::new(
        "none",
        "no automatic staging",
        "Switches off a deck-wide `autoSteps`, so this slide arrives whole.",
    ));
    terms
}

fn describe_auto(mode: AutoSteps) -> &'static str {
    match mode {
        AutoSteps::List => {
            "Reveals top-level list items one at a time. Nested items come with \
                            their parent."
        }
        AutoSteps::Block => {
            "Reveals every top-level block one at a time. A fenced block stays \
                             whole."
        }
        AutoSteps::Row => "Reveals table body rows one at a time. The header is not a stop.",
    }
}

/// Slide geometries.
pub fn aspects() -> Vec<Term> {
    AspectRatio::ALL
        .into_iter()
        .map(|aspect| {
            let (width, height) = aspect.dimensions();

            Term::new(aspect.as_token(), format!("{width}×{height}"), describe_aspect(aspect))
        })
        .collect()
}

fn describe_aspect(aspect: AspectRatio) -> &'static str {
    match aspect {
        AspectRatio::Wide => "Widescreen. The default, and what most venues project.",
        AspectRatio::Golden => "Slightly taller than widescreen. Common on laptop panels.",
        AspectRatio::Classic => "Four by three. Older halls and some fixed installations.",
    }
}

/// What a frontmatter key accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Values {
    /// Free text, with a hint at the shape it should take.
    Text(&'static str),
    Duration,
    Boolean,
    Themes,
    Transitions,
    AutoSteps,
    Aspects,
    /// A list of step actions, which is structure rather than a value.
    Steps,
}

impl Values {
    /// The closed set this key accepts, if it has one.
    pub fn terms(self) -> Option<Vec<Term>> {
        match self {
            Self::Themes => Some(themes()),
            Self::Transitions => Some(transitions()),
            Self::AutoSteps => Some(auto_steps()),
            Self::Aspects => Some(aspects()),
            Self::Text(_) | Self::Duration | Self::Boolean | Self::Steps => None,
        }
    }

    /// What to show when there is nothing to pick from a list.
    pub fn hint(self) -> &'static str {
        match self {
            Self::Text(hint) => hint,
            Self::Duration => "seconds, or `25m`, `90s`, `25:00`, `1h30m`",
            Self::Boolean => "`true` or `false`",
            Self::Themes => "a built-in theme id, or a theme package name",
            Self::Transitions => "a transition name",
            Self::AutoSteps => "an automatic staging mode",
            Self::Aspects => "an aspect ratio",
            Self::Steps => "a list of step actions",
        }
    }
}

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
        summary: "Layout name the theme resolves, such as `split` or `top`.",
        values: Values::Text("a layout name"),
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
    use slidx_core::{parse_deck, DeckParseOptions};

    fn parse(source: &str) -> slidx_core::Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    fn labels(terms: &[Term]) -> Vec<&str> {
        terms.iter().map(|term| term.label.as_str()).collect()
    }

    #[test]
    fn every_offered_theme_resolves_to_a_real_theme() {
        // The whole point of deriving: a theme added to `builtin::all` is
        // offered here without anyone editing this crate.
        for term in themes() {
            assert!(slidx_theme::resolve(&term.label).is_some(), "{} does not resolve", term.label);
        }
        assert!(labels(&themes()).contains(&"minimal"));
    }

    #[test]
    fn a_theme_is_described_in_its_own_words() {
        let minimal = find(&themes(), "minimal").unwrap().clone();

        assert_eq!(minimal.detail, builtin::minimal().name);
        assert_eq!(minimal.documentation, builtin::minimal().description);
    }

    #[test]
    fn every_offered_transition_is_one_the_theme_layer_parses() {
        for term in transitions() {
            assert!(Transition::parse(&term.label).is_some(), "{} is not a transition", term.label);
        }
        assert_eq!(transitions().len(), Transition::ALL.len());
    }

    #[test]
    fn a_transition_that_moves_the_whole_slide_says_so() {
        assert!(find(&transitions(), "push").unwrap().detail.contains("cancelled"));
        assert!(find(&transitions(), "fade").unwrap().detail.contains("safe"));
    }

    #[test]
    fn every_offered_preset_is_understood_by_a_step_marker() {
        // An editor offering a name the parser shrugs at costs the author an
        // animation and says nothing about why.
        for preset in EffectPreset::ALL {
            let deck = parse(&format!("- one <!-- step: {} -->\n", preset.as_token()));
            let action = &deck.slides[0].steps.actions[0];

            assert_eq!(action.options().preset, Some(preset), "{}", preset.as_token());
        }
        assert_eq!(presets().len(), EffectPreset::ALL.len());
    }

    #[test]
    fn a_preset_that_repaints_is_flagged_where_the_author_picks_it() {
        // The linter says the same thing later. Saying it at the point of
        // choice is the difference between a warning and a decision.
        assert!(find(&presets(), "typewriter").unwrap().documentation.contains("repaints"));
        assert!(find(&presets(), "fade").unwrap().documentation.contains("compositor"));
    }

    #[test]
    fn a_preset_is_labelled_with_the_phase_it_belongs_to() {
        assert_eq!(find(&presets(), "fade").unwrap().detail, "entrance");
        assert_eq!(find(&presets(), "pulse").unwrap().detail, "emphasis");
        assert_eq!(find(&presets(), "fade-out").unwrap().detail, "exit");
    }

    #[test]
    fn every_offered_auto_steps_mode_is_accepted_without_complaint() {
        for term in auto_steps() {
            let deck = parse(&format!("---\nautoSteps: {}\n---\n\n- one\n", term.label));

            assert!(
                deck.diagnostics.iter().all(|d| d.code != "slide/unknown-auto-steps"),
                "{} was rejected",
                term.label
            );
        }
    }

    #[test]
    fn switching_auto_steps_off_is_offered_alongside_the_modes() {
        // Without `none` a slide cannot opt out of a deck-wide default.
        assert!(labels(&auto_steps()).contains(&"none"));
        assert_eq!(auto_steps().len(), AutoSteps::ALL.len() + 1);
    }

    #[test]
    fn every_offered_aspect_is_accepted_without_complaint() {
        for term in aspects() {
            let deck = parse(&format!("---\naspect: \"{}\"\n---\n\n# One\n", term.label));

            assert!(
                deck.diagnostics.iter().all(|d| d.code != "deck/unknown-aspect"),
                "{} was rejected",
                term.label
            );
            assert_eq!(deck.meta.aspect.as_token(), term.label);
        }
    }

    #[test]
    fn an_aspect_is_annotated_with_the_canvas_it_renders_at() {
        assert_eq!(find(&aspects(), "16:9").unwrap().detail, "1920×1080");
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
