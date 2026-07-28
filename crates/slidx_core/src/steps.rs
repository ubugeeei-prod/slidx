//! Declarative slide advancement.
//!
//! Authors describe *what each stop looks like*, not *what to do on click*.
//! The split between modules follows the questions they answer:
//!
//! | Module | Question |
//! |---|---|
//! | [`action`] | what happens to an element |
//! | [`timing`] | how long it takes and what it looks like |
//! | [`preset`] | the shared animation vocabulary |
//! | [`parse`] | how to recognise an action in what an author wrote |
//! | [`compile`] | how a list of intents becomes a list of stops |
//! | [`timeline`] | what a stop *is* |

pub mod action;
pub mod compile;
pub mod parse;
pub mod preset;
pub mod timeline;
pub mod timing;

pub use action::{AutoSteps, Patch, StepAction, StepSource, Visibility};
pub use compile::compile_timeline;
pub use parse::parse_step_actions;
pub use preset::{Easing, EffectKind, EffectPreset, Origin};
pub use timeline::{ElementState, StepFrame, StepTimeline};
pub use timing::{Effect, StepOptions, DEFAULT_DURATION_MS};
