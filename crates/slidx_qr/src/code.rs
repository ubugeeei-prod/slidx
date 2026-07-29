//! A finished code, and what a caller can ask of it.
//!
//! The module grid is kept private behind [`QrCode::module`] because its two
//! invariants — square, and exactly `size` on a side — are the only things
//! every consumer downstream relies on, and a public `Vec<bool>` is one
//! `push` away from breaking both.

use crate::options::SvgOptions;
use crate::svg;
use crate::version::Version;
use crate::Ecc;

/// An encoded QR symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrCode {
    version: Version,
    ecc: Ecc,
    mask: u8,
    size: usize,
    modules: Vec<bool>,
}

impl QrCode {
    pub(crate) fn new(version: Version, ecc: Ecc, mask: u8, modules: Vec<bool>) -> Self {
        Self { version, ecc, mask, size: version.size(), modules }
    }

    /// Modules along one edge, always `4 * version + 17`.
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn ecc(&self) -> Ecc {
        self.ecc
    }

    /// Which of the eight mask patterns was applied. Recorded so a caller can
    /// reproduce or diff a symbol; nothing needs it to display one.
    pub fn mask(&self) -> u8 {
        self.mask
    }

    /// Whether the module at this position is dark.
    ///
    /// Positions outside the grid read light rather than panicking, because the
    /// quiet zone is exactly that: a caller walking a region larger than the
    /// code is drawing the margin, not making a mistake.
    pub fn module(&self, row: usize, column: usize) -> bool {
        if row >= self.size || column >= self.size {
            return false;
        }

        self.modules[row * self.size + column]
    }

    /// Renders the code as a self-contained SVG.
    pub fn to_svg(&self, options: &SvgOptions) -> String {
        svg::render(self, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode, QrOptions};

    #[test]
    fn a_codes_size_follows_from_its_version() {
        let code = encode("slidx", &QrOptions::default()).unwrap();

        assert_eq!(code.size(), 4 * code.version().value() as usize + 17);
    }

    #[test]
    fn reading_outside_the_grid_returns_light_rather_than_panicking() {
        // Callers draw the quiet zone by walking past the edge; making that a
        // panic would push the bounds check into every consumer.
        let code = encode("slidx", &QrOptions::default()).unwrap();

        assert!(!code.module(code.size(), 0));
        assert!(!code.module(0, code.size()));
        assert!(!code.module(usize::MAX, usize::MAX));
    }

    #[test]
    fn a_code_reports_the_level_it_was_asked_for() {
        let code = encode("slidx", &QrOptions::new(Ecc::High)).unwrap();

        assert_eq!(code.ecc(), Ecc::High);
    }

    #[test]
    fn the_mask_is_one_of_the_eight_the_format_field_can_express() {
        // Three bits record it; a ninth mask could not be communicated.
        for text in ["a", "https://slidx.dev", &"x".repeat(200)] {
            let code = encode(text, &QrOptions::default()).unwrap();

            assert!(code.mask() < 8, "{} produced mask {}", text, code.mask());
        }
    }
}
