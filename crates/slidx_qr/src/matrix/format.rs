//! Format and version information: the metadata a reader needs before it can
//! read anything else.
//!
//! Both fields are BCH-coded and both are written twice, in opposite corners,
//! because they are unrecoverable by any other means: the error correction that
//! protects the payload cannot be applied until the reader knows which level to
//! apply, and that is what these bits say. Damage here does not degrade the
//! code, it ends it.
//!
//! The format field is additionally masked with a fixed pattern so that the
//! all-zero case — level M with mask 0 — does not produce a large blank region
//! next to the finder patterns that a reader could mistake for quiet zone.

use crate::matrix::Matrix;
use crate::options::Ecc;

/// Generator for the BCH(15, 5) code protecting the format information.
const FORMAT_GENERATOR: u32 = 0b101_0011_0111;

/// Applied to the finished format bits so the field is never uniformly light.
const FORMAT_MASK: u32 = 0b101_0100_0001_0010;

/// Generator for the BCH(18, 6) code protecting the version information.
const VERSION_GENERATOR: u32 = 0b1_1111_0010_0101;

/// The 15 format bits for a level and mask.
pub(crate) fn format_bits(ecc: Ecc, mask: u8) -> u32 {
    let data = (ecc.indicator() << 3) | u32::from(mask);

    ((data << 10) | remainder(data << 10, FORMAT_GENERATOR, 11)) ^ FORMAT_MASK
}

/// The 18 version bits for versions 7 and above.
pub(crate) fn version_bits(version: u8) -> u32 {
    let data = u32::from(version);

    (data << 12) | remainder(data << 12, VERSION_GENERATOR, 13)
}

/// Polynomial remainder over GF(2), for a generator of `generator_bits` bits.
///
/// Long division where subtraction is XOR: align the generator with the value's
/// highest set bit and cancel it, until nothing is left above the generator's
/// degree.
fn remainder(mut value: u32, generator: u32, generator_bits: u32) -> u32 {
    while bit_length(value) >= generator_bits {
        value ^= generator << (bit_length(value) - generator_bits);
    }

    value
}

fn bit_length(value: u32) -> u32 {
    u32::BITS - value.leading_zeros()
}

/// Writes both copies of the format information, and the one module that is
/// always dark.
pub(crate) fn write_format(matrix: &mut Matrix, ecc: Ecc, mask: u8) {
    let bits = format_bits(ecc, mask);
    let size = matrix.size();
    let bit = |index: u32| (bits >> index) & 1 == 1;

    // First copy, wrapped around the top-left finder. Column and row 6 are the
    // timing patterns, which the field steps over.
    for index in 0..6 {
        matrix.set(8, index as usize, bit(index));
    }
    matrix.set(8, 7, bit(6));
    matrix.set(8, 8, bit(7));
    matrix.set(7, 8, bit(8));
    for index in 9..15 {
        matrix.set(14 - index as usize, 8, bit(index));
    }

    // Second copy, split between the two remaining finders so that no single
    // region of damage can take both copies.
    for index in 0..7 {
        matrix.set(size - 1 - index as usize, 8, bit(index));
    }
    for index in 7..15 {
        matrix.set(8, size - 15 + index as usize, bit(index));
    }

    // Always dark, and not part of either copy. A reader uses it to confirm it
    // has found a QR symbol rather than something that merely looks like one.
    matrix.set(size - 8, 8, true);
}

