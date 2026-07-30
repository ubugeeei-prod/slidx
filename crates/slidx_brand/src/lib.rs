//! # slidx brand
//!
//! The mark, the wordmark, the palette, and the tokens the rest of the
//! repository draws itself with.
//!
//! ## The brand is checked, not asserted
//!
//! This repository's habit is to turn a promise into a check: the offline
//! guarantee is a lint rule, the size guideline is a script, a theme is held to
//! the linter by [`slidx_theme::audit`]. A brand is usually the one part of a
//! project that escapes that and lives in a PDF nobody opens, so it does not
//! escape here.
//!
//! Three promises, three checks:
//!
//! - **The colours pass slidx's own linter**, projector model included, in every
//!   room it models. [`audit`] runs them. The obvious blue for this palette was
//!   the default theme's accent and it measures 4.46:1 on brand paper in a
//!   bright room — a fail, on the exact check slidx exists to run. That is why
//!   the signal is a stop deeper, and it is the reason the audit is not
//!   decoration.
//! - **Nothing is flat by convention.** No shadow, no gradient, anywhere the
//!   brand or a deck is drawn. `scripts/check-flat.mjs` fails on either.
//! - **Nothing is exported by hand.** Every file under `assets/brand/` is
//!   generated from the constants here and compared against the committed copy
//!   by [`assets`], so an asset that stopped being true fails to reproduce.
//!
//! ## One system with the deck themes
//!
//! The type scale is [`slidx_theme::TypeScale`] and the spacing is
//! [`slidx_theme::Spacing`] — the same structs, the same modular ratio — and the
//! font stacks are read off the default theme rather than repeated. Only the
//! base size differs, because a documentation page is read at fifty centimetres
//! and a slide from row fifteen, and [`slidx_lint`]'s angular model is precisely
//! the thing that says those are different numbers.
//!
//! ```
//! use slidx_brand::{audit, mark, palette::Scheme, tokens};
//!
//! assert!(audit::audit_every_room().is_empty());
//! assert!(mark::render(Scheme::Light).contains("<rect"));
//! assert_eq!(tokens::TOKENS_PATH, "assets/brand/tokens.json");
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod assets;
pub mod audit;
pub mod css;

pub mod mark;
pub mod palette;
pub mod tokens;
pub mod wordmark;

pub use mark::Geometry;
pub use palette::{Palette, Scheme};
pub use tokens::{Tokens, TOKENS_PATH};
pub use wordmark::{Lockup, WORDMARK};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_brand_resolves_both_schemes() {
        assert_ne!(palette::of(Scheme::Light), palette::of(Scheme::Dark));
    }

    #[test]
    fn the_published_tokens_path_is_where_the_file_is() {
        let path = assets::workspace_root().join(TOKENS_PATH);
        assert!(path.exists(), "{} does not exist", path.display());
    }
}
