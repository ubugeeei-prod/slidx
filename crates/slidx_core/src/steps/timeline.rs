//! The compiled half of the step pipeline.
//!
//! Compilation turns an ordered list of intents into a vector of *complete*
//! state snapshots. Nothing downstream ever replays a delta: the renderer, the
//! presenter view, the deep-link handler, and the PDF exporter all just index
//! into this vector. That is what makes stepping backwards, resuming at
//! `?step=7`, and printing agree with each other by construction.

use serde::{Deserialize, Serialize};

use super::action::{Effect, StepAction, StepOptions, StepSource, Visibility};
use super::preset::EffectKind;

/// One element's state within one frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementState {
    pub target: String,
    pub visibility: Visibility,
    /// Set only on the frame that triggers the animation, so scrubbing
    /// backwards past an entrance does not replay it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<Effect>,
}

/// A complete description of the slide at one stop.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepFrame {
    pub index: u32,
    pub states: Vec<ElementState>,
}

impl StepFrame {
    pub fn visibility(&self, target: &str) -> Option<Visibility> {
        self.state(target).map(|state| state.visibility)
    }

    pub fn effect(&self, target: &str) -> Option<&Effect> {
        self.state(target).and_then(|state| state.effect.as_ref())
    }

    pub fn state(&self, target: &str) -> Option<&ElementState> {
        self.states.iter().find(|state| state.target == target)
    }

    /// Selectors that should be painted in this frame.
    pub fn visible_targets(&self) -> Vec<&str> {
        self.states
            .iter()
            .filter(|state| state.visibility == Visibility::Visible)
            .map(|state| state.target.as_str())
            .collect()
    }

    fn state_mut(&mut self, target: &str) -> Option<&mut ElementState> {
        self.states.iter_mut().find(|state| state.target == target)
    }

    fn clear_effects(&mut self) {
        for state in &mut self.states {
            state.effect = None;
        }
    }
}

/// Every stop on a slide, in order.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepTimeline {
    frames: Vec<StepFrame>,
}

impl StepTimeline {
    pub fn frames(&self) -> &[StepFrame] {
        &self.frames
    }

    pub fn frame(&self, index: usize) -> Option<&StepFrame> {
        self.frames.get(index)
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Highest index a presenter can reach on this slide.
    ///
    /// Always valid: a timeline is never empty, so this never underflows.
    pub fn last_index(&self) -> usize {
        self.frames.len().saturating_sub(1)
    }

    /// True when the slide advances in one press, with no internal stops.
    pub fn is_single_stop(&self) -> bool {
        self.frames.len() <= 1
    }

    /// Clamps an arbitrary index to a reachable frame.
    ///
    /// Deep links and restored sessions can name a step that no longer exists
    /// after an edit; landing on the nearest real frame beats a blank slide.
    pub fn clamp(&self, index: usize) -> usize {
        index.min(self.last_index())
    }

    /// The frame used for print and PDF output.
    ///
    /// Anything that was ever on screen is shown, because a handout that hides
    /// content the audience saw is worse than one that shows a little more.
    pub fn print_frame(&self) -> StepFrame {
        let mut frame = StepFrame { index: self.last_index() as u32, states: Vec::new() };

        for source in &self.frames {
            for state in &source.states {
                match frame.state_mut(&state.target) {
                    Some(existing) => {
                        if state.visibility == Visibility::Visible {
                            existing.visibility = Visibility::Visible;
                        }
                    }
                    None => frame.states.push(ElementState {
                        target: state.target.clone(),
                        visibility: state.visibility,
                        effect: None,
                    }),
                }
            }
        }

        frame
    }
}

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
            effect: Some(effect),
        }),
    }
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
