//! What the caller gets to choose.
//!
//! Two knobs, deliberately: how much damage the code survives, and how the SVG
//! sits on a slide. Everything else — version, mask, module layout — is decided
//! by the spec's own rules, and exposing it would only offer an author a way to
//! produce a code that scans worse.
//!
//! The types are serialisable because a deck configures a QR code in
//! frontmatter, which reaches this crate as data rather than as Rust.

use serde::{Deserialize, Serialize};

/// How much of the code can be obscured and still read.
///
/// Ordered from least to most redundancy, which is also the order the tables in
/// [`crate::version`] are indexed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ecc {
    /// ~7% recoverable.
    Low,
    /// ~15% recoverable.
    Medium,
    /// ~25% recoverable.
    Quartile,
    /// ~30% recoverable.
    High,
}

impl Ecc {
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::Quartile => "quartile",
            Self::High => "high",
        }
    }

    /// Row index into the per-version block tables.
    pub(crate) fn index(self) -> usize {
        self as usize
    }

    /// The two bits this level contributes to the format information.
    ///
    /// Not the same as [`Ecc::index`], and not in the same order: the spec
    /// assigns L=01, M=00, Q=11, H=10 so that the four values differ in more
    /// bit positions than a plain 0..3 would. Conflating the two writes a code
    /// that every reader rejects.
    pub(crate) fn indicator(self) -> u32 {
        match self {
            Self::Low => 0b01,
            Self::Medium => 0b00,
            Self::Quartile => 0b11,
            Self::High => 0b10,
        }
    }

    pub const ALL: [Self; 4] = [Self::Low, Self::Medium, Self::Quartile, Self::High];
}

/// How the payload is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QrOptions {
    pub ecc: Ecc,
}

impl QrOptions {
    pub fn new(ecc: Ecc) -> Self {
        Self { ecc }
    }

    pub fn with_ecc(mut self, ecc: Ecc) -> Self {
        self.ecc = ecc;
        self
    }
}

impl Default for QrOptions {
    /// Medium, because a slide is the adversarial case in both directions.
    ///
    /// A projected code is read from across a room, so every extra module of
    /// redundancy shrinks the modules a phone camera has to resolve; but glare
    /// and a head in the way mean some of it will be lost. Medium is the level
    /// that survives both. High belongs on print, where the code is close and
    /// the paper is creased.
    fn default() -> Self {
        Self { ecc: Ecc::Medium }
    }
}

/// How the code is drawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SvgOptions {
    /// Light margin around the code, in modules.
    ///
    /// The spec requires four. Readers use the margin to find the code's edge,
    /// and a code butted against slide content is one many phones never lock
    /// onto — so this is configurable upward, and clamping keeps it from being
    /// configured into unscannability.
    pub quiet_zone: u32,
    /// Fill behind the code, or transparent when `None`.
    ///
    /// Transparent by default so the theme's surface shows through and the code
    /// inherits whatever contrast the theme already guarantees. Set it when the
    /// code sits over an image, where nothing guarantees anything.
    pub background: Option<String>,
    /// Accessible name, emitted as `<title>`.
    ///
    /// A QR code is a link with no visible text, so a reader that cannot see it
    /// gets nothing at all unless this says where it goes.
    pub title: Option<String>,
}

/// Below this a reader has no reliable margin to lock onto.
pub const MIN_QUIET_ZONE: u32 = 4;

impl SvgOptions {
    pub fn with_quiet_zone(mut self, modules: u32) -> Self {
        self.quiet_zone = modules;
        self
    }

    pub fn with_background(mut self, color: impl Into<String>) -> Self {
        self.background = Some(color.into());
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The quiet zone actually drawn, never below the spec's minimum.
    pub(crate) fn effective_quiet_zone(&self) -> u32 {
        self.quiet_zone.max(MIN_QUIET_ZONE)
    }
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self { quiet_zone: MIN_QUIET_ZONE, background: None, title: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecc_levels_are_ordered_by_how_much_damage_they_survive() {
        // The table lookups in `version` index by this order, so reordering the
        // enum silently reassigns every block layout.
        assert!(Ecc::Low < Ecc::Medium);
        assert!(Ecc::Medium < Ecc::Quartile);
        assert!(Ecc::Quartile < Ecc::High);
        assert_eq!(Ecc::ALL.map(Ecc::index), [0, 1, 2, 3]);
    }

    #[test]
    fn format_indicators_match_the_spec_rather_than_the_enum_order() {
        // The spec's assignment is not monotonic; a reader rejects anything else.
        assert_eq!(Ecc::Low.indicator(), 0b01);
        assert_eq!(Ecc::Medium.indicator(), 0b00);
        assert_eq!(Ecc::Quartile.indicator(), 0b11);
        assert_eq!(Ecc::High.indicator(), 0b10);
    }

    #[test]
    fn the_default_level_is_medium() {
        assert_eq!(QrOptions::default().ecc, Ecc::Medium);
    }

    #[test]
    fn a_quiet_zone_cannot_be_configured_below_the_scannable_minimum() {
        // An author trimming the margin to fit a layout would otherwise ship a
        // code that no longer scans, and nothing on screen would show it.
        assert_eq!(SvgOptions::default().with_quiet_zone(0).effective_quiet_zone(), 4);
        assert_eq!(SvgOptions::default().with_quiet_zone(8).effective_quiet_zone(), 8);
    }

    #[test]
    fn options_round_trip_through_json_so_frontmatter_can_carry_them() {
        let options = QrOptions::new(Ecc::Quartile);
        let json = serde_json::to_string(&options).unwrap();

        assert_eq!(json, r#"{"ecc":"quartile"}"#);
        assert_eq!(serde_json::from_str::<QrOptions>(&json).unwrap(), options);
    }

    #[test]
    fn omitted_svg_fields_fall_back_to_the_defaults() {
        let options: SvgOptions = serde_json::from_str("{}").unwrap();

        assert_eq!(options, SvgOptions::default());
    }
}
