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
//! `Transition::ALL`, `AutoSteps::ALL`, `AspectRatio::ALL`,
//! [`slidx_theme::builtin::all`], and [`slidx_theme::layout::all`].
//!
//! Prose is the part that cannot be derived, and it is attached with an
//! exhaustive `match` on the enum rather than a lookup table. That makes the
//! compiler the thing that notices: adding a variant upstream fails this
//! crate's build until someone writes the sentence describing it.
//!
//! Theme descriptions are not written here at all — a theme already describes
//! itself, and that description is what the author sees.

use slidx_core::{AspectRatio, AutoSteps, EffectPreset};
use slidx_theme::{builtin, layout, Transition};

mod keys;

pub use keys::{key, keys_for, Key, Scope, KEYS};

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
        Transition::Wipe => {
            "The arriving slide is revealed behind a moving edge. The outgoing slide stays put."
        }
        Transition::Rise => "Both slides move vertically, the arriving one rising from below.",
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

/// The built-in layouts, described by the regions they offer.
///
/// The region names are the documentation, because they are what an author has
/// to write next: picking `aside` and then guessing at `.sidebar` is the mistake
/// this list exists to prevent.
pub fn layouts() -> Vec<Term> {
    layout::all()
        .into_iter()
        .map(|layout| {
            let regions = layout
                .regions
                .iter()
                .map(|region| format!("- `{{.{}}}` — {}", region.name, region.summary))
                .collect::<Vec<_>>()
                .join("\n");

            let detail = layout.region_names().join(", ");
            let documentation = format!(
                "{}\n\nA block picks a region with a class on a line of its own; one that names \
                 none goes to `{}`.\n\n{regions}",
                layout.summary, layout.default_region,
            );

            Term::new(layout.id, detail, documentation)
        })
        .collect()
}

/// Every region name any built-in layout uses.
///
/// Offered flat rather than per layout because completion happens while the key
/// is being typed, and the slide's `layout:` may be a line that is not written
/// yet. A name this slide's layout does not have is reported by
/// `slidx_theme::layout`, which is where the pairing is actually known.
pub fn regions() -> Vec<Term> {
    let layouts = layout::all();

    layout::REGION_NAMES
        .iter()
        .map(|name| {
            let offered: Vec<&str> = layouts
                .iter()
                .filter(|layout| layout.has_region(name))
                .map(|layout| layout.id.as_str())
                .collect();

            let summary = layouts
                .iter()
                .find_map(|layout| layout.region(name))
                .map_or_else(String::new, |region| region.summary.clone());

            Term::new(*name, format!("in {}", offered.join(", ")), summary)
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
    Layouts,
    Regions,
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
            Self::Layouts => Some(layouts()),
            Self::Regions => Some(regions()),
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
            Self::Layouts => "a layout name",
            Self::Regions => "a region of this slide's layout",
            Self::Steps => "a list of step actions",
        }
    }
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
    fn every_offered_layout_resolves_to_a_real_one() {
        // `layout:` used to complete as free text, so a name nobody implemented
        // looked exactly like one that worked. That is the whole reason this key
        // has a closed set now.
        for term in layouts() {
            assert!(layout::find(&term.label).is_some(), "{} does not resolve", term.label);
        }
        assert_eq!(layouts().len(), layout::all().len());
    }

    #[test]
    fn a_layout_is_annotated_with_the_regions_a_block_can_choose() {
        // Picking `aside` and then guessing at `.sidebar` is the mistake the
        // annotation exists to prevent, so the names have to be in the list
        // itself rather than only in the expanded documentation.
        let offered = layouts();
        let term = find(&offered, "aside").unwrap();

        assert_eq!(term.detail, "main, side");
        assert!(term.documentation.contains("{.side}"), "got: {}", term.documentation);
    }
}
