//! # slidx themes
//!
//! Theme tokens, the four built-in themes, and the CSS they compile to.
//!
//! ## Themes are linted, not trusted
//!
//! Most contrast and legibility problems originate in a theme rather than in a
//! slide: the author inherits them and never sees a colour value. So a theme
//! describes itself to the linter as a list of [`slidx_lint::Surface`]s and is
//! held to the same rules as a deck. Every built-in theme passes those rules in
//! every room slidx models — that is asserted in [`audit`], and it is a gate,
//! not a report.
//!
//! The same audit runs over third-party theme packages, which is the reason a
//! theme is described as resolved surfaces rather than as CSS. See [`package`]
//! for what one is, how a name resolves to it, and what a document arriving
//! from a registry is not allowed to do.
//!
//! ## Sizes are relative, deliberately
//!
//! Sizes come from a modular scale and compile to container-height units, so a
//! slide scales as one piece. There is no arbitrary pixel value for an author
//! to reach for, which makes "shrink the text until it fits" — the reflex that
//! produces unreadable slides — something the system does not offer.
//!
//! ## And so is everything else about setting the type
//!
//! A scale answers how large and nothing else. How far apart the lines sit, how
//! tightly the letters do, and how long a line may run were constants in the
//! shell stylesheet — one set, applied to every theme and to every script. See
//! [`typography`] for the curves that replaced them, and for why a Japanese
//! heading was breaking after ten characters.
//!
//! ## A layout is a named set of regions
//!
//! `layout: aside` is a grid the theme owns, and `{.side}` on a block is an
//! author choosing one of its regions. That is the whole difference between a
//! placement a reviewer can read and four floats in a file: the geometry belongs
//! to the layout, so it survives a 4:3 projector and a rule can measure it. See
//! [`layout`].
//!
//! ## Motion is opt-in and reversible
//!
//! Slide-to-slide transitions are CSS the browser runs across a real
//! navigation, so a deck keeps one document per slide and still animates
//! between them. A deck gets none unless it asks, and whatever it asks for is
//! cancelled for a viewer who prefers reduced motion. See [`transition`].
//!
//! ```
//! use slidx_lint::LintOptions;
//! use slidx_theme::{audit, builtin, css, transition, Transition};
//!
//! let theme = builtin::contrast();
//! assert!(audit::audit(&theme, &LintOptions::default()).is_empty());
//! assert!(css::render(&theme).contains("--slidx-color-text:"));
//! assert!(transition::render(&theme, Transition::Push).contains("@view-transition"));
//! assert!(transition::render(&theme, Transition::None).is_empty());
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod audit;
pub mod builtin;
pub mod css;
pub mod layout;
pub mod mix;
pub mod package;
pub mod palette;
pub mod published;
pub mod scale;
pub mod theme;
pub mod transition;
pub mod typography;

pub use layout::{Layout, Region, RegionAlign};
pub use package::{Catalogue, Published, Resolved};
pub use palette::{Palette, Scheme};
pub use scale::{TypeScale, REFERENCE_HEIGHT_PX};
pub use theme::{Motion, Spacing, Theme, REDUCED_MOTION_CEILING_MS};
pub use transition::Transition;
pub use typography::{Script, Typography};

/// Resolves a theme by name, against the built-ins alone.
///
/// This is the answer for a caller that has no project to read — the language
/// server on a lone file, a rule checking a name. A caller holding the
/// documents a project installed wants [`Catalogue::resolve`], which tries the
/// built-ins first and these same ids win there too.
///
/// Returning `None` rather than falling back silently means a typo in `theme:`
/// is reported instead of producing a deck that looks subtly wrong.
pub fn resolve(id: &str) -> Option<Theme> {
    builtin::find(id)
}

/// The theme used when a deck names none.
pub fn default_theme() -> Theme {
    builtin::minimal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_ids_resolve() {
        assert!(resolve("minimal").is_some());
        assert!(resolve("contrast").is_some());
    }

    #[test]
    fn an_unknown_id_does_not_silently_fall_back() {
        assert!(resolve("editoral").is_none(), "a typo must be reported, not absorbed");
    }

    #[test]
    fn the_default_theme_is_a_built_in() {
        assert!(resolve(&default_theme().id).is_some());
    }
}
