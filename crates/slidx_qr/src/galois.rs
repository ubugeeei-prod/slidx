//! Arithmetic in GF(256), the field Reed–Solomon error correction lives in.
//!
//! Without this a damaged code is an unreadable code, which on a slide means a
//! link nobody can follow: the codes that need recovery most are the ones being
//! photographed at an angle, through glare, from the back of a room.
//!
//! Multiplication is done by shift-and-reduce rather than through log tables.
//! The tables are the faster choice at scale, but the largest polynomial here
//! has 30 terms and the whole encode runs once per build — so the version with
//! no lookup table to initialise, no zero special case, and nothing to get
//! subtly wrong is the better trade.

/// The field's reduction polynomial, x^8 + x^4 + x^3 + x^2 + 1, minus its
/// x^8 term. Fixed by the QR spec; any other choice produces a different field
/// and error correction that no reader can verify.
const REDUCTION: u8 = 0x1D;

/// Multiplies two field elements.
///
/// Addition in GF(256) is XOR, so this is the usual double-and-add with the
/// doubling reduced back into the field whenever it overflows 8 bits.
pub(crate) fn multiply(left: u8, right: u8) -> u8 {
    let mut product = 0u8;
    let mut left = left;
    let mut right = right;

    while right != 0 {
        if right & 1 == 1 {
            product ^= left;
        }

        let overflows = left & 0x80 != 0;
        left <<= 1;
        if overflows {
            left ^= REDUCTION;
        }

        right >>= 1;
    }

    product
}

/// The generator polynomial for `degree` error correction codewords.
///
/// It is the product of `(x - α^i)` for `i` in `0..degree`, which is what makes
/// every root of the generator a root of a valid codeword — the property a
/// reader checks to detect damage. Coefficients run from the highest degree
/// down, and the leading one is always 1.
pub(crate) fn generator(degree: usize) -> Vec<u8> {
    let mut polynomial = vec![1u8];
    let mut root = 1u8;

    for _ in 0..degree {
        let mut next = vec![0u8; polynomial.len() + 1];
        for (index, &coefficient) in polynomial.iter().enumerate() {
            next[index] ^= coefficient;
            next[index + 1] ^= multiply(coefficient, root);
        }

        polynomial = next;
        root = multiply(root, 2);
    }

    polynomial
}

/// The error correction codewords for one block.
///
/// This is the remainder of the data polynomial, shifted up by `count`, divided
/// by the generator — computed by synthetic division, which needs no storage
/// beyond the remainder itself.
pub(crate) fn error_correction(data: &[u8], count: usize) -> Vec<u8> {
    let generator = generator(count);
    let mut remainder = vec![0u8; count];

    for &codeword in data {
        let factor = codeword ^ remainder[0];
        remainder.remove(0);
        remainder.push(0);

        // The generator is monic, so its leading term only ever cancels the
        // codeword already shifted out above.
        for (index, &coefficient) in generator.iter().enumerate().skip(1) {
            remainder[index - 1] ^= multiply(coefficient, factor);
        }
    }

    remainder
}

/// Evaluates a polynomial at `x`, coefficients highest degree first.
#[cfg(test)]
pub(crate) fn evaluate(coefficients: &[u8], x: u8) -> u8 {
    coefficients
        .iter()
        .fold(0u8, |accumulator, &coefficient| multiply(accumulator, x) ^ coefficient)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplication_has_one_as_its_identity_and_zero_as_its_annihilator() {
        for value in 0..=255u8 {
            assert_eq!(multiply(value, 1), value);
            assert_eq!(multiply(value, 0), 0);
        }
    }

    #[test]
    fn multiplication_is_commutative() {
        for left in 0..=255u8 {
            for right in [0u8, 1, 2, 7, 29, 128, 255] {
                assert_eq!(multiply(left, right), multiply(right, left));
            }
        }
    }

    #[test]
    fn every_non_zero_element_has_an_inverse() {
        // A field where some element has no inverse is not a field, and the
        // error correction built on it would be silently wrong rather than
        // obviously broken.
        for value in 1..=255u8 {
            assert!(
                (1..=255u8).any(|candidate| multiply(value, candidate) == 1),
                "{value} has no multiplicative inverse"
            );
        }
    }

    #[test]
    fn the_powers_of_the_primitive_element_cycle_through_every_non_zero_value() {
        // α must generate the whole multiplicative group, or the generator
        // polynomial's roots repeat and the code corrects less than it claims.
        let mut seen = vec![false; 256];
        let mut value = 1u8;

        for _ in 0..255 {
            assert!(!seen[value as usize], "the powers of α repeat before 255");
            seen[value as usize] = true;
            value = multiply(value, 2);
        }

        assert_eq!(value, 1, "α^255 must return to 1");
    }

    #[test]
    fn the_generator_polynomial_has_one_more_term_than_its_degree_and_is_monic() {
        for degree in [7usize, 10, 13, 17, 30] {
            let polynomial = generator(degree);

            assert_eq!(polynomial.len(), degree + 1);
            assert_eq!(polynomial[0], 1, "synthetic division assumes a monic generator");
        }
    }

    #[test]
    fn the_generator_for_seven_codewords_matches_the_published_polynomial() {
        // ISO/IEC 18004 annex A lists these coefficients; they are the fixture
        // that anchors the whole field implementation to the spec.
        assert_eq!(generator(7), vec![1, 127, 122, 154, 164, 11, 68, 117]);
    }

    #[test]
    fn the_generator_for_ten_codewords_matches_the_published_polynomial() {
        assert_eq!(generator(10), vec![1, 216, 194, 159, 111, 199, 94, 95, 113, 157, 193]);
    }

    #[test]
    fn every_root_of_the_generator_is_a_root_of_the_encoded_block() {
        // This is the property a reader relies on to spot damage. Checking it
        // directly tests the division against the field rather than against
        // itself, which no fixture comparison can do.
        let data: Vec<u8> = (0..16u8).map(|index| index.wrapping_mul(37).wrapping_add(5)).collect();
        let count = 10;

        let mut block = data.clone();
        block.extend(error_correction(&data, count));

        let mut root = 1u8;
        for power in 0..count {
            assert_eq!(evaluate(&block, root), 0, "the block does not vanish at α^{power}");
            root = multiply(root, 2);
        }
    }

    #[test]
    fn error_correction_produces_exactly_the_requested_number_of_codewords() {
        for count in [7usize, 17, 22, 30] {
            assert_eq!(error_correction(&[1, 2, 3, 4, 5], count).len(), count);
        }
    }
}
