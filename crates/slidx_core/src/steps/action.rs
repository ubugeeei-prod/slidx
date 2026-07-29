//! The authored half of the step pipeline.
//!
//! A [`StepSource`] is exactly what the author wrote — a flat, order-preserving
//! list of intents. It is deliberately free of resolved state so that the same
//! source can be recompiled after an edit without replaying history.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::preset::{EffectPreset, Origin};
use super::timing::StepOptions;

/// Whether an element is painted in a given frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Present in the layout but not painted, so revealing never reflows.
    Hidden,
    Visible,
}

/// A change to an element that is already on screen.
///
/// Reveal and hide cover "not there yet" and "gone". This covers the third
/// thing a presenter does, which is to change something the audience is
/// already looking at — a number that updates, a label that turns red, a line
/// of code that becomes the focus.
///
/// It is the counterpart of a mark: a mark names a range, a patch says what
/// that range becomes. Absent fields mean "leave alone", so a patch that only
/// changes colour does not have to restate the text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    /// Replaces the element's text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Data properties to set. Sorted, so a patch serialises canonically and
    /// the editor never produces a diff nobody asked for.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

impl Patch {
    pub fn content(text: impl Into<String>) -> Self {
        Self { content: Some(text.into()), ..Self::default() }
    }

    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_none() && self.properties.is_empty()
    }
}

/// One authored intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepAction {
    Reveal {
        target: String,
        options: StepOptions,
    },
    Hide {
        target: String,
        options: StepOptions,
    },
    Emphasize {
        target: String,
        options: StepOptions,
    },
    /// Changes an element that is already visible, in place.
    Set {
        target: String,
        patch: Patch,
        options: StepOptions,
    },
    /// Several intents that land on the same click.
    Group {
        actions: Vec<StepAction>,
        options: StepOptions,
    },
}

impl StepAction {
    pub fn reveal(target: impl Into<String>) -> Self {
        Self::Reveal { target: target.into(), options: StepOptions::default() }
    }

    pub fn hide(target: impl Into<String>) -> Self {
        Self::Hide { target: target.into(), options: StepOptions::default() }
    }

    pub fn emphasize(target: impl Into<String>, preset: EffectPreset) -> Self {
        Self::Emphasize {
            target: target.into(),
            options: StepOptions { preset: Some(preset), ..StepOptions::default() },
        }
    }

    pub fn set(target: impl Into<String>, patch: Patch) -> Self {
        Self::Set { target: target.into(), patch, options: StepOptions::default() }
    }

    pub fn group(actions: Vec<StepAction>) -> Self {
        Self::Group { actions, options: StepOptions::default() }
    }

    /// Plays this action automatically `ms` after the frame it belongs to,
    /// instead of waiting for the presenter to advance.
    pub fn after_ms(mut self, ms: u32) -> Self {
        self.options_mut().after = Some(ms);
        self
    }

    /// Overrides the animation used by this action.
    pub fn with_preset(mut self, preset: EffectPreset) -> Self {
        self.options_mut().preset = Some(preset);
        self
    }

    /// Overrides the animation length, in milliseconds.
    pub fn with_duration(mut self, ms: u32) -> Self {
        self.options_mut().duration = ms;
        self
    }

    /// Sets the direction the effect travels from or towards.
    pub fn with_origin(mut self, origin: Origin) -> Self {
        self.options_mut().origin = Some(origin);
        self
    }

    pub fn options(&self) -> &StepOptions {
        match self {
            Self::Reveal { options, .. }
            | Self::Hide { options, .. }
            | Self::Emphasize { options, .. }
            | Self::Set { options, .. }
            | Self::Group { options, .. } => options,
        }
    }

    fn options_mut(&mut self) -> &mut StepOptions {
        match self {
            Self::Reveal { options, .. }
            | Self::Hide { options, .. }
            | Self::Emphasize { options, .. }
            | Self::Set { options, .. }
            | Self::Group { options, .. } => options,
        }
    }

    /// True when the action plays on a timer rather than on a click.
    pub fn is_auto(&self) -> bool {
        self.options().after.is_some()
    }

    /// Every selector this action touches, including nested group members.
    pub fn targets(&self) -> Vec<&str> {
        match self {
            Self::Reveal { target, .. }
            | Self::Hide { target, .. }
            | Self::Emphasize { target, .. }
            | Self::Set { target, .. } => vec![target.as_str()],
            Self::Group { actions, .. } => actions.iter().flat_map(Self::targets).collect(),
        }
    }
}

/// Automatic staging derived from slide structure rather than explicit actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutoSteps {
    /// Reveal top-level list items one at a time.
    List,
    /// Reveal every top-level block one at a time.
    Block,
    /// Reveal table rows one at a time.
    Row,
}

impl AutoSteps {
    pub fn as_token(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Block => "block",
            Self::Row => "row",
        }
    }
}

/// Everything the author declared about how a slide advances.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepSource {
    pub actions: Vec<StepAction>,
    pub auto: Option<AutoSteps>,
}

impl StepSource {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_compose() {
        let action = StepAction::reveal(".a")
            .with_preset(EffectPreset::Zoom)
            .with_duration(900)
            .with_origin(Origin::Bottom)
            .after_ms(120);

        let options = action.options();
        assert_eq!(options.preset, Some(EffectPreset::Zoom));
        assert_eq!(options.duration, 900);
        assert_eq!(options.origin, Some(Origin::Bottom));
        assert_eq!(options.after, Some(120));
        assert!(action.is_auto());
    }
}
