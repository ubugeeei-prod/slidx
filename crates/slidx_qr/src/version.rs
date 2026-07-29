//! Version tables: how big the grid is, and how its codewords are grouped.
//!
//! Everything downstream is derived from what is transcribed here, so this is
//! the module where a typo is most expensive: a wrong block count produces a
//! grid that looks like a QR code, renders without complaint, and decodes on no
//! reader. The tests below re-derive the spec's published codeword totals from
//! the block layout, which is what catches such a typo.
//!
//! Versions stop at 10 on purpose. A code that has to carry more than ~270
//! bytes has 57 modules across it, and projected at slide size those modules
//! land below what a phone camera resolves from the third row — so the useful
//! ceiling is well under the format's.

use crate::error::QrError;
use crate::options::Ecc;

/// The largest version this crate encodes.
pub const MAX_VERSION: u8 = 10;

/// A QR symbol size, 1 through [`MAX_VERSION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u8);

impl Version {
    pub fn new(value: u8) -> Option<Self> {
        (1..=MAX_VERSION).contains(&value).then_some(Self(value))
    }

    pub fn value(self) -> u8 {
        self.0
    }

    /// Modules along one edge.
    ///
    /// Every version is 4 modules wider than the last, starting at 21.
    pub fn size(self) -> usize {
        4 * self.0 as usize + 17
    }

    /// Version information is only carried by version 7 and above; smaller
    /// symbols are identified by their size alone.
    pub(crate) fn carries_version_information(self) -> bool {
        self.0 >= 7
    }

    /// Bits the byte-mode character count field occupies.
    ///
    /// Eight bits stops being enough at version 10, where the payload can
    /// exceed 255 bytes. Using the wrong width shifts the entire bit stream.
    pub(crate) fn character_count_bits(self) -> usize {
        if self.0 <= 9 {
            8
        } else {
            16
        }
    }

    /// Centres of the alignment patterns, as coordinates on both axes.
    pub(crate) fn alignment_centers(self) -> &'static [usize] {
        ALIGNMENT_CENTERS[self.0 as usize - 1]
    }

    pub(crate) fn layout(self, ecc: Ecc) -> BlockLayout {
        let (ecc_per_block, group1_blocks, group1_data, group2_blocks, group2_data) =
            LAYOUTS[self.0 as usize - 1][ecc.index()];

        BlockLayout {
            ecc_per_block: ecc_per_block as usize,
            group1_blocks: group1_blocks as usize,
            group1_data: group1_data as usize,
            group2_blocks: group2_blocks as usize,
            group2_data: group2_data as usize,
        }
    }

    /// Payload bytes that fit at this version and level.
    pub fn byte_capacity(self, ecc: Ecc) -> usize {
        let payload_bits = self.layout(ecc).data_codewords() * 8 - 4 - self.character_count_bits();

        payload_bits / 8
    }

    pub const ALL: [Self; MAX_VERSION as usize] =
        [Self(1), Self(2), Self(3), Self(4), Self(5), Self(6), Self(7), Self(8), Self(9), Self(10)];
}

/// How one version's codewords are split into Reed–Solomon blocks.
///
/// Two groups rather than one because a version's data codewords rarely divide
/// evenly: group 2's blocks each hold exactly one more codeword than group 1's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlockLayout {
    pub ecc_per_block: usize,
    pub group1_blocks: usize,
    pub group1_data: usize,
    pub group2_blocks: usize,
    pub group2_data: usize,
}

impl BlockLayout {
    pub(crate) fn blocks(&self) -> usize {
        self.group1_blocks + self.group2_blocks
    }

    pub(crate) fn data_codewords(&self) -> usize {
        self.group1_blocks * self.group1_data + self.group2_blocks * self.group2_data
    }

    pub(crate) fn total_codewords(&self) -> usize {
        self.data_codewords() + self.blocks() * self.ecc_per_block
    }
}

/// Picks the smallest version the payload fits in.
///
/// Smallest rather than a fixed one because module size is what a camera has to
/// resolve: an unnecessarily large version puts finer modules on the wall for
/// no gain.
pub(crate) fn select(byte_length: usize, ecc: Ecc) -> Result<Version, QrError> {
    Version::ALL.into_iter().find(|version| byte_length <= version.byte_capacity(ecc)).ok_or(
        QrError::TooLong {
            bytes: byte_length,
            capacity: Version::ALL[MAX_VERSION as usize - 1].byte_capacity(ecc),
            ecc,
        },
    )
}

