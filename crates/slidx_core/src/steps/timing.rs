//! How a step is timed and animated.
//!
//! Separate from the action model because they change for different reasons:
//! an action says *what* happens to an element, this says how long it takes,
//! what it looks like, and whether it waits for a click. Authors reach for one
//! without touching the other, and so does the editor's timeline panel.

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
