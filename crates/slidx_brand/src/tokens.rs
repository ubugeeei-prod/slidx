//! The brand as data, and the file the rest of the repository reads it from.
//!
//! # Why a committed JSON file
//!
//! Two consumers need these numbers and neither can call Rust: the
//! documentation site, which is TypeScript, and any hand-written page or
//! template that wants the signal colour. So the tokens are emitted to
//! `assets/brand/tokens.json` and committed, exactly the way
//! `crates/slidx_wasm/deck.d.ts` is — the generated file is the contract, and a
//! test fails when the committed copy stops describing what this module
//! produces. `vp run generate:brand` is how you fix that failure.
//!
//! Committing it rather than generating at build time is what lets a consumer
//! `import` it with no build step and lets a change to the brand arrive in
//! review as a readable diff.
//!
//! # Why the type scale is smaller than a deck's
//!
//! A slide is read from row fifteen and a documentation page from fifty
//! centimetres, and [`slidx_lint`]'s angular model is precisely the thing that
//! says those are different numbers. The *machinery* is shared — the same
//! [`TypeScale`], the same modular ratio, so there is no arbitrary size to
//! reach for on either side — and only the base differs.

use serde::{Deserialize, Serialize};
use slidx_theme::{Spacing, TypeScale};

use crate::mark::Geometry;
use crate::palette::{self, Palette, Scheme};
use crate::wordmark::Lockup;

/// Where the generated tokens live, relative to the workspace root.
///
/// Stated as a constant because it is a contract with consumers outside this
/// crate: the documentation site resolves this path, and a rename that only
/// happened in a script would break it silently.
pub const TOKENS_PATH: &str = "assets/brand/tokens.json";

/// The brand's type scale.
///
/// 17px base at a 1.25 ratio. The base is a reading size rather than a
/// projection size; the ratio is the default theme's, so a heading steps by the
/// same interval on the site as it does on a slide.
///
/// Code is set slightly below body — 0.94 — because the mono stack runs larger
/// on the metrics of the faces it names, and matching the *apparent* size is
/// what keeps a fenced line from towering over the sentence introducing it.
pub const TYPE_SCALE: TypeScale = TypeScale { base_px: 17.0, ratio: 1.25, code_factor: 0.94 };

/// The unit every space in the brand is a multiple of.
///
/// One number instead of a list, for the reason the type scale is a ratio
/// instead of a list: it removes the arbitrary value. A gap is two steps or
/// three, never 19 pixels.
pub const SPACE_STEP_PX: f64 = 8.0;

/// Brand spacing, in multiples of [`SPACE_STEP_PX`].
///
/// Flat: the radius is zero, and that is the same decision the built-in themes
/// make for the same reason. A radius and a shadow are the first things a
/// projector turns to mud, and `scripts/check-flat.mjs` fails if either
/// reappears anywhere the brand is drawn.
pub const SPACING: Spacing = Spacing {
    padding_px: SPACE_STEP_PX * 4.0,
    block_px: SPACE_STEP_PX * 3.0,
    radius_px: 0.0,
    hairline_px: 1.0,
};

/// The prose face.
///
/// The default deck theme's stack, read from it rather than repeated here. A
/// wordmark set in a face the themes do not name would be a brand that does not
/// match the product it is on — and a stack of its own would be a second place
/// to remember when a CJK fallback is added.
///
/// It is a *system* stack, which is the whole reason the wordmark is set type
/// rather than drawn letterforms: a logo needing a downloaded typeface would
/// break the one promise every other part of this repository keeps.
pub fn font_sans() -> String {
    slidx_theme::default_theme().font_sans
}

/// The mono face, from the same place and for the same reason.
pub fn font_mono() -> String {
    slidx_theme::default_theme().font_mono
}

/// Every brand token, in the shape consumers read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    /// The workspace version these tokens were generated at, so a consumer
    /// pinning an older copy can tell.
    pub version: String,
    pub color: Schemes,
    pub typography: Typography,
    pub space: Space,
    /// The mark's construction, so a consumer can lay the mark out without
    /// re-deriving the grid from the SVG.
    pub mark: Geometry,
    pub lockup: Lockup,
}

/// Both schemes, as hex strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schemes {
    pub light: Colors,
    pub dark: Colors,
}

/// One scheme's five colours.
///
/// Hex strings rather than [`slidx_lint::Rgba`]'s four channels: this file is
/// read by a stylesheet author and by a template, and `{"r":27,…}` is not a
/// value either of them can paste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Colors {
    pub paper: String,
    pub ink: String,
    pub muted: String,
    pub signal: String,
    pub line: String,
}

impl Colors {
    fn of(palette: &Palette) -> Self {
        Self {
            paper: palette.paper.to_hex(),
            ink: palette.ink.to_hex(),
            muted: palette.muted.to_hex(),
            signal: palette.signal.to_hex(),
            line: palette.line.to_hex(),
        }
    }
}

