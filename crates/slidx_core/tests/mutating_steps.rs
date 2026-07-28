//! Changing an element that is already on screen.
//!
//! Revealing covers "not there yet" and hiding covers "gone". The third thing
//! a presenter does is change something the audience is already looking at: a
//! number that updates, a label that turns red, a line of code that becomes
//! the focus. `<!-- step -->` cannot express any of those.
//!
//! Two forms, chosen by what is changing:
//!
//! - **Takes**, in the prose, for content: `[10]{#count}[42]{#count}`.
//!   Content belongs next to the words it is part of, and adjacency makes the
//!   sequence obvious in a diff.
//! - **`set:`**, in frontmatter, for properties: colour, weight, anything a
//!   theme defines. Those are timeline concerns, not prose.
//!
//! Both compile to the same `Set` action, and both produce **one** element
//! whose state changes — not two that swap. That is what makes stepping
//! backwards show the earlier value again with no history kept anywhere.

use slidx_core::{parse_deck, DeckParseOptions, StepAction, Visibility};

fn parse(source: &str) -> slidx_core::Deck {
    parse_deck(source, &DeckParseOptions::default())
}

#[test]
fn adjacent_takes_become_one_element_with_two_states() {
    let deck = parse("The answer is [10]{#count}[42]{#count}.\n");
    let slide = &deck.slides[0];

    assert_eq!(slide.timeline.len(), 2, "one take after the first is one stop");

    // One element in the markup, not two.
    assert_eq!(slide.content.matches("data-slidx-mark=\"count\"").count(), 1);
    assert!(slide.content.contains(">10</span>"));
    assert!(!slide.content.contains("42"), "the later take lives in the timeline, not the markup");
}

#[test]
fn the_timeline_carries_the_text_for_each_stop() {
    let deck = parse("The answer is [10]{#count}[42]{#count}.\n");
    let timeline = &deck.slides[0].timeline;
    let target = "[data-slidx-mark=\"count\"]";

    assert_eq!(timeline.frame(0).unwrap().content(target), None, "stop 0 shows the markup");
    assert_eq!(timeline.frame(1).unwrap().content(target), Some("42"));
}

#[test]
fn stepping_back_restores_the_earlier_value_with_no_history() {
    // The point of snapshots: going back is reading frame N-1, not undoing.
    let deck = parse("[one]{#w}[two]{#w}[three]{#w}\n");
    let timeline = &deck.slides[0].timeline;
    let target = "[data-slidx-mark=\"w\"]";

    assert_eq!(timeline.len(), 3);
    assert_eq!(timeline.frame(2).unwrap().content(target), Some("three"));
    assert_eq!(timeline.frame(1).unwrap().content(target), Some("two"));
    assert_eq!(timeline.frame(0).unwrap().content(target), None);
}

#[test]
fn a_take_stays_visible_throughout() {
    // A changing value is not an entrance; it must not start hidden.
    let deck = parse("[10]{#count}[42]{#count}\n");
    let target = "[data-slidx-mark=\"count\"]";

    assert_eq!(
        deck.slides[0].timeline.frame(0).unwrap().visibility(target),
        Some(Visibility::Visible)
    );
}

#[test]
fn a_take_can_change_properties_as_well_as_text() {
    let deck = parse("[pending]{#status}[done]{#status color=success}\n");
    let frame = deck.slides[0].timeline.frame(1).unwrap();
    let target = "[data-slidx-mark=\"status\"]";

    assert_eq!(frame.content(target), Some("done"));
    assert_eq!(frame.property(target, "color"), Some("success"));
}

#[test]
fn takes_must_be_adjacent_to_count_as_one_element() {
    // Two marks sharing a key from opposite ends of a slide is a duplicated
    // key, not a sequence. Silently merging them would delete content.
    let deck = parse("First [a]{#k} and then some prose and then [b]{#k}.\n");

    assert!(deck.diagnostics.iter().any(|d| d.code == "mark/ambiguous-key"));
    assert!(deck.slides[0].content.contains("a"));
    assert!(deck.slides[0].content.contains("b"), "neither mark is removed");
}

#[test]
fn whitespace_between_takes_is_allowed_and_removed() {
    let deck = parse("[10]{#count} [42]{#count} apples\n");

    assert_eq!(deck.slides[0].timeline.len(), 2);
    assert!(!deck.slides[0].content.contains("  "), "removing a take leaves no double space");
}

