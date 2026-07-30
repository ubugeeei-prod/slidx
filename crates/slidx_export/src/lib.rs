//! # slidx_export
//!
//! Turning what a build produced into one file somebody can hand over.
//!
//! **Nothing here renders a deck.** The pages, the PDF, and the images all come
//! from `@slidx/vite-plugin` driving a real browser over the emitted print
//! shell; this crate takes those files and puts them in a container a
//! conference form, a review panel, or Google Slides will accept. That
//! separation is the whole design: a second renderer would mean the artefact a
//! speaker hands over could differ from the one they checked, which is the one
//! failure this project spends most of its architecture avoiding.
//!
//! What is here is therefore only the two things a build cannot do for itself:
//! [`zip`], because a deck's output is a directory and an attachment is a file,
//! and the target list in [`target`], because the CLI, its help text and its
//! shell completions all have to agree about which exports exist.
//!
//! ## Determinism
//!
//! No clock and no filesystem. Archive entries carry a fixed timestamp, so
//! exporting the same build twice produces the same bytes — which is what lets
//! a CI job cache one, and what makes a diff of two exports mean something.

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod target;
pub mod zip;

pub use target::{ExportTarget, Frame, EXPORT_TARGETS, FRAME_DIRECTORY};
pub use zip::{names, write, Entry};