/// The scale, and every size derived from it.
///
/// The derived sizes are emitted as well as the base and the ratio. A consumer
/// that recomputed them would be a second implementation of the scale, and two
/// implementations eventually round differently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Typography {
    pub font_sans: String,
    pub font_mono: String,
    pub base_px: f64,
    pub ratio: f64,
    pub size_px: Sizes,
    /// Tracking for the wordmark and for headings, in em. Negative: at display
    /// sizes the default spacing of a system face reads loose.
    pub heading_tracking_em: f64,
    pub heading_weight: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sizes {
    pub heading1: f64,
    pub heading2: f64,
    pub heading3: f64,
    pub body: f64,
    pub code: f64,
    pub caption: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub step_px: f64,
    pub padding_px: f64,
    pub block_px: f64,
    /// Zero, and checked. See [`SPACING`].
    pub radius_px: f64,
    pub hairline_px: f64,
}

/// Every token, resolved.
pub fn tokens() -> Tokens {
    let round = |value: f64| (value * 100.0).round() / 100.0;

    Tokens {
        version: env!("CARGO_PKG_VERSION").to_string(),
        color: Schemes {
            light: Colors::of(&palette::of(Scheme::Light)),
            dark: Colors::of(&palette::of(Scheme::Dark)),
        },
        typography: Typography {
            font_sans: font_sans(),
            font_mono: font_mono(),
            base_px: TYPE_SCALE.base_px,
            ratio: TYPE_SCALE.ratio,
            size_px: Sizes {
                heading1: round(TYPE_SCALE.heading_px(1)),
                heading2: round(TYPE_SCALE.heading_px(2)),
                heading3: round(TYPE_SCALE.heading_px(3)),
                body: round(TYPE_SCALE.body_px()),
                code: round(TYPE_SCALE.code_px()),
                caption: round(TYPE_SCALE.caption_px()),
            },
            heading_tracking_em: -0.02,
            heading_weight: 650,
        },
        space: Space {
            step_px: SPACE_STEP_PX,
            padding_px: SPACING.padding_px,
            block_px: SPACING.block_px,
            radius_px: SPACING.radius_px,
            hairline_px: SPACING.hairline_px,
        },
        mark: Geometry::default(),
        lockup: Lockup::default(),
    }
}

/// The tokens as the committed file spells them.
pub fn render_json() -> String {
    let mut json = serde_json::to_string_pretty(&tokens()).expect("brand tokens serialise");
    json.push('\n');
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generated_tokens_are_json_a_consumer_can_parse() {
        // The committed copy is compared against this in `crate::assets`. What
        // matters here is that what we emit loads at all: the file is imported
        // by a TypeScript build with no schema in front of it, so a malformed
        // number would surface as a runtime failure on the documentation site.
        let json = render_json();
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("the generated tokens parse");

        assert!(value.get("color").is_some());
        assert!(value.get("typography").is_some());
        assert!(json.ends_with('\n'), "a text file ends in a newline");
    }

    #[test]
    fn the_brand_and_the_default_theme_name_the_same_faces() {
        // One system, not two. A wordmark in a face the deck themes do not have
        // is a brand that does not match its own product.
        let theme = slidx_theme::default_theme();

        assert_eq!(font_sans(), theme.font_sans);
        assert_eq!(font_mono(), theme.font_mono);
    }

    #[test]
    fn the_brand_names_no_font_it_would_have_to_download() {
        // The promise the whole repository keeps. A logo that needed a webfont
        // would break it in the most visible place there is.
        for stack in [font_sans(), font_mono()] {
            assert!(!stack.contains("http"), "the brand reaches for a remote font");
            assert!(!stack.contains("url("), "the brand reaches for a remote font");
        }
    }

    #[test]
    fn every_space_is_a_multiple_of_one_step() {
        let space = tokens().space;

        for value in [space.padding_px, space.block_px] {
            assert_eq!(value % SPACE_STEP_PX, 0.0, "{value} is not a multiple of the step");
        }
    }

    #[test]
    fn the_brand_is_flat() {
        // Stated as a number rather than only as prose, so a radius introduced
        // later fails here as well as in the repository-wide check.
        assert_eq!(tokens().space.radius_px, 0.0);
    }

    #[test]
    fn sizes_descend_from_the_largest_heading_to_the_caption() {
        let sizes = tokens().typography.size_px;
        let ordered = [sizes.heading1, sizes.heading2, sizes.heading3, sizes.body, sizes.caption];

        for pair in ordered.windows(2) {
            assert!(pair[0] > pair[1], "sizes must descend: {ordered:?}");
        }
    }

    #[test]
    fn the_derived_sizes_are_the_scale_rather_than_a_second_list() {
        let sizes = tokens().typography.size_px;

        assert_eq!(sizes.body, TYPE_SCALE.base_px);
        assert!((sizes.heading1 - TYPE_SCALE.base_px * TYPE_SCALE.ratio.powi(3)).abs() < 0.01);
    }

    #[test]
    fn colours_are_emitted_as_something_a_stylesheet_can_paste() {
        let light = tokens().color.light;

        assert_eq!(light.signal, "#1b3bc9");
        assert!(light.paper.starts_with('#') && light.paper.len() == 7);
    }

    #[test]
    fn the_tokens_round_trip() {
        let json = render_json();
        let parsed: Tokens = serde_json::from_str(&json).expect("tokens parse back");

        assert_eq!(parsed, tokens());
    }
}
