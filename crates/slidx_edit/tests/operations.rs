//! What each operation does to a file.
//!
//! Every assertion here is on the *source*, not on the model. A test that
//! checked `deck.slides[0].title` would pass just as happily for an
//! implementation that reformatted the rest of the file on the way past, and
//! that implementation is precisely the one this crate exists to avoid.

use slidx_core::{DeckParseOptions, Mark, StepAction};
use slidx_edit::{apply, plan, EditError, EditOp, MarkAttributes};

/// A deck with one of everything an operation can address.
const DECK: &str = "\
---
title: Fast Decks
duration: 20m
---

#   Introduction

Some [words]{#hero .accent} here.

<!-- notes: say hello -->

---

# Deep Dive

- one
- two

---
layout: split
---

# Closing

Thanks.
";

fn edit(source: &str, op: EditOp) -> String {
    apply(source, &DeckParseOptions::default(), &op).expect("the operation names something real")
}

/// The one-based lines of `before` that an edit did not leave alone.
///
/// Computed from the longest common prefix and suffix rather than line by
/// line, so an operation that changes the number of lines still reports the
/// window it touched instead of everything below it.
fn touched_lines(before: &str, after: &str) -> Vec<usize> {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();

    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let room = old.len().min(new.len()) - prefix;
    let suffix =
        old.iter().rev().zip(new.iter().rev()).take(room).take_while(|(a, b)| a == b).count();

    ((prefix + 1)..=(old.len() - suffix)).collect()
}

// ---------------------------------------------------------------- slide text

#[test]
fn setting_a_heading_keeps_the_level_and_the_spacing_the_author_chose() {
    let result =
        edit(DECK, EditOp::SetHeading { slide: 0.into(), text: "Where decks go wrong".into() });

    assert!(result.contains("#   Where decks go wrong"), "{result}");
    assert_eq!(touched_lines(DECK, &result), vec![6]);
}

#[test]
fn a_slide_without_a_heading_gains_one_above_its_body() {
    let source = "Just prose.\n";
    let result = edit(source, EditOp::SetHeading { slide: 0.into(), text: "Title".into() });

    assert_eq!(result, "# Title\n\nJust prose.\n");
}

#[test]
fn setting_a_heading_to_what_it_already_says_is_not_an_edit() {
    let op = EditOp::SetHeading { slide: 0.into(), text: "Introduction".into() };
    let edit = plan(DECK, &DeckParseOptions::default(), &op).unwrap();

    assert!(edit.is_empty(), "{:?}", edit.splices());
    assert_eq!(edit.apply(DECK), DECK);
}

#[test]
fn setting_a_body_leaves_the_frontmatter_and_the_neighbours_alone() {
    let result =
        edit(DECK, EditOp::SetBody { slide: 1.into(), body: "# Deep Dive\n\n- only one".into() });

    assert!(result.starts_with("---\ntitle: Fast Decks\nduration: 20m\n---\n"));
    assert!(result.contains("# Deep Dive\n\n- only one\n"));
    assert!(result.ends_with("# Closing\n\nThanks.\n"));
    assert_eq!(touched_lines(DECK, &result), vec![16, 17]);
}

// --------------------------------------------------------------------- order

#[test]
fn inserting_a_slide_writes_one_separator_and_moves_nothing_else() {
    let result = edit(DECK, EditOp::InsertSlide { at: 1, body: "# Agenda".into() });

    assert_eq!(result, DECK.replace("---\n\n# Deep Dive", "---\n\n# Agenda\n\n---\n\n# Deep Dive"));
}

#[test]
fn a_slide_can_be_appended_after_the_last_one() {
    let result = edit(DECK, EditOp::InsertSlide { at: 3, body: "# Questions".into() });

    assert_eq!(result, DECK.replace("Thanks.\n", "Thanks.\n\n---\n\n# Questions\n"));
}

#[test]
fn inserting_before_a_slide_that_carries_frontmatter_still_writes_a_separator() {
    // The `---` opening that slide's frontmatter is also the separator that
    // ends the slide before it, so a new slide in between needs its own.
    let result = edit(DECK, EditOp::InsertSlide { at: 2, body: "# Interlude".into() });

    assert_eq!(
        result,
        DECK.replace("---\nlayout: split", "---\n\n# Interlude\n\n---\nlayout: split")
    );
    assert_eq!(slide_titles(&result), ["Introduction", "Deep Dive", "Interlude", "Closing"]);
}

#[test]
fn removing_a_slide_takes_its_separator_with_it() {
    let result = edit(DECK, EditOp::RemoveSlide { slide: 1.into() });

    assert_eq!(result, DECK.replace("\n\n---\n\n# Deep Dive\n\n- one\n- two", ""));
    assert_eq!(slide_titles(&result), ["Introduction", "Closing"]);
}

#[test]
fn removing_the_first_slide_leaves_the_decks_frontmatter_behind() {
    let result = edit(DECK, EditOp::RemoveSlide { slide: 0.into() });

    assert!(result.starts_with("---\ntitle: Fast Decks\nduration: 20m\n---\n"));
    assert_eq!(slide_titles(&result), ["Deep Dive", "Closing"]);
}

#[test]
fn moving_a_slide_moves_its_bytes_and_leaves_the_separators_where_they_were() {
    let result = edit(DECK, EditOp::MoveSlide { slide: 0.into(), to: 1 });

    assert_eq!(slide_titles(&result), ["Deep Dive", "Introduction", "Closing"]);
    assert!(result.contains("Some [words]{#hero .accent} here."), "the mark travels intact");
    assert!(result.contains("<!-- notes: say hello -->"), "so do the notes");
}

#[test]
fn moving_a_slide_onto_itself_is_not_an_edit() {
    let op = EditOp::MoveSlide { slide: 1.into(), to: 1 };
    assert!(plan(DECK, &DeckParseOptions::default(), &op).unwrap().is_empty());
}

// --------------------------------------------------------------- frontmatter

#[test]
fn setting_a_field_replaces_the_value_and_nothing_around_it() {
    let result = edit(
        DECK,
        EditOp::SetField { slide: 0.into(), key: "title".into(), value: "Slow Decks".into() },
    );

    assert_eq!(touched_lines(DECK, &result), vec![2]);
    assert!(result.contains("title: Slow Decks"));
}

#[test]
fn a_field_the_deck_does_not_have_is_appended_to_the_block() {
    let result = edit(
        DECK,
        EditOp::SetField { slide: 0.into(), key: "theme".into(), value: "terminal".into() },
    );

    assert!(result.starts_with("---\ntitle: Fast Decks\nduration: 20m\ntheme: terminal\n---\n"));
}

#[test]
fn a_slide_without_frontmatter_gains_a_block_between_the_separators() {
    let result =
        edit(DECK, EditOp::SetField { slide: 1.into(), key: "budget".into(), value: "90s".into() });

    assert!(result.contains("---\nbudget: 90s\n---\n\n# Deep Dive"), "{result}");
    assert_eq!(slide_titles(&result), ["Introduction", "Deep Dive", "Closing"]);
}

#[test]
fn a_field_is_written_with_its_type_rather_than_as_text() {
    let source = "# One\n";
    let result = edit(
        source,
        EditOp::SetField { slide: 0.into(), key: "optional".into(), value: true.into() },
    );

    assert_eq!(result, "---\noptional: true\n---\n\n# One\n");
    assert!(parse(&result).slides[0].optional);
}

#[test]
fn setting_a_field_to_its_current_value_is_not_an_edit() {
    let op = EditOp::SetField { slide: 0.into(), key: "title".into(), value: "Fast Decks".into() };
    assert!(plan(DECK, &DeckParseOptions::default(), &op).unwrap().is_empty());
}

// -------------------------------------------------------------------- marks

#[test]
fn adding_a_mark_wraps_the_selection_and_leaves_the_line_otherwise_intact() {
    let source = "# One\n\nMaking decks fast.\n";
    let attributes = MarkAttributes::default().with_key("point").with_class("accent");
    let result =
        edit(source, EditOp::AddMark { slide: 0.into(), range: (20..24).into(), attributes });

    assert_eq!(result, "# One\n\nMaking decks [fast]{#point .accent}.\n");
}

#[test]
fn editing_a_mark_rewrites_its_attributes_and_not_its_text() {
    let attributes = MarkAttributes::default().with_key("hero").with_class("danger");
    let result = edit(DECK, EditOp::SetMark { slide: 0.into(), mark: 0.into(), attributes });

    assert_eq!(touched_lines(DECK, &result), vec![8]);
    assert!(result.contains("Some [words]{#hero .danger} here."));
}

#[test]
fn a_mark_stripped_of_every_attribute_becomes_the_words_it_wrapped() {
    // `[words]{}` is not something a person meant to write, so removing the
    // last class removes the mark rather than leaving an empty one.
    let result = edit(
        DECK,
        EditOp::SetMark {
            slide: 0.into(),
            mark: "hero".into(),
            attributes: MarkAttributes::default(),
        },
    );

    assert!(result.contains("Some words here."));
}

#[test]
fn removing_a_mark_keeps_the_words() {
    let result = edit(DECK, EditOp::RemoveMark { slide: 0.into(), mark: "hero".into() });

    assert_eq!(touched_lines(DECK, &result), vec![8]);
    assert!(result.contains("Some words here."));
    assert!(parse(&result).slides[0].marks.is_empty());
}

#[test]
fn a_mark_that_is_not_there_is_an_error_rather_than_a_panic() {
    let op = EditOp::RemoveMark { slide: 0.into(), mark: "missing".into() };

    assert_eq!(
        plan(DECK, &DeckParseOptions::default(), &op),
        Err(EditError::NoSuchMark { mark: "missing".into() })
    );
}

// -------------------------------------------------------------------- steps

#[test]
fn adding_a_step_creates_the_list_on_a_slide_that_has_none() {
    let source = "---\ntitle: T\n---\n\n# One\n";
    let result = edit(
        source,
        EditOp::AddStep { slide: 0.into(), at: None, action: StepAction::reveal(".a") },
    );

    assert_eq!(result, "---\ntitle: T\nsteps:\n  - reveal: \".a\"\n---\n\n# One\n");
}

#[test]
fn a_second_step_is_one_more_line_and_nothing_else() {
    let source = "---\nsteps:\n  - reveal: \".a\"\n---\n\n# One\n";
    let result =
        edit(source, EditOp::AddStep { slide: 0.into(), at: None, action: StepAction::hide(".b") });

    assert_eq!(result, "---\nsteps:\n  - reveal: \".a\"\n  - hide: \".b\"\n---\n\n# One\n");
    assert!(touched_lines(source, &result).is_empty(), "a new step adds a line and rewrites none");
}

#[test]
fn removing_a_step_removes_its_line() {
    let source =
        "---\nsteps:\n  - reveal: \".a\"\n  - hide: \".b\"\n  - reveal: \".c\"\n---\n\n# One\n";
    let result = edit(source, EditOp::RemoveStep { slide: 0.into(), index: 1 });

    assert_eq!(result, "---\nsteps:\n  - reveal: \".a\"\n  - reveal: \".c\"\n---\n\n# One\n");
}

#[test]
fn a_step_that_is_not_declared_is_an_error_rather_than_a_panic() {
    let op = EditOp::RemoveStep { slide: 0.into(), index: 3 };

    assert_eq!(
        plan(DECK, &DeckParseOptions::default(), &op),
        Err(EditError::NoSuchStep { index: 3, present: 0 })
    );
}

#[test]
fn a_step_added_to_a_slide_staged_with_markers_keeps_the_staging_it_had() {
    // Promoting the light form to the explicit one must not silently drop the
    // reveals the author already has: `steps:` takes precedence over markers,
    // so writing it has to carry them across.
    let source = "# One\n\n- a <!-- step -->\n- b <!-- step -->\n";
    let result =
        edit(source, EditOp::AddStep { slide: 0.into(), at: None, action: StepAction::hide(".c") });

    let deck = parse(&result);
    assert_eq!(deck.slides[0].steps.actions.len(), 3);
    assert_eq!(deck.slides[0].steps.actions[2].targets(), vec![".c"]);
}

#[test]
fn a_step_added_at_a_position_becomes_the_stop_that_was_there() {
    // A timeline's cell is a stop, not the end of a list. Clicking one has to
    // put the action where the click was, or the gesture would be an add
    // followed by a move — two operations and two undo presses.
    let source = "---\nsteps:\n  - reveal: \".a\"\n  - reveal: \".c\"\n---\n\n# One\n";
    let result = edit(
        source,
        EditOp::AddStep { slide: 0.into(), at: Some(1), action: StepAction::reveal(".b") },
    );

    assert_eq!(
        result,
        "---\nsteps:\n  - reveal: \".a\"\n  - reveal: \".b\"\n  - reveal: \".c\"\n---\n\n# One\n"
    );
    assert!(touched_lines(source, &result).is_empty(), "an inserted step rewrites no line");
}

#[test]
fn a_position_past_the_end_of_the_list_appends_rather_than_refusing() {
    // The last column of a timeline is one past the last action, and reaching
    // it must not depend on the editor counting the list the same way.
    let source = "---\nsteps:\n  - reveal: \".a\"\n---\n\n# One\n";
    let result = edit(
        source,
        EditOp::AddStep { slide: 0.into(), at: Some(9), action: StepAction::hide(".b") },
    );

    assert_eq!(result, "---\nsteps:\n  - reveal: \".a\"\n  - hide: \".b\"\n---\n\n# One\n");
}

#[test]
fn moving_a_step_moves_its_line_and_writes_no_other() {
    let source =
        "---\nsteps:\n  - reveal: \".a\"\n  - hide: \".b\"\n  - reveal: \".c\"\n---\n\n# One\n";
    let result = edit(source, EditOp::MoveStep { slide: 0.into(), from: 2, to: 0 });

    assert_eq!(
        result,
        "---\nsteps:\n  - reveal: \".c\"\n  - reveal: \".a\"\n  - hide: \".b\"\n---\n\n# One\n"
    );
}

#[test]
fn moving_a_step_to_where_it_already_is_is_not_an_edit_at_all() {
    let source = "---\nsteps:\n  - reveal: \".a\"\n  - hide: \".b\"\n---\n\n# One\n";
    let op = EditOp::MoveStep { slide: 0.into(), from: 1, to: 1 };

    assert!(plan(source, &DeckParseOptions::default(), &op).unwrap().is_empty());
}

#[test]
fn moving_a_step_somewhere_the_list_does_not_reach_is_an_error() {
    let source = "---\nsteps:\n  - reveal: \".a\"\n  - hide: \".b\"\n---\n\n# One\n";

    for op in [
        EditOp::MoveStep { slide: 0.into(), from: 5, to: 0 },
        EditOp::MoveStep { slide: 0.into(), from: 0, to: 5 },
    ] {
        assert_eq!(
            plan(source, &DeckParseOptions::default(), &op),
            Err(EditError::NoSuchStep { index: 5, present: 2 })
        );
    }
}

#[test]
fn setting_a_step_rewrites_its_line_and_leaves_the_list_around_it() {
    let source =
        "---\nsteps:\n  - reveal: \".a\"\n  - hide: \".b\"\n  - reveal: \".c\"\n---\n\n# One\n";
    let result = edit(
        source,
        EditOp::SetStep {
            slide: 0.into(),
            index: 1,
            action: StepAction::emphasize(".b", slidx_core::EffectPreset::Pulse),
        },
    );

    assert_eq!(touched_lines(source, &result), vec![4]);
    assert!(result.contains("- emphasize: { target: \".b\", preset: pulse }"), "{result}");
}

#[test]
fn retiming_a_step_is_the_same_one_line_because_timing_is_written_inline() {
    // The reason an action serialises as a flow mapping: a retimed step has to
    // stay one line, or a timeline that adjusts one stop would diff as three.
    let source = "---\nsteps:\n  - reveal: \".a\"\n---\n\n# One\n";
    let result = edit(
        source,
        EditOp::SetStep {
            slide: 0.into(),
            index: 0,
            action: StepAction::reveal(".a").with_duration(700),
        },
    );

    assert_eq!(
        result,
        "---\nsteps:\n  - reveal: { target: \".a\", duration: 700 }\n---\n\n# One\n"
    );
}

#[test]
fn setting_a_step_that_is_not_declared_is_an_error_rather_than_an_append() {
    let source = "---\nsteps:\n  - reveal: \".a\"\n---\n\n# One\n";
    let op = EditOp::SetStep { slide: 0.into(), index: 4, action: StepAction::hide(".b") };

    assert_eq!(
        plan(source, &DeckParseOptions::default(), &op),
        Err(EditError::NoSuchStep { index: 4, present: 1 })
    );
}

#[test]
fn writing_out_generated_steps_puts_the_list_the_slide_was_running_into_the_file() {
    // `autoSteps:` is a one-way door and this operation is the door. What it
    // writes has to be the pipeline the slide already ran, or opening the door
    // would change the talk.
    let source = "---\nautoSteps: list\n---\n\n- one\n- two\n";
    let result = edit(source, EditOp::AdoptSteps { slide: 0.into() });

    assert_eq!(
        result,
        concat!(
            "---\nautoSteps: list\nsteps:\n",
            "  - reveal: \"[data-slidx-step=\\\"1\\\"]\"\n",
            "  - reveal: \"[data-slidx-step=\\\"2\\\"]\"\n",
            "---\n\n- one\n- two\n"
        )
    );
    assert_eq!(parse(source).slides[0].timeline.len(), parse(&result).slides[0].timeline.len());
}

#[test]
fn writing_out_generated_steps_leaves_auto_steps_in_place_because_it_owns_the_anchors() {
    // The written-out steps name `[data-slidx-step="N"]`, and `autoSteps:` is
    // what puts those anchors in the markup. Removing the key would leave a
    // list of steps that target nothing.
    let source = "---\nautoSteps: list\n---\n\n- one\n- two\n";
    let result = edit(source, EditOp::AdoptSteps { slide: 0.into() });

    assert!(result.contains("autoSteps: list"));
    let deck = parse(&result);
    assert_eq!(deck.slides[0].timeline.frame(1).unwrap().visible_targets().len(), 1);
}

#[test]
fn writing_out_steps_a_slide_already_declares_is_not_an_edit() {
    // Idempotence, and the thing that keeps the door one-way rather than a
    // switch: a second press cannot rewrite a list the author has since
    // reordered.
    let source = "---\nsteps:\n  - reveal: \".a\"\n---\n\n# One\n";
    let op = EditOp::AdoptSteps { slide: 0.into() };

    assert!(plan(source, &DeckParseOptions::default(), &op).unwrap().is_empty());
}

#[test]
fn writing_out_the_steps_of_a_marker_staged_slide_keeps_the_reveals_it_had() {
    let source = "# One\n\n- a <!-- step -->\n- b <!-- step -->\n";
    let result = edit(source, EditOp::AdoptSteps { slide: 0.into() });

    let deck = parse(&result);
    assert_eq!(deck.slides[0].steps.actions.len(), 2);
    assert_eq!(deck.slides[0].timeline.len(), 3);
}

#[test]
fn writing_out_the_steps_of_a_slide_that_has_none_leaves_an_empty_list() {
    // An empty `steps:` rather than no key: the slide now says it stages
    // nothing, which is a different statement from never having been asked.
    let source = "---\ntitle: T\n---\n\n# One\n";
    let result = edit(source, EditOp::AdoptSteps { slide: 0.into() });

    assert_eq!(result, "---\ntitle: T\nsteps: []\n---\n\n# One\n");
}

// -------------------------------------------------------------------- notes

#[test]
fn setting_notes_rewrites_the_words_and_not_the_comment_around_them() {
    let result =
        edit(DECK, EditOp::SetNotes { slide: 0.into(), notes: "mention the outage".into() });

    assert_eq!(touched_lines(DECK, &result), vec![10]);
    assert!(result.contains("<!-- notes: mention the outage -->"));
}

#[test]
fn a_slide_without_notes_gains_a_comment_after_its_body() {
    let result = edit(DECK, EditOp::SetNotes { slide: 1.into(), notes: "slow down here".into() });

    assert!(result.contains("- two\n\n<!-- notes: slow down here -->\n"), "{result}");
    assert_eq!(parse(&result).slides[1].notes, vec!["slow down here"]);
}

#[test]
fn emptying_notes_removes_the_comment_and_the_line_it_sat_on() {
    let result = edit(DECK, EditOp::SetNotes { slide: 0.into(), notes: String::new() });

    assert!(!result.contains("notes:"));
    assert_eq!(parse(&result).slides[0].notes, Vec::<String>::new());
    assert!(result.contains("Some [words]{#hero .accent} here.\n\n---\n"), "{result}");
}

#[test]
fn a_slide_with_several_note_blocks_ends_up_with_one() {
    let source = "# One\n\n<!-- notes: first -->\n\nBody.\n\n<!-- notes: second -->\n";
    let result = edit(source, EditOp::SetNotes { slide: 0.into(), notes: "only".into() });

    assert_eq!(parse(&result).slides[0].notes, vec!["only"]);
    assert!(result.contains("Body."), "the prose between the notes is not the notes");
}

// ------------------------------------------------------------------ helpers

fn parse(source: &str) -> slidx_core::Deck {
    slidx_core::parse_deck(source, &DeckParseOptions::default())
}

fn slide_titles(source: &str) -> Vec<String> {
    parse(source).slides.iter().map(|slide| slide.display_title()).collect()
}

#[test]
fn a_marks_own_source_is_what_the_editor_writes() {
    // Guards the seam between this crate and the mark model: an operation
    // hands over attributes, and the canonical form comes from `Mark`.
    let attributes =
        MarkAttributes::default().with_class("accent").with_property("color", "danger");
    assert_eq!(
        attributes.onto("x"),
        Mark::new("x").with_class("accent").with_property("color", "danger")
    );
}