/// Writes both copies of the version information, for versions that carry it.
pub(crate) fn write_version(matrix: &mut Matrix) {
    if !matrix.version().carries_version_information() {
        return;
    }

    let bits = version_bits(matrix.version().value());
    let size = matrix.size();

    for index in 0..18 {
        let dark = (bits >> index) & 1 == 1;
        let far = size - 11 + index % 3;
        let near = index / 3;

        matrix.set(far, near, dark);
        matrix.set(near, far, dark);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::Version;

    #[test]
    fn format_bits_match_the_published_table_for_every_level_and_mask() {
        // ISO/IEC 18004 table 10 lists all thirty-two values. They are the only
        // external check available on the BCH code, and a reader rejects any
        // symbol whose format bits are not one of exactly these.
        const PUBLISHED: [[u32; 8]; 4] = [
            [0x77C4, 0x72F3, 0x7DAA, 0x789D, 0x662F, 0x6318, 0x6C41, 0x6976],
            [0x5412, 0x5125, 0x5E7C, 0x5B4B, 0x45F9, 0x40CE, 0x4F97, 0x4AA0],
            [0x355F, 0x3068, 0x3F31, 0x3A06, 0x24B4, 0x2183, 0x2EDA, 0x2BED],
            [0x1689, 0x13BE, 0x1CE7, 0x19D0, 0x0762, 0x0255, 0x0D0C, 0x083B],
        ];

        for (level, ecc) in Ecc::ALL.into_iter().enumerate() {
            for mask in 0..8u8 {
                assert_eq!(
                    format_bits(ecc, mask),
                    PUBLISHED[level][mask as usize],
                    "{} with mask {mask}",
                    ecc.as_token()
                );
            }
        }
    }

    #[test]
    fn every_pair_of_format_values_differs_in_at_least_three_bits() {
        // The BCH code exists so that damaged format bits can be corrected to
        // the nearest valid value. If two values were closer than this, damage
        // would silently correct to the wrong level.
        let all: Vec<u32> = Ecc::ALL
            .into_iter()
            .flat_map(|ecc| (0..8u8).map(move |mask| format_bits(ecc, mask)))
            .collect();

        for (index, left) in all.iter().enumerate() {
            for right in &all[index + 1..] {
                assert!((left ^ right).count_ones() >= 3, "{left:#x} and {right:#x} are too close");
            }
        }
    }

    #[test]
    fn the_all_zero_format_case_is_still_written_as_dark_modules() {
        // Level M with mask 0 encodes to five zero bits and a zero remainder.
        // Without the fixed mask it would paint a blank strip beside a finder,
        // which a reader can read as quiet zone.
        assert_eq!(format_bits(Ecc::Medium, 0), FORMAT_MASK);
        assert!(format_bits(Ecc::Medium, 0).count_ones() >= 5);
    }

    #[test]
    fn version_bits_match_the_published_table() {
        // ISO/IEC 18004 table 11, for the versions this crate supports.
        assert_eq!(version_bits(7), 0x07C94);
        assert_eq!(version_bits(8), 0x085BC);
        assert_eq!(version_bits(9), 0x09A99);
        assert_eq!(version_bits(10), 0x0A4D3);
    }

    #[test]
    fn version_bits_carry_the_version_number_in_their_top_six_bits() {
        for version in 7..=10u8 {
            assert_eq!(version_bits(version) >> 12, u32::from(version));
        }
    }

    #[test]
    fn both_copies_of_the_format_information_carry_the_same_bits() {
        // A reader falls back to the second copy when the first is damaged, so
        // a mismatch turns recoverable damage into an unreadable code.
        let mut matrix = Matrix::new(Version::new(1).unwrap());
        write_format(&mut matrix, Ecc::Quartile, 5);
        let size = matrix.size();
        let bits = format_bits(Ecc::Quartile, 5);

        let first: Vec<bool> = (0..6)
            .map(|index| matrix.get(8, index))
            .chain([matrix.get(8, 7), matrix.get(8, 8), matrix.get(7, 8)])
            .chain((9..15).map(|index| matrix.get(14 - index, 8)))
            .collect();
        let second: Vec<bool> = (0..7)
            .map(|index| matrix.get(size - 1 - index, 8))
            .chain((7..15).map(|index| matrix.get(8, size - 15 + index)))
            .collect();

        assert_eq!(first, second);
        assert_eq!(first, (0..15).map(|index| (bits >> index) & 1 == 1).collect::<Vec<_>>());
    }

    #[test]
    fn the_dark_module_is_always_dark_whatever_the_format_says() {
        // It is the one module whose value never depends on anything.
        for ecc in Ecc::ALL {
            for mask in 0..8u8 {
                let mut matrix = Matrix::new(Version::new(2).unwrap());
                write_format(&mut matrix, ecc, mask);

                assert!(matrix.get(matrix.size() - 8, 8), "{} with mask {mask}", ecc.as_token());
            }
        }
    }

    #[test]
    fn small_versions_are_left_without_version_information() {
        // Versions below 7 have no reserved area for it; writing anyway would
        // overwrite payload.
        let mut matrix = Matrix::new(Version::new(6).unwrap());
        let before = matrix.clone();
        write_version(&mut matrix);

        assert_eq!(matrix, before);
    }

    #[test]
    fn both_copies_of_the_version_information_carry_the_same_bits() {
        let mut matrix = Matrix::new(Version::new(9).unwrap());
        write_version(&mut matrix);
        let size = matrix.size();

        for index in 0..18usize {
            let far = size - 11 + index % 3;
            let near = index / 3;

            assert_eq!(matrix.get(far, near), matrix.get(near, far), "bit {index}");
        }
    }
}