/// One version-and-level cell of the block table: `(ecc codewords per block,
/// group 1 blocks, group 1 data codewords, group 2 blocks, group 2 data
/// codewords)`.
type LayoutEntry = (u8, u8, u8, u8, u8);

/// The block table, indexed by version then by [`Ecc`] order.
///
/// Transcribed from ISO/IEC 18004 table 9. `total_codewords_match_the_spec`
/// re-derives the published per-version totals from these rows, so a mistyped
/// digit fails a test rather than shipping an undecodable code.
#[rustfmt::skip]
const LAYOUTS: [[LayoutEntry; 4]; MAX_VERSION as usize] = [
    // Version 1
    [(7, 1, 19, 0, 0), (10, 1, 16, 0, 0), (13, 1, 13, 0, 0), (17, 1, 9, 0, 0)],
    // Version 2
    [(10, 1, 34, 0, 0), (16, 1, 28, 0, 0), (22, 1, 22, 0, 0), (28, 1, 16, 0, 0)],
    // Version 3
    [(15, 1, 55, 0, 0), (26, 1, 44, 0, 0), (18, 2, 17, 0, 0), (22, 2, 13, 0, 0)],
    // Version 4
    [(20, 1, 80, 0, 0), (18, 2, 32, 0, 0), (26, 2, 24, 0, 0), (16, 4, 9, 0, 0)],
    // Version 5
    [(26, 1, 108, 0, 0), (24, 2, 43, 0, 0), (18, 2, 15, 2, 16), (22, 2, 11, 2, 12)],
    // Version 6
    [(18, 2, 68, 0, 0), (16, 4, 27, 0, 0), (24, 4, 19, 0, 0), (28, 4, 15, 0, 0)],
    // Version 7
    [(20, 2, 78, 0, 0), (18, 4, 31, 0, 0), (18, 2, 14, 4, 15), (26, 4, 13, 1, 14)],
    // Version 8
    [(24, 2, 97, 0, 0), (22, 2, 38, 2, 39), (22, 4, 18, 2, 19), (26, 4, 14, 2, 15)],
    // Version 9
    [(30, 2, 116, 0, 0), (22, 3, 36, 2, 37), (20, 4, 16, 4, 17), (24, 4, 12, 4, 13)],
    // Version 10
    [(18, 2, 68, 2, 69), (26, 4, 43, 1, 44), (24, 6, 19, 2, 20), (28, 6, 15, 2, 16)],
];

