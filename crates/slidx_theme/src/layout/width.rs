//! How much of its region a block takes.
//!
//! `place` answers which region a block is in. This answers the other half of
//! "make it smaller", and it is the half where the design could quietly break.
//!
//! # Why not a width
//!
//! A resize handle wants to write a number, and `width=340px` throws away all
//! three things a region bought:
//!
//! - **Nobody can review it.** `{.side}` in a diff says where a block went.
//!   `340px` says nothing a reader can check against the slide they remember.
//! - **It means something else at another aspect ratio.** A slide is a design box
//!   scaled to whatever the projector is. 340 of what — of 1280, of 1024, of the
//!   4:3 the venue actually has? The number is right on exactly one screen.
//! - **No rule can reason about it.** The overflow rule needs to know whether a
//!   line of text will still be legible in the box, and the legibility model is
//!   angular size: it needs the box as a *share of the slide*, which is what a
//!   `fr` track gives it and what a pixel does not.
//!
//! # What a size is instead
//!
//! Content-sized by default, or a **share of the region** from a closed set the
//! theme names: `{width=half}`. Every property those three complaints ask for
//! holds:
//!
//! - `half` in a diff says what happened to the slide.
//! - A half is a half at 16:9 and at 4:3, because the region is already a share
//!   of the slide and this is a share of the region.
//! - The chain of shares closes arithmetically. A block at `half` of `side` in
//!   `aside` is `1/3 × 1/2` of the safe area, so the box a legibility or overflow
//!   rule needs is a number both the linter and the editor can compute — without
//!   either of them knowing what a projector is.
//!
//! `Fit` is the default and is written by *removing* the property, the same rule
//! [`super::place`] holds for the default region: a block that says nothing only
//! takes the width its content needs. `Full` is therefore deliberate and stays
//! visible as `{width=full}` in the file and `data-slidx-width="full"` on the
//! rendered block.
//!
//! # The gesture that is not here
//!
//! Making a block *wider* than its region. That is not a size at all — it is a
//! block in a different region, or a layout with a different grid, and both are
//! already sayable. A `span=2` reaching across the layout's columns would need
//! the block to be a child of the slide's grid rather than of a region, which
//! would give one block two answers about where it is.

use serde::{Deserialize, Serialize};
use slidx_core::Block;

/// The property an author writes: `{width=half}`.
pub const WIDTH_PROPERTY: &str = "width";

/// The attribute the renderer writes onto a block that asked for a share.
pub const WIDTH_ATTRIBUTE: &str = "data-slidx-width";

/// How much of its region a block takes.
///
/// A closed set rather than a fraction an author writes, because these are what
/// the layouts' own tracks are made of: halves, thirds and quarters are the cuts
/// a grid of `1fr 1fr` and `2fr 1fr` already makes, so a block sized this way
/// lines up with the region beside it instead of nearly lining up with it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockWidth {
    /// The content's intrinsic width, capped by the region.
    #[default]
    Fit,
    /// The whole region, written explicitly.
    Full,
    ThreeQuarters,
    TwoThirds,
    Half,
    Third,
    Quarter,
}

impl BlockWidth {
    /// Every width the model accepts, with the canonical default first.
    pub const ALL: &'static [Self] = &[
        Self::Fit,
        Self::Full,
        Self::ThreeQuarters,
        Self::TwoThirds,
        Self::Half,
        Self::Third,
        Self::Quarter,
    ];

    /// The fixed shares, widest first, which is the order a handle steps through.
    pub const SHARES: &'static [Self] =
        &[Self::Full, Self::ThreeQuarters, Self::TwoThirds, Self::Half, Self::Third, Self::Quarter];

    /// The name an author writes and a diff shows.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Fit => "fit",
            Self::Full => "full",
            Self::ThreeQuarters => "three-quarters",
            Self::TwoThirds => "two-thirds",
            Self::Half => "half",
            Self::Third => "third",
            Self::Quarter => "quarter",
        }
    }

    /// How much of the region, as a fraction, or nothing for intrinsic width.
    ///
    /// This is the number a rule reasons with, and the only place it is written
    /// down: the CSS below is generated from it, so the box the browser lays out
    /// and the box the linter models cannot disagree.
    pub fn share(self) -> Option<f64> {
        match self {
            Self::Fit => None,
            Self::Full => Some(1.0),
            Self::ThreeQuarters => Some(0.75),
            Self::TwoThirds => Some(2.0 / 3.0),
            Self::Half => Some(0.5),
            Self::Third => Some(1.0 / 3.0),
            Self::Quarter => Some(0.25),
        }
    }

    /// The width an author's word names, or nothing.
    ///
    /// `None` rather than a silent fallback, for the same reason a layout name
    /// resolves that way: `width=340px` has to be reported, not absorbed into a
    /// slide that looks subtly unlike what was asked for.
    pub fn find(token: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|width| width.as_token() == token)
    }

    /// Names an author may write, for a diagnostic and for completion.
    pub fn names() -> Vec<&'static str> {
        Self::ALL.iter().map(|width| width.as_token()).collect()
    }
}

