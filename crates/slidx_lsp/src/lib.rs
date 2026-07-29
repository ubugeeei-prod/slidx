//! # slidx language server
//!
//! Everything the compiler already knows, delivered to where the author is
//! typing.
//!
//! The parser produces diagnostics, the linter produces findings with
//! remedies, and the mark, step, theme, and transition vocabularies are each
//! defined in exactly one place. None of it reached an editor before this
//! crate: a contrast failure surfaced at `vite build`, which is after the
//! author has stopped looking at the slide that caused it.
//!
//! ## What it serves
//!
//! Diagnostics, completion, document symbols, and hover — in that order,
//! because that is the order they repay the author.
//!
//! ## The server invents nothing
//!
//! Every diagnostic it publishes comes from [`slidx_core`] or [`slidx_lint`],
//! carrying their code, severity, and remedy. Every closed set it completes is
//! read from the Rust that defines it — [`slidx_theme::builtin`],
//! `EffectPreset::ALL`, `Transition::ALL` — never from a list restated here. A
//! language server with opinions of its own is a fourth place for the rules to
//! disagree.
//!
//! Open sets work the other way round. A mark's classes and keys belong to the
//! author, not to slidx, so those are harvested from the document being
//! edited. Closed sets come from Rust, open sets come from the deck, and
//! nothing is typed twice. See [`vocabulary`] and [`completion`].
//!
//! ## Three constraints worth stating
//!
//! **Positions.** LSP counts UTF-16 code units and Rust counts bytes. A deck
//! with Japanese in it lands every diagnostic in the wrong column if that
//! conversion is wrong. [`position`] is the only module allowed to make it.
//!
//! **Incremental.** Edits arrive as ranges and are applied to a buffer;
//! analysis is deferred until the input queue drains, so a burst of keystrokes
//! costs one parse rather than one per character. See [`analysis`] for what
//! that costs and what the next move would be.
//!
//! **A parse error must not blank the outline.** Parsing a deck never fails,
//! but a half-typed code fence swallows every separator below it and collapses
//! a deck into one slide. That state is detected rather than guessed at, and
//! the last trustworthy outline is served until it passes. See
//! [`analysis::Analysis::outline_is_trustworthy`].
//!
//! ## Running it
//!
//! The `slidx-lsp` binary speaks the base protocol over stdin and stdout. It
//! takes no arguments and reads no configuration: an editor starts it, and
//! everything else is protocol.
//!
//! ```
//! use slidx_lsp::{analyze, PositionEncoding};
//!
//! let analysis = analyze("---\ntitle: T\n---\n\n# Hello\n\n---\n\n# World\n");
//!
//! assert_eq!(analysis.deck.slides.len(), 2);
//! assert!(analysis.outline_is_trustworthy());
//! assert_eq!(PositionEncoding::default(), PositionEncoding::Utf16);
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod analysis;
pub mod completion;
pub mod diagnostics;
pub mod document;
pub mod hover;
pub mod position;
pub mod protocol;
pub mod server;
pub mod symbols;
pub mod vocabulary;

pub use analysis::{analyze, Analysis};
pub use completion::CompletionItem;
pub use diagnostics::LspDiagnostic;
pub use document::{DocumentStore, TextDocument};
pub use hover::Hover;
pub use position::{LineIndex, Position, PositionEncoding, Range};
pub use protocol::{Message, RequestId};
pub use server::Server;
pub use symbols::DocumentSymbol;

/// Name reported to the client at `initialize`.
pub const SERVER_NAME: &str = "slidx";

/// Value put in every published diagnostic's `source` field.
///
/// Editors group by it, so an author can tell a slidx finding from their
/// Markdown linter's without reading the code.
pub const DIAGNOSTIC_SOURCE: &str = "slidx";
