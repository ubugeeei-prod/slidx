//! What a slide looks like at each stop.
//!
//! A frame is a *complete* snapshot, not a delta. Nothing downstream ever
//! replays history: the renderer, the presenter view, the deep-link handler,
//! and the PDF exporter all index into this vector. That is what makes
//! stepping backwards, resuming at `?step=7`, and printing agree with each
//! other by construction rather than by care.
//!
//! Three kinds of state live in a frame, and they compose differently, which
//! is why they are separate fields rather than one bag of properties:
//! visibility accumulates towards the handout, content replaces, and
//! properties are independent switches.
//!
//! [`compile`](super::compile) turns authored intents into these snapshots.

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use super::action::Visibility;
use super::timing::Effect;

/// One element's state within one frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementState {
    pub target: String,
    pub visibility: Visibility,
    /// Text the element shows at this stop, when a step has changed it.
    ///
    /// `None` means "whatever is in the markup". Carrying the override in the
    /// snapshot rather than as a diff is what lets a presenter step backwards
    /// through a changing value and see the earlier one again, without the
    /// runtime remembering anything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Data properties in force at this stop. Accumulated, so a later patch
    /// that changes colour does not clear an earlier one that changed weight.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
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

    /// Text the element shows at this stop, if a step overrode it.
    pub fn content(&self, target: &str) -> Option<&str> {
        self.state(target).and_then(|state| state.content.as_deref())
    }

    /// Value of one data property at this stop.
    pub fn property(&self, target: &str, name: &str) -> Option<&str> {
        self.state(target).and_then(|state| state.properties.get(name)).map(String::as_str)
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

    pub(super) fn state_mut(&mut self, target: &str) -> Option<&mut ElementState> {
        self.states.iter_mut().find(|state| state.target == target)
    }

    pub(super) fn clear_effects(&mut self) {
        for state in &mut self.states {
            state.effect = None;
        }
    }
}

/// Every stop on a slide, in order.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepTimeline {
    pub(super) frames: Vec<StepFrame>,
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
                        // Content is a replacement rather than an addition, so
                        // the handout shows the value the slide ended on.
                        if state.content.is_some() {
                            existing.content = state.content.clone();
                        }
                        existing.properties.extend(state.properties.clone());
                    }
                    None => frame.states.push(ElementState {
                        target: state.target.clone(),
                        visibility: state.visibility,
                        content: state.content.clone(),
                        properties: state.properties.clone(),
                        effect: None,
                    }),
                }
            }
        }

        frame
    }
}