/// The share a block asks for, and whether its word meant anything.
///
/// `Err` carries the word as written so a diagnostic can quote it. A block whose
/// width does not resolve still renders at the safe default, because a slide
/// that lost a block over a typo is worse than one that ignores the typo.
pub fn of(block: &Block) -> Result<BlockWidth, &str> {
    let Some(written) = block.attributes.properties.get(WIDTH_PROPERTY) else {
        return Ok(BlockWidth::Fit);
    };

    BlockWidth::find(written).ok_or(written.as_str())
}

/// Every share, as rules to inline into a page.
///
/// Percentages of the region rather than lengths, so the share survives the
/// slide being scaled — which is the whole point of writing a name instead of a
/// number. Centred, because a narrowed block in a region whose content is
/// already centred should stay on the axis the rest of the slide is on; an
/// author who wants it against an edge is asking for a region, and regions are
/// what `place` is for.
pub fn css() -> String {
    let mut css = String::new();

    for width in BlockWidth::SHARES {
        css.push_str(&format!(
            ".slidx-block[{WIDTH_ATTRIBUTE}=\"{token}\"] \
             {{ width: {percent:.4}%; margin-inline: auto; }}\n",
            token = width.as_token(),
            percent = width.share().expect("SHARES only contains fixed shares") * 100.0,
        ));
    }

    css
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::Attributes;

    fn block(attributes: Attributes) -> Block {
        Block { span: Default::default(), attributes }
    }

    #[test]
    fn a_block_that_says_nothing_about_width_fits_its_content() {
        assert_eq!(BlockWidth::default(), BlockWidth::Fit);
        assert_eq!(of(&block(Attributes::default())), Ok(BlockWidth::Fit));
    }

    #[test]
    fn a_block_that_names_a_share_gets_it() {
        let attributed = block(Attributes::default().with_property("width", "two-thirds"));
        assert_eq!(of(&attributed), Ok(BlockWidth::TwoThirds));
    }

    #[test]
    fn a_length_is_not_a_share_and_is_reported_rather_than_absorbed() {
        // The one thing this vocabulary exists to refuse. A pixel is right on
        // one screen, unreviewable in a diff, and opaque to every rule.
        assert_eq!(of(&block(Attributes::default().with_property("width", "340px"))), Err("340px"));
        assert_eq!(of(&block(Attributes::default().with_property("width", "50%"))), Err("50%"));
    }

    #[test]
    fn every_width_has_a_name_and_every_name_resolves_to_the_width_it_says() {
        for width in BlockWidth::ALL {
            assert_eq!(BlockWidth::find(width.as_token()), Some(*width));
        }
    }

    #[test]
    fn the_names_are_the_ones_the_editor_understands_and_the_diagnostic_lists() {
        // Pinned rather than derived, because the editor mirrors this list to
        // draw its snap targets. Drift is safe in both directions — a name only
        // one side knows is a snap target nobody can reach or a value the
        // linter reports — but it should arrive in review as a visible diff.
        assert_eq!(
            BlockWidth::names(),
            ["fit", "full", "three-quarters", "two-thirds", "half", "third", "quarter"]
        );
    }

    #[test]
    fn the_shares_run_from_widest_to_narrowest() {
        // Which is what makes a handle's step order the order a hand expects,
        // and what lets the editor pick a snap target by nearest share.
        assert_eq!(BlockWidth::Fit.share(), None);
        let shares: Vec<f64> =
            BlockWidth::SHARES.iter().map(|width| width.share().expect("a fixed share")).collect();

        assert!(shares.windows(2).all(|pair| pair[0] > pair[1]), "{shares:?}");
    }

    #[test]
    fn the_intrinsic_default_writes_no_attribute_rule() {
        assert!(!css().contains("fit"));
    }

    #[test]
    fn an_explicit_full_width_gets_a_full_region_rule() {
        assert!(css().contains(
            ".slidx-block[data-slidx-width=\"full\"] { width: 100.0000%; margin-inline: auto; }"
        ));
    }

    #[test]
    fn every_rule_is_a_percentage_of_the_region_rather_than_a_length() {
        // A block measured in pixels is the wrong size on the next projector.
        // The percentage resolves against the region, which is a share of the
        // slide, which is what keeps the whole thing scaling as one piece.
        let css = css();

        for line in css.lines().filter(|line| !line.trim().is_empty()) {
            assert!(line.contains("width: ") && line.contains('%'), "{line}");
            assert!(!line.contains("px"), "{line}");
        }
    }

    #[test]
    fn every_fixed_share_gets_a_rule() {
        let css = css();

        for width in BlockWidth::SHARES {
            assert!(css.contains(width.as_token()), "{} has no rule", width.as_token());
        }
    }

    #[test]
    fn a_share_crosses_the_boundary_as_the_word_an_author_writes() {
        // The editor sends one of these, so an unknown word is refused at the
        // boundary rather than written into somebody's deck.
        assert_eq!(serde_json::to_value(BlockWidth::TwoThirds).unwrap(), "two-thirds");
        assert_eq!(serde_json::to_value(BlockWidth::Fit).unwrap(), "fit");
        assert_eq!(
            serde_json::from_value::<BlockWidth>(serde_json::json!("fit")).unwrap(),
            BlockWidth::Fit
        );
        assert_eq!(
            serde_json::from_value::<BlockWidth>(serde_json::json!("half")).unwrap(),
            BlockWidth::Half
        );
        assert!(serde_json::from_value::<BlockWidth>(serde_json::json!("340px")).is_err());
    }
}
