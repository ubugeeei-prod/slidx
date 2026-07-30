//! # slidx core
//!
//! The deck model and the pipeline that produces it.
//!
//! Everything in slidx that reads a talk goes through here: the Vite plugin,
//! the visual editor, the presenter view, the PDF exporter, and the linter all
//! consume the same [`Deck`]. That is deliberate — the failure mode of
//! presentation tooling is that the editor, the projector, and the exported
//! handout quietly disagree, and the only durable fix is to give them one
//! parser and one execution model.
//!
//! ## Pipeline
//!
//! ```text
//! source ──▶ segment ──▶ frontmatter ──▶ notes ──▶ markers ──▶ steps ──▶ Deck
//! ```
//!
//! Each stage is a pure function, so a changed file re-parses on its own and
//! nothing downstream needs invalidating.
//!
//! ## Two guarantees
//!
//! **Parsing never fails.** Decks get edited minutes before a talk starts. A
//! malformed line produces a [`Diagnostic`] and a slide that still renders,
//! never an error that leaves a speaker with nothing to present.
//!
//! **Steps are snapshots, not deltas.** A slide's [`StepTimeline`] is a vector
//! of complete states. Advancing, going back, deep-linking to a step, and
//! printing all index into that vector, so they cannot drift apart.
//!
//! ## Example
//!
//! ```
//! use slidx_core::{parse_deck, DeckParseOptions};
//!
//! let deck = parse_deck(
//!     "---\ntitle: Fast Decks\nduration: 20m\n---\n\n# Hello\n\n- one <!-- step -->\n",
//!     &DeckParseOptions::default(),
//! );
//!
//! assert_eq!(deck.meta.title.as_deref(), Some("Fast Decks"));
//! assert_eq!(deck.meta.duration_seconds, Some(1200));
//! assert_eq!(deck.slides[0].stop_count(), 2);
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod attributes;
pub mod block;
pub mod camera;
pub mod demo;
pub mod diagnostic;
pub mod frontmatter;
pub mod grid;
pub mod mark;
pub mod markers;
pub mod model;
pub mod notes;
pub mod parser;
pub mod scanner;
pub mod slug;
pub mod span;
pub mod steps;
pub mod style;
pub mod summary;

pub use attributes::Attributes;
pub use block::{extract_blocks, find_blocks, Block, ExtractedBlocks, FoundBlock};
pub use camera::{Camera, CAMERA_ATTRIBUTE, CAMERA_STATE_ATTRIBUTE};
pub use demo::{Demo, DEMO_ATTRIBUTE};
pub use diagnostic::{Diagnostic, Diagnostics, Severity, SourceSpan};
pub use grid::{step_grid, StepGrid, StepKind, StepPlacement, StepRow};
pub use mark::{compile_marks, find_marks, Mark, MARK_ATTRIBUTE};
pub use markers::{anchor_selector, StagedContent, ANCHOR_ATTRIBUTE};
pub use model::{estimate_speaking_seconds, AspectRatio, Deck, DeckMeta, Slide, TalkMeta};
pub use notes::{extract_notes, find_notes, ExtractedNotes, FoundNote};
pub use parser::{parse_deck, DeckParseOptions};
pub use slug::{slugify, SlugAllocator};
pub use span::ByteSpan;
pub use steps::{
    compile_timeline, AutoSteps, Easing, Effect, EffectKind, EffectPreset, ElementState, Origin,
    StepAction, StepFrame, StepOptions, StepSource, StepTimeline, Visibility,
};
pub use style::{extract_style, find_styles, ExtractedStyle, FoundStyle};
pub use summary::Summary;

/// The version of the deck format this build understands.
///
/// Written into generated artefacts so a stale build output can be detected
/// rather than silently mixed with a newer one.
pub const FORMAT_VERSION: u32 = 1;
