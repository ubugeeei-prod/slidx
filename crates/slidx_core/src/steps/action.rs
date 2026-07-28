//! The authored half of the step pipeline.
//!
//! A [`StepSource`] is exactly what the author wrote — a flat, order-preserving
//! list of intents. It is deliberately free of resolved state so that the same
//! source can be recompiled after an edit without replaying history.

use serde::{Deserialize, Serialize};

use super::preset::{Easing, EffectKind, EffectPreset, Origin};

/// Default animation length, in milliseconds.
///
/// Short enough that a fast presenter never waits on the tool, long enough to
/// read as intentional motion from the back of a room.
pub const DEFAULT_DURATION_MS: u32 = 400;

/// Tuning shared by every action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StepOptions {
    /// Milliseconds to wait before playing. `Some` also means the action plays
    /// automatically instead of consuming a click.
    pub after: Option<u32>,
    pub preset: Option<EffectPreset>,
    pub duration: u32,
    pub easing: Easing,
    pub origin: Option<Origin>,
}

impl Default for StepOptions {
    fn default() -> Self {
        Self {
            after: None,
            preset: None,
            duration: DEFAULT_DURATION_MS,
            easing: Easing::default(),
            origin: None,
        }
    }
}

impl StepOptions {
    /// Resolves the effect this action contributes to a frame.
    pub fn resolve(&self, kind: EffectKind) -> Effect {
        Effect {
            kind,
            preset: self.preset.unwrap_or_else(|| EffectPreset::default_for(kind)),
            duration_ms: self.duration,
            delay_ms: self.after.unwrap_or(0),
            easing: self.easing,
            origin: self.origin,
        }
    }
}

/// A resolved animation attached to one element in one frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub kind: EffectKind,
    pub preset: EffectPreset,
    pub duration_ms: u32,
    pub delay_ms: u32,
    pub easing: Easing,
    pub origin: Option<Origin>,
}

impl Default for Effect {
    fn default() -> Self {
        Self {
            kind: EffectKind::default(),
            preset: EffectPreset::default(),
            duration_ms: DEFAULT_DURATION_MS,
            delay_ms: 0,
            easing: Easing::default(),
            origin: None,
        }
    }
}

/// Whether an element is painted in a given frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Present in the layout but not painted, so revealing never reflows.
    Hidden,
    Visible,
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
            | Self::Group { options, .. } => options,
        }
    }

    fn options_mut(&mut self) -> &mut StepOptions {
        match self {
            Self::Reveal { options, .. }
            | Self::Hide { options, .. }
            | Self::Emphasize { options, .. }
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
            | Self::Emphasize { target, .. } => vec![target.as_str()],
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