#[test]
fn set_changes_a_property_without_touching_the_text() {
    let deck = parse(
        "---\nsteps:\n  - set: { target: \"#status\", color: danger }\n---\n\nStatus: [ok]{#status}\n",
    );

    let frame = deck.slides[0].timeline.frame(1).unwrap();
    let target = "[data-slidx-mark=\"status\"]";

    assert_eq!(frame.property(target, "color"), Some("danger"));
    assert_eq!(frame.content(target), None, "text is left alone");
}

#[test]
fn set_can_change_the_text_too() {
    let deck =
        parse("---\nsteps:\n  - set: { target: \"#count\", text: \"42\" }\n---\n\n[10]{#count}\n");

    assert_eq!(
        deck.slides[0].timeline.frame(1).unwrap().content("[data-slidx-mark=\"count\"]"),
        Some("42")
    );
}

#[test]
fn properties_accumulate_across_steps() {
    // Two independent switches must not clear each other: setting the colour
    // at one stop and the weight at the next leaves both in force.
    let deck = parse(
        "---\nsteps:\n  - set: { target: \"#s\", color: danger }\n  - set: { target: \"#s\", weight: bold }\n---\n\n[x]{#s}\n",
    );

    let frame = deck.slides[0].timeline.frame(2).unwrap();
    let target = "[data-slidx-mark=\"s\"]";

    assert_eq!(frame.property(target, "color"), Some("danger"));
    assert_eq!(frame.property(target, "weight"), Some("bold"));
}

#[test]
fn a_later_step_overwrites_the_same_property() {
    let deck = parse(
        "---\nsteps:\n  - set: { target: \"#s\", color: danger }\n  - set: { target: \"#s\", color: success }\n---\n\n[x]{#s}\n",
    );

    assert_eq!(
        deck.slides[0].timeline.frame(2).unwrap().property("[data-slidx-mark=\"s\"]", "color"),
        Some("success")
    );
}

#[test]
fn a_set_that_changes_nothing_is_reported() {
    let deck = parse("---\nsteps:\n  - set: { target: \"#s\" }\n---\n\n[x]{#s}\n");

    assert!(deck.diagnostics.iter().any(|d| d.code == "steps/invalid-action"));
    assert!(!deck.diagnostics.has_blocking(), "the deck still renders");
}

#[test]
fn set_actions_are_parsed_as_such() {
    let deck = parse("---\nsteps:\n  - set: { target: \"#s\", text: \"y\" }\n---\n\n[x]{#s}\n");
    assert!(matches!(deck.slides[0].steps.actions[0], StepAction::Set { .. }));
}

#[test]
fn the_handout_shows_the_value_the_slide_ended_on() {
    // Content replaces rather than accumulates, so the printed page shows the
    // answer rather than every intermediate value stacked up.
    let deck = parse("[10]{#count}[20]{#count}[42]{#count}\n");
    let print = deck.slides[0].timeline.print_frame();

    assert_eq!(print.content("[data-slidx-mark=\"count\"]"), Some("42"));
}

#[test]
fn takes_run_after_the_reveals_that_brought_the_element_on_screen() {
    let deck = parse("- one <!-- step -->\n- answer [10]{#c}[42]{#c} <!-- step -->\n");
    let actions = &deck.slides[0].steps.actions;

    assert!(matches!(actions[0], StepAction::Reveal { .. }));
    assert!(matches!(actions[1], StepAction::Reveal { .. }));
    assert!(matches!(actions[2], StepAction::Set { .. }));
}

#[test]
fn a_single_mark_is_not_a_take() {
    let deck = parse("The answer is [42]{#count}.\n");

    assert!(deck.slides[0].timeline.is_single_stop());
    assert!(deck.diagnostics.is_empty());
}

#[test]
fn takes_need_a_key_to_be_a_sequence() {
    // Without a key there is nothing to identify the element across stops, so
    // two styled fragments stay two fragments.
    let deck = parse("[a]{.accent}[b]{.accent}\n");

    assert!(deck.slides[0].timeline.is_single_stop());
    assert_eq!(deck.slides[0].content.matches("<span").count(), 2);
}
