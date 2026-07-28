//! # slidx render
//!
//! Markdown to HTML, on the [Ox Content] engine.
//!
//! slidx does not implement Markdown. Ox Content's arena-allocated, zero-copy
//! CommonMark and GFM parser does the work, and this crate adds only what a
//! slide needs on top of it.
//!
//! [Ox Content]: https://github.com/ubugeeei-prod/ox-content
//!
//! ## The seam that matters
//!
//! Step anchors — empty `<span data-slidx-step="N" hidden>` elements planted
//! by [`slidx_core::markers`] — have to reach the output intact and in the
//! right position, because the entire step pipeline hangs off them. Nothing
//! here can verify that by inspection, so it is asserted end to end: core
//! compiles a marker, this crate renders it, and the test checks the anchor
//! came out where the runtime's contract expects to find it.
//!
//! ```
//! use slidx_core::{parse_deck, DeckParseOptions};
//! use slidx_render::{render_markdown, MarkdownOptions};
//!
//! let deck = parse_deck("# Agenda\n\n- one <!-- step -->\n", &DeckParseOptions::default());
//! let html = render_markdown(&deck.slides[0].content, &MarkdownOptions::default());
//!
//! // The anchor closes inside its list item, which is what tells the runtime
//! // to stage that bullet rather than the whole list.
//! assert!(html.contains(r#"<span data-slidx-step="1" hidden></span></li>"#));
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod layout;
pub mod markdown;
pub mod presenter;
pub mod presenter_layout;
pub mod print;
pub mod print_layout;
pub mod shell;

pub use markdown::{render as render_markdown, MarkdownOptions};
pub use presenter::{render_presenter, PresenterOptions};
pub use print::{render_print, PrintOptions};
pub use shell::{render_slide, ShellOptions};
