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

pub mod highlight;
pub mod layout;
pub mod markdown;
pub mod og;
pub mod presenter;
pub mod presenter_layout;
mod presenter_script;
pub mod print;
pub mod print_layout;
pub mod qr;
pub mod region;
pub mod seo;
pub mod shell;
pub mod snippet;
pub mod url;

pub use highlight::highlight_code_blocks;
pub use markdown::{render as render_markdown, MarkdownOptions};
pub use og::{render_deck_card, render_slide_card, OgOptions, OG_HEIGHT, OG_WIDTH};
pub use presenter::{render_presenter, PresenterOptions};
pub use print::{render_print, PrintOptions};
pub use qr::{render_qr, SlideQrOptions};
pub use region::{layout_of, BLOCK_ATTRIBUTE};
pub use seo::{describe, render_robots, render_sitemap, SeoOptions};
pub use shell::{render_slide, ShellOptions};
pub use snippet::{
    collect as collect_snippets, render_snippet, render_snippets, Snippet, SnippetOptions,
    SnippetPage, SNIPPET_DIR,
};
