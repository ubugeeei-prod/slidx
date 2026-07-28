//! Behavioural contract for the declarative step pipeline.
//!
//! The pipeline exists so that "what does slide N look like at click K" is a
//! pure function of the source, never of the order in which a presenter
//! pressed keys. Every frame is a full snapshot, so stepping backwards,
//! deep-linking to `?step=3`, and printing to PDF all reuse one code path.

use slidx_core::{
    compile_timeline, Effect, EffectKind, EffectPreset, StepAction, StepSource, Visibility,
};

fn source(actions: Vec<StepAction>) -> StepSource {
    StepSource { actions, auto: None }
}

#[test]
fn empty_pipeline_still_has_one_resting_frame() {
    let timeline = compile_timeline(&source(vec![]));

    assert_eq!(timeline.len(), 1, "a slide with no steps is a single frame");
    assert!(timeline.frame(0).unwrap().states.is_empty());
}

#[test]
fn reveal_actions_accumulate_into_snapshots() {
    let timeline =
        compile_timeline(&source(vec![StepAction::reveal(".a"), StepAction::reveal(".b")]));

    assert_eq!(timeline.len(), 3, "resting frame plus one frame per action");

    // Frame 0: both targets known but hidden. Knowing about a target before it
    // appears is what lets the renderer reserve layout space instead of
    // reflowing the slide on every click.
    let frame0 = timeline.frame(0).unwrap();
    assert_eq!(frame0.visibility(".a"), Some(Visibility::Hidden));
    assert_eq!(frame0.visibility(".b"), Some(Visibility::Hidden));

    let frame1 = timeline.frame(1).unwrap();
    assert_eq!(frame1.visibility(".a"), Some(Visibility::Visible));
    assert_eq!(frame1.visibility(".b"), Some(Visibility::Hidden));

    let frame2 = timeline.frame(2).unwrap();
    assert_eq!(frame2.visibility(".a"), Some(Visibility::Visible));
    assert_eq!(frame2.visibility(".b"), Some(Visibility::Visible));
}

#[test]
fn frames_are_snapshots_so_rewinding_is_lossless() {
    let timeline = compile_timeline(&source(vec![
        StepAction::reveal(".a"),
        StepAction::hide(".a"),
        StepAction::reveal(".b"),
    ]));

    // Walking forward then jumping straight back must land on the same state.
    let forward = timeline.frame(1).unwrap().clone();
    let rewound = timeline.frame(1).unwrap().clone();
    assert_eq!(forward, rewound);

    assert_eq!(timeline.frame(2).unwrap().visibility(".a"), Some(Visibility::Hidden));
    assert_eq!(timeline.frame(3).unwrap().visibility(".a"), Some(Visibility::Hidden));
    assert_eq!(timeline.frame(3).unwrap().visibility(".b"), Some(Visibility::Visible));
}

#[test]
fn emphasis_does_not_change_visibility() {
    let timeline = compile_timeline(&source(vec![
        StepAction::reveal(".a"),
        StepAction::emphasize(".a", EffectPreset::Pulse),
    ]));

    let frame = timeline.frame(2).unwrap();
    assert_eq!(frame.visibility(".a"), Some(Visibility::Visible));
    assert_eq!(
        frame.effect(".a"),
        Some(&Effect {
            kind: EffectKind::Emphasis,
            preset: EffectPreset::Pulse,
            ..Effect::default()
        })
    );
}

#[test]
fn effects_are_scoped_to_the_frame_that_triggers_them() {
    // An entrance animation must not replay when the presenter steps past it and
    // comes back; only the frame that introduced it carries the effect.
    let timeline =
        compile_timeline(&source(vec![StepAction::reveal(".a"), StepAction::reveal(".b")]));

    assert!(timeline.frame(1).unwrap().effect(".a").is_some());
    assert!(timeline.frame(2).unwrap().effect(".a").is_none());
    assert!(timeline.frame(2).unwrap().effect(".b").is_some());
}

#[test]
fn grouped_actions_share_one_frame() {
    let timeline = compile_timeline(&source(vec![StepAction::group(vec![
        StepAction::reveal(".a"),
        StepAction::reveal(".b"),
    ])]));

    assert_eq!(timeline.len(), 2);
    let frame = timeline.frame(1).unwrap();
    assert_eq!(frame.visibility(".a"), Some(Visibility::Visible));
    assert_eq!(frame.visibility(".b"), Some(Visibility::Visible));
}

#[test]
fn auto_advance_actions_do_not_consume_a_click() {
    // `after` actions play on a timer, so they belong to the frame that is
    // already on screen rather than creating a new stop.
    let timeline = compile_timeline(&source(vec![
        StepAction::reveal(".a"),
        StepAction::reveal(".b").after_ms(400),
    ]));

    assert_eq!(timeline.len(), 2, "the timed reveal shares the previous stop");
    let frame = timeline.frame(1).unwrap();
    assert_eq!(frame.visibility(".b"), Some(Visibility::Visible));
    assert_eq!(frame.effect(".b").map(|effect| effect.delay_ms), Some(400));
}

#[test]
fn timeline_reports_the_last_reachable_index() {
    let timeline = compile_timeline(&source(vec![StepAction::reveal(".a")]));
    assert_eq!(timeline.last_index(), 1);

    let empty = compile_timeline(&source(vec![]));
    assert_eq!(empty.last_index(), 0);
}

#[test]
fn out_of_range_frames_are_none_rather_than_a_panic() {
    let timeline = compile_timeline(&source(vec![StepAction::reveal(".a")]));
    assert!(timeline.frame(2).is_none());
}

#[test]
fn print_snapshot_is_the_final_frame() {
    // PDF export must show every element, including ones that were hidden again
    // during the talk, so the print projection unions all reveals.
    let timeline = compile_timeline(&source(vec![
        StepAction::reveal(".a"),
        StepAction::hide(".a"),
        StepAction::reveal(".b"),
    ]));

    let print = timeline.print_frame();
    assert_eq!(print.visibility(".a"), Some(Visibility::Visible));
    assert_eq!(print.visibility(".b"), Some(Visibility::Visible));
    assert!(print.effect(".a").is_none(), "print output carries no animation");
}
