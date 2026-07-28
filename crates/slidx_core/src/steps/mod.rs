//! Declarative slide advancement.
//!
//! Authors describe *what each stop looks like*, not *what to do on click*.
//! [`action`] holds the authored intents, [`timeline`] compiles them into
//! snapshots, and [`preset`] is the shared animation vocabulary.

pub mod action;
pub mod preset;
pub mod timeline;

pub use action::{
    parse_step_actions, AutoSteps, Effect, StepAction, StepOptions, StepSource, Visibility,
    DEFAULT_DURATION_MS,
};
pub use preset::{Easing, EffectKind, EffectPreset, Origin};
pub use timeline::{compile_timeline, ElementState, StepFrame, StepTimeline};