/// Alignment pattern centres per version, on both axes.
///
/// Every pairing of these coordinates carries a pattern except the three that
/// would land on a finder pattern.
const ALIGNMENT_CENTERS: [&[usize]; MAX_VERSION as usize] = [
    &[],
    &[6, 18],
    &[6, 22],
    &[6, 26],
    &[6, 30],
    &[6, 34],
    &[6, 22, 38],
    &[6, 24, 42],
    &[6, 26, 46],
    &[6, 28, 50],
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Total codewords per version, from ISO/IEC 18004 table 1 — a source
    /// independent of the block table these tests check.
    const PUBLISHED_TOTALS: [usize; MAX_VERSION as usize] =
        [26, 44, 70, 100, 134, 172, 196, 242, 292, 346];

    #[test]
    fn a_versions_size_is_four_times_its_number_plus_seventeen() {
        assert_eq!(Version::new(1).unwrap().size(), 21);
        assert_eq!(Version::new(2).unwrap().size(), 25);
        assert_eq!(Version::new(10).unwrap().size(), 57);
    }

    #[test]
    fn versions_outside_the_supported_range_do_not_exist() {
        assert!(Version::new(0).is_none());
        assert!(Version::new(11).is_none());
        assert!(Version::new(40).is_none());
    }

    #[test]
    fn total_codewords_match_the_spec_for_every_version_and_level() {
        // Data and ECC codewords must exactly fill the symbol. Deriving the
        // total from the block layout and comparing it against the separately
        // published figure is what catches a mistyped row.
        for version in Version::ALL {
            for ecc in Ecc::ALL {
                let total = version.layout(ecc).total_codewords();

                assert_eq!(
                    total,
                    PUBLISHED_TOTALS[version.value() as usize - 1],
                    "version {} at {}",
                    version.value(),
                    ecc.as_token()
                );
            }
        }
    }

    #[test]
    fn group_two_blocks_hold_exactly_one_more_codeword_than_group_one() {
        // The interleaver relies on this: it walks a single extra column for
        // group 2 rather than tracking per-block lengths.
        for version in Version::ALL {
            for ecc in Ecc::ALL {
                let layout = version.layout(ecc);
                if layout.group2_blocks > 0 {
                    assert_eq!(
                        layout.group2_data,
                        layout.group1_data + 1,
                        "version {} at {}",
                        version.value(),
                        ecc.as_token()
                    );
                }
            }
        }
    }

    #[test]
    fn byte_capacity_matches_the_published_figures() {
        // Spot checks against ISO/IEC 18004 table 7, at both ends of the range.
        assert_eq!(Version::new(1).unwrap().byte_capacity(Ecc::Low), 17);
        assert_eq!(Version::new(1).unwrap().byte_capacity(Ecc::Medium), 14);
        assert_eq!(Version::new(1).unwrap().byte_capacity(Ecc::Quartile), 11);
        assert_eq!(Version::new(1).unwrap().byte_capacity(Ecc::High), 7);
        assert_eq!(Version::new(10).unwrap().byte_capacity(Ecc::Low), 271);
        assert_eq!(Version::new(10).unwrap().byte_capacity(Ecc::Medium), 213);
        assert_eq!(Version::new(10).unwrap().byte_capacity(Ecc::Quartile), 151);
        assert_eq!(Version::new(10).unwrap().byte_capacity(Ecc::High), 119);
    }

    #[test]
    fn capacity_grows_with_version_and_shrinks_with_redundancy() {
        for ecc in Ecc::ALL {
            for pair in Version::ALL.windows(2) {
                assert!(
                    pair[0].byte_capacity(ecc) < pair[1].byte_capacity(ecc),
                    "version {} did not hold more than {}",
                    pair[1].value(),
                    pair[0].value()
                );
            }
        }

        let version = Version::new(5).unwrap();
        assert!(version.byte_capacity(Ecc::Low) > version.byte_capacity(Ecc::High));
    }

    #[test]
    fn the_smallest_version_that_fits_is_the_one_chosen() {
        // A larger version means finer modules on a projector for no gain.
        assert_eq!(select(1, Ecc::Medium).unwrap().value(), 1);
        assert_eq!(select(14, Ecc::Medium).unwrap().value(), 1);
        assert_eq!(select(15, Ecc::Medium).unwrap().value(), 2);
    }

    #[test]
    fn a_payload_beyond_the_largest_version_is_refused() {
        let error = select(1000, Ecc::Medium).unwrap_err();

        assert_eq!(error, QrError::TooLong { bytes: 1000, capacity: 213, ecc: Ecc::Medium });
    }

    #[test]
    fn the_character_count_field_widens_at_version_ten() {
        // Version 10 can carry more than 255 bytes, which an 8-bit count cannot
        // express; getting this wrong shifts every bit after it.
        assert_eq!(Version::new(9).unwrap().character_count_bits(), 8);
        assert_eq!(Version::new(10).unwrap().character_count_bits(), 16);
        assert!(Version::new(10).unwrap().byte_capacity(Ecc::Low) > 255);
    }

    #[test]
    fn only_version_seven_and_above_carry_version_information() {
        assert!(!Version::new(6).unwrap().carries_version_information());
        assert!(Version::new(7).unwrap().carries_version_information());
    }

    #[test]
    fn version_one_has_no_alignment_patterns() {
        assert!(Version::new(1).unwrap().alignment_centers().is_empty());
        assert_eq!(Version::new(2).unwrap().alignment_centers(), [6, 18]);
        assert_eq!(Version::new(10).unwrap().alignment_centers(), [6, 28, 50]);
    }

    #[test]
    fn every_alignment_centre_lands_inside_the_symbol() {
        for version in Version::ALL {
            for &center in version.alignment_centers() {
                assert!(
                    center + 2 < version.size(),
                    "version {} alignment centre {center} overflows the grid",
                    version.value()
                );
            }
        }
    }
}
