//! Turning authored intents into snapshots.
//!
//! One pass, no lookahead, no backtracking: walk the actions in order, and
//! after each one the frame holds the complete state of the slide. Nothing
//! here computes a difference, which is why there is nothing to invert when a
//! presenter steps backwards.
//!
//! Split from [`super::timeline`] because the two answer different questions —
//! that module says what a stop *is*, this one says how a list of intents
//! becomes a list of stops.

use std::collections::BTreeMap;

use super::action::{Patch, StepAction, StepSource, Visibility};
use super::preset::EffectKind;
use super::timeline::{ElementState, StepFrame, StepTimeline};
use super::timing::{Effect, StepOptions};

/// Compiles authored intents into snapshots.
pub fn compile_timeline(source: &StepSource) -> StepTimeline {
    let mut frames = vec![initial_frame(source)];

    for action in &source.actions {
        if !action.is_auto() {
            let mut next = frames.last().expect("seeded above").clone();
            next.index += 1;
            next.clear_effects();
            frames.push(next);
        }

        let frame = frames.last_mut().expect("seeded above");
        apply(frame, action, None);
    }

    StepTimeline { frames }
}

/// Seeds frame zero with every target the slide will ever touch.
///
/// Registering targets up front means the renderer can reserve layout space, so
/// a reveal never shifts the text already on screen.
fn initial_frame(source: &StepSource) -> StepFrame {
    let mut frame = StepFrame::default();

    for action in &source.actions {
        seed(&mut frame, action);
    }

    frame
}

fn seed(frame: &mut StepFrame, action: &StepAction) {
    match action {
        StepAction::Group { actions, .. } => {
            for nested in actions {
                seed(frame, nested);
            }
        }
        _ => {
            for target in action.targets() {
                if frame.state(target).is_some() {
                    continue;
                }

                // An element whose first mention is a reveal starts off screen.
                // Anything else — hidden later, emphasised — was authored into
                // the slide body and starts visible.
                let visibility = match action {
                    StepAction::Reveal { .. } => Visibility::Hidden,
                    _ => Visibility::Visible,
                };

                frame.states.push(ElementState {
                    target: target.to_string(),
                    visibility,
                    content: None,
                    properties: BTreeMap::new(),
                    effect: None,
                });
            }
        }
    }
}

/// Applies one action to a frame in place.
///
/// `inherited` carries a wrapping group's timing so nested actions that did not
/// set their own `after` still stagger with the group.
fn apply(frame: &mut StepFrame, action: &StepAction, inherited: Option<&StepOptions>) {
    match action {
        StepAction::Group { actions, options } => {
            for nested in actions {
                apply(frame, nested, Some(options));
            }
        }
        StepAction::Reveal { target, options } => {
            let options = merge(options, inherited);
            set(frame, target, Some(Visibility::Visible), options.resolve(EffectKind::Entrance));
        }
        StepAction::Hide { target, options } => {
            let options = merge(options, inherited);
            set(frame, target, Some(Visibility::Hidden), options.resolve(EffectKind::Exit));
        }
        StepAction::Emphasize { target, options } => {
            let options = merge(options, inherited);
            set(frame, target, None, options.resolve(EffectKind::Emphasis));
        }
        StepAction::Set { target, patch, options } => {
            let options = merge(options, inherited);
            // A patch describes a change to something already on screen, so it
            // reads as emphasis rather than as an entrance.
            set(frame, target, None, options.resolve(EffectKind::Emphasis));
            patch_state(frame, target, patch);
        }
    }
}

fn merge(options: &StepOptions, inherited: Option<&StepOptions>) -> StepOptions {
    let Some(inherited) = inherited else {
        return options.clone();
    };

    StepOptions {
        after: options.after.or(inherited.after),
        preset: options.preset.or(inherited.preset),
        origin: options.origin.or(inherited.origin),
        ..options.clone()
    }
}

fn set(frame: &mut StepFrame, target: &str, visibility: Option<Visibility>, effect: Effect) {
    match frame.state_mut(target) {
        Some(state) => {
            if let Some(visibility) = visibility {
                state.visibility = visibility;
            }
            state.effect = Some(effect);
        }
        None => frame.states.push(ElementState {
            target: target.to_string(),
            visibility: visibility.unwrap_or(Visibility::Visible),
            content: None,
            properties: BTreeMap::new(),
            effect: Some(effect),
        }),
    }
}

