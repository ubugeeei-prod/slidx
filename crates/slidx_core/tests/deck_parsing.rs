//! Contract for turning Markdown sources into a deck model.

use slidx_core::{parse_deck, DeckParseOptions, StepAction, Visibility};

fn parse(source: &str) -> slidx_core::Deck {
    parse_deck(source, &DeckParseOptions::default())
}

#[test]
fn deck_without_frontmatter_is_a_single_slide() {
    let deck = parse("# Hello\n\nBody text.");

    assert_eq!(deck.slides.len(), 1);
    assert_eq!(deck.slides[0].content, "# Hello\n\nBody text.");
    assert!(deck.meta.title.is_none());
}

#[test]
fn deck_frontmatter_is_lifted_off_the_first_slide() {
    let deck = parse("---\ntitle: Launch Plan\ntheme: editorial\n---\n\n# Hello\n");

    assert_eq!(deck.meta.title.as_deref(), Some("Launch Plan"));
    assert_eq!(deck.meta.theme.as_deref(), Some("editorial"));
    assert_eq!(deck.slides[0].content, "# Hello");
}

#[test]
fn separator_splits_slides() {
    let deck = parse("# One\n\n---\n\n# Two\n\n---\n\n# Three\n");

    assert_eq!(deck.slides.len(), 3);
    assert_eq!(deck.slides[2].content, "# Three");
}

#[test]
fn a_separator_inside_a_fenced_code_block_is_not_a_slide_break() {
    // Horizontal rules are common inside shell transcripts and diff snippets.
    let deck = parse("# One\n\n```sh\n---\n```\n\n---\n\n# Two\n");

    assert_eq!(deck.slides.len(), 2, "only the fence-external separator splits");
    assert!(deck.slides[0].content.contains("```sh\n---\n```"));
}

#[test]
fn per_slide_frontmatter_follows_the_separator() {
    let deck = parse("# One\n\n---\nlayout: split\naccent: \"#334155\"\n---\n\n# Two\n");

    assert_eq!(deck.slides.len(), 2);
    assert_eq!(deck.slides[1].layout.as_deref(), Some("split"));
    assert_eq!(deck.slides[1].content, "# Two");
}

#[test]
fn speaker_notes_are_extracted_out_of_the_body() {
    let deck = parse("# One\n\n<!-- notes:\nOpen with the outcome.\nMention the fallback.\n-->\n");

    assert_eq!(deck.slides[0].notes.len(), 1);
    assert!(deck.slides[0].notes[0].contains("Open with the outcome."));
    assert!(!deck.slides[0].content.contains("notes:"), "notes never reach the public body");
}

#[test]
fn slide_ids_are_stable_and_url_safe() {
    let deck = parse("# Getting Started\n\n---\n\n# Why Rust?\n");

    assert_eq!(deck.slides[0].id, "getting-started");
    assert_eq!(deck.slides[1].id, "why-rust");
}

#[test]
fn duplicate_headings_get_disambiguated_ids() {
    let deck = parse("# Demo\n\n---\n\n# Demo\n");

    assert_eq!(deck.slides[0].id, "demo");
    assert_eq!(deck.slides[1].id, "demo-2");
}

#[test]
fn slides_without_a_heading_fall_back_to_their_index() {
    let deck = parse("Just a paragraph.\n");
    assert_eq!(deck.slides[0].id, "slide-1");
}

#[test]
fn non_ascii_headings_produce_usable_ids() {
    let deck = parse("# はじめに\n");
    assert!(!deck.slides[0].id.is_empty());
    assert!(!deck.slides[0].id.contains(' '));
}

#[test]
fn steps_frontmatter_compiles_into_a_timeline() {
    let deck = parse(
        "---\nsteps:\n  - reveal: \".point-1\"\n  - reveal: \".point-2\"\n---\n\n# Stepped\n",
    );

    let slide = &deck.slides[0];
    assert_eq!(slide.timeline.len(), 3);
    assert_eq!(slide.timeline.frame(1).unwrap().visibility(".point-1"), Some(Visibility::Visible));
}

#[test]
fn inline_step_markers_are_a_shorthand_for_reveal_actions() {
    // `<!-- step -->` after a block is the low-ceremony way to stage a slide
    // without leaving the prose.
    let deck = parse("# Stepped\n\n- one <!-- step -->\n- two <!-- step -->\n");

    let slide = &deck.slides[0];
    assert_eq!(slide.timeline.len(), 3, "two staged blocks plus the resting frame");
    assert!(!slide.content.contains("<!-- step -->"), "markers are consumed by the compiler");
}

#[test]
fn auto_steps_stage_list_items_without_any_markup() {
    let deck = parse("---\nautoSteps: list\n---\n\n# Agenda\n\n- one\n- two\n- three\n");

    assert_eq!(deck.slides[0].timeline.len(), 4);
}

#[test]
fn transitions_default_to_the_deck_setting_and_can_be_overridden_per_slide() {
    let deck =
        parse("---\ntransition: fade\n---\n\n# One\n\n---\ntransition: slide-left\n---\n\n# Two\n");

    assert_eq!(deck.meta.transition.as_deref(), Some("fade"));
    assert_eq!(deck.slides[0].transition.as_deref(), Some("fade"));
    assert_eq!(deck.slides[1].transition.as_deref(), Some("slide-left"));
}

#[test]
fn malformed_frontmatter_degrades_instead_of_failing_the_build() {
    // A broken deck mid-rehearsal should still render; the diagnostic is
    // surfaced separately rather than as a hard parse error.
    let deck = parse("---\ntitle: [unclosed\n---\n\n# Still Here\n");

    assert_eq!(deck.slides.len(), 1);
    assert!(deck.slides[0].content.contains("# Still Here"));
    assert!(!deck.diagnostics.is_empty(), "the failure is reported, not swallowed");
}

#[test]
fn step_actions_round_trip_through_frontmatter() {
    let deck = parse(
        "---\nsteps:\n  - reveal: \".a\"\n  - emphasize: { target: \".a\", preset: pulse }\n  - hide: \".a\"\n---\n\n# Round trip\n",
    );

    let actions = &deck.slides[0].steps.actions;
    assert_eq!(actions.len(), 3);
    assert!(matches!(actions[0], StepAction::Reveal { .. }));
    assert!(matches!(actions[1], StepAction::Emphasize { .. }));
    assert!(matches!(actions[2], StepAction::Hide { .. }));
}