/// Applies a patch to a frame's state for one target.
///
/// Properties accumulate and content replaces, which matches how the two are
/// used: properties are independent switches, content is one value.
fn patch_state(frame: &mut StepFrame, target: &str, patch: &Patch) {
    let Some(state) = frame.state_mut(target) else { return };

    if let Some(content) = &patch.content {
        state.content = Some(content.clone());
    }
    state.properties.extend(patch.properties.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steps::preset::{EffectPreset, Origin};

    fn source(actions: Vec<StepAction>) -> StepSource {
        StepSource { actions, auto: None }
    }

    #[test]
    fn a_target_first_mentioned_by_hide_starts_visible() {
        let timeline = compile_timeline(&source(vec![StepAction::hide(".a")]));
        assert_eq!(timeline.frame(0).unwrap().visibility(".a"), Some(Visibility::Visible));
        assert_eq!(timeline.frame(1).unwrap().visibility(".a"), Some(Visibility::Hidden));
    }

    #[test]
    fn a_target_first_mentioned_by_emphasize_starts_visible() {
        let timeline =
            compile_timeline(&source(vec![StepAction::emphasize(".a", EffectPreset::Shake)]));
        assert_eq!(timeline.frame(0).unwrap().visibility(".a"), Some(Visibility::Visible));
    }

    #[test]
    fn group_options_are_inherited_by_nested_actions() {
        let timeline = compile_timeline(&source(vec![StepAction::Group {
            actions: vec![StepAction::reveal(".a"), StepAction::reveal(".b")],
            options: StepOptions {
                preset: Some(EffectPreset::FlyIn),
                origin: Some(Origin::Left),
                ..StepOptions::default()
            },
        }]));

        let frame = timeline.frame(1).unwrap();
        assert_eq!(frame.effect(".a").unwrap().preset, EffectPreset::FlyIn);
        assert_eq!(frame.effect(".b").unwrap().origin, Some(Origin::Left));
    }

    #[test]
    fn nested_options_win_over_group_options() {
        let timeline = compile_timeline(&source(vec![StepAction::Group {
            actions: vec![StepAction::reveal(".a").with_preset(EffectPreset::Zoom)],
            options: StepOptions { preset: Some(EffectPreset::FlyIn), ..StepOptions::default() },
        }]));

        assert_eq!(timeline.frame(1).unwrap().effect(".a").unwrap().preset, EffectPreset::Zoom);
    }

    #[test]
    fn clamp_keeps_stale_deep_links_on_a_real_frame() {
        let timeline = compile_timeline(&source(vec![StepAction::reveal(".a")]));
        assert_eq!(timeline.clamp(0), 0);
        assert_eq!(timeline.clamp(1), 1);
        assert_eq!(timeline.clamp(99), 1, "an out-of-date link lands on the last stop");
    }

    #[test]
    fn a_slide_with_only_timed_actions_is_a_single_stop() {
        let timeline = compile_timeline(&source(vec![
            StepAction::reveal(".a").after_ms(200),
            StepAction::reveal(".b").after_ms(400),
        ]));

        assert!(timeline.is_single_stop(), "timed reveals never block the presenter");
        assert_eq!(timeline.frame(0).unwrap().visibility(".b"), Some(Visibility::Visible));
    }

    #[test]
    fn visible_targets_lists_only_painted_elements() {
        let timeline =
            compile_timeline(&source(vec![StepAction::reveal(".a"), StepAction::reveal(".b")]));

        assert_eq!(timeline.frame(1).unwrap().visible_targets(), vec![".a"]);
        assert_eq!(timeline.frame(2).unwrap().visible_targets(), vec![".a", ".b"]);
    }

    #[test]
    fn frames_carry_their_own_index() {
        let timeline =
            compile_timeline(&source(vec![StepAction::reveal(".a"), StepAction::reveal(".b")]));

        for (position, frame) in timeline.frames().iter().enumerate() {
            assert_eq!(frame.index as usize, position);
        }
    }

    #[test]
    fn compilation_is_deterministic() {
        let actions = vec![
            StepAction::reveal(".a"),
            StepAction::group(vec![StepAction::reveal(".b"), StepAction::hide(".a")]),
            StepAction::emphasize(".b", EffectPreset::Pulse),
        ];

        assert_eq!(compile_timeline(&source(actions.clone())), compile_timeline(&source(actions)));
    }
}
