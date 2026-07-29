//! Choosing which of the eight masks to XOR over the data.
//!
//! An unmasked symbol can come out with large blank areas, or with runs that
//! imitate a finder pattern — both of which a reader either fails to lock onto
//! or locks onto in the wrong place. The mask does not change what the code
//! says; it only redistributes dark modules until the symbol looks like the
//! noise a reader expects.
//!
//! Which mask is applied is recorded in the format information, so this is
//! purely an optimisation over appearance. It still has to be done: the spec
//! gives four penalty rules and requires the lowest-scoring mask, and a reader
//! that struggles with an unmasked symbol has no way to say so.

use crate::matrix::{format, Matrix};
use crate::options::Ecc;

/// The number of mask patterns, fixed by the three bits that record the choice.
pub(crate) const MASK_COUNT: u8 = 8;

/// Whether mask `pattern` inverts the module at this position.
///
/// The formulas are the spec's, and their only shared property is that each
/// covers roughly half the grid in a different geometry — stripes, checks,
/// diagonals — so that whatever regularity the data has, some mask breaks it.
pub(crate) fn inverts(pattern: u8, row: usize, column: usize) -> bool {
    match pattern {
        0 => (row + column) % 2 == 0,
        1 => row % 2 == 0,
        2 => column % 3 == 0,
        3 => (row + column) % 3 == 0,
        4 => (row / 2 + column / 3) % 2 == 0,
        5 => (row * column) % 2 + (row * column) % 3 == 0,
        6 => ((row * column) % 2 + (row * column) % 3) % 2 == 0,
        _ => ((row + column) % 2 + (row * column) % 3) % 2 == 0,
    }
}

/// Applies every mask in turn and keeps the one the spec's rules prefer.
///
/// Returns the chosen pattern alongside the finished grid, because the choice
/// has to be recorded in the format information for a reader to undo it.
pub(crate) fn apply_best(matrix: &Matrix, ecc: Ecc) -> (u8, Matrix) {
    (0..MASK_COUNT)
        .map(|pattern| {
            let candidate = apply(matrix, ecc, pattern);
            (penalty(&candidate), pattern, candidate)
        })
        // Lowest penalty wins, and the lowest pattern number breaks a tie so
        // that the same input always produces the same symbol.
        .min_by_key(|(score, pattern, _)| (*score, *pattern))
        .map(|(_, pattern, candidate)| (pattern, candidate))
        .expect("there is always at least one mask pattern")
}

/// One masked candidate, with its format information already written.
///
/// The format bits are part of the candidate rather than added afterwards
/// because the penalty rules score the whole symbol, and the strips beside the
/// finders are a meaningful fraction of a small one.
fn apply(matrix: &Matrix, ecc: Ecc, pattern: u8) -> Matrix {
    let mut candidate = matrix.clone();

    for row in 0..candidate.size() {
        for column in 0..candidate.size() {
            // Function patterns are how a reader finds the code at all; masking
            // them would destroy the symbol rather than tidy it.
            if candidate.is_function(row, column) {
                continue;
            }

            if inverts(pattern, row, column) {
                candidate.set(row, column, !candidate.get(row, column));
            }
        }
    }

    format::write_format(&mut candidate, ecc, pattern);

    candidate
}

/// The spec's four penalty rules, summed. Lower is better.
fn penalty(matrix: &Matrix) -> usize {
    runs(matrix) + blocks(matrix) + finder_imitations(matrix) + imbalance(matrix)
}

/// Rule 1: long runs of one colour, which give a reader nothing to count
/// module boundaries against.
fn runs(matrix: &Matrix) -> usize {
    let mut score = 0;

    for index in 0..matrix.size() {
        for line in [row_of(matrix, index), column_of(matrix, index)] {
            let mut run = 1;
            for position in 1..line.len() {
                if line[position] == line[position - 1] {
                    run += 1;
                } else {
                    score += run_penalty(run);
                    run = 1;
                }
            }
            score += run_penalty(run);
        }
    }

    score
}

fn run_penalty(run: usize) -> usize {
    if run >= 5 {
        3 + run - 5
    } else {
        0
    }
}

/// Rule 2: 2×2 areas of one colour, which blur together at a distance.
fn blocks(matrix: &Matrix) -> usize {
    let mut score = 0;

    for row in 0..matrix.size() - 1 {
        for column in 0..matrix.size() - 1 {
            let module = matrix.get(row, column);
            let uniform = matrix.get(row, column + 1) == module
                && matrix.get(row + 1, column) == module
                && matrix.get(row + 1, column + 1) == module;

            if uniform {
                score += 3;
            }
        }
    }

    score
}

/// Rule 3: sequences that imitate a finder pattern's 1:1:3:1:1 ratio next to a
/// light margin. These are the expensive ones — a reader that locks onto a
/// false finder decodes the wrong region entirely.
fn finder_imitations(matrix: &Matrix) -> usize {
    const IMITATION: [bool; 7] = [true, false, true, true, true, false, true];
    const MARGIN: [bool; 4] = [false; 4];

    let mut score = 0;

    for index in 0..matrix.size() {
        for line in [row_of(matrix, index), column_of(matrix, index)] {
            for window in line.windows(11) {
                let leading = window[..4] == MARGIN && window[4..] == IMITATION;
                let trailing = window[..7] == IMITATION && window[7..] == MARGIN;

                if leading || trailing {
                    score += 40;
                }
            }
        }
    }

    score
}

/// Rule 4: a symbol that is mostly dark or mostly light, which loses contrast
/// against whatever it is printed or projected on.
fn imbalance(matrix: &Matrix) -> usize {
    let total = matrix.size() * matrix.size();
    let dark = (0..matrix.size())
        .flat_map(|row| (0..matrix.size()).map(move |column| (row, column)))
        .filter(|&(row, column)| matrix.get(row, column))
        .count();

    // Integer arithmetic throughout: this is `10 * floor(|percent - 50| / 5)`
    // without ever forming the percentage, so the result cannot drift with
    // rounding.
    let deviation = (dark * 100).abs_diff(total * 50);

    10 * (deviation / (total * 5))
}

fn row_of(matrix: &Matrix, row: usize) -> Vec<bool> {
    (0..matrix.size()).map(|column| matrix.get(row, column)).collect()
}

fn column_of(matrix: &Matrix, column: usize) -> Vec<bool> {
    (0..matrix.size()).map(|row| matrix.get(row, column)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::place;
    use crate::version::Version;

    fn placed(version: u8, ecc: Ecc, fill: u8) -> Matrix {
        let version = Version::new(version).unwrap();
        let mut matrix = Matrix::new(version);
        format::write_version(&mut matrix);
        place::fill(&mut matrix, &vec![fill; version.layout(ecc).total_codewords()]);
        matrix
    }

    #[test]
    fn every_mask_covers_between_a_third_and_two_thirds_of_the_grid() {
        // A mask that inverted almost nothing, or almost everything, would not
        // be able to redistribute anything — the spec's eight are all near half.
        for pattern in 0..MASK_COUNT {
            let inverted = (0..21)
                .flat_map(|row| (0..21).map(move |column| (row, column)))
                .filter(|&(row, column)| inverts(pattern, row, column))
                .count();

            assert!((147..=294).contains(&inverted), "mask {pattern} covers {inverted} of 441");
        }
    }

    #[test]
    fn no_two_masks_invert_the_same_set_of_modules() {
        // Eight distinct geometries is the whole point; two that coincided
        // would leave the selector with seven real options.
        let signatures: Vec<Vec<bool>> = (0..MASK_COUNT)
            .map(|pattern| {
                (0..21)
                    .flat_map(|row| (0..21).map(move |column| (row, column)))
                    .map(|(row, column)| inverts(pattern, row, column))
                    .collect()
            })
            .collect();

        for (index, left) in signatures.iter().enumerate() {
            for right in &signatures[index + 1..] {
                assert_ne!(left, right, "mask {index} duplicates another");
            }
        }
    }

    #[test]
    fn masking_leaves_every_function_pattern_untouched() {
        // The finders are how a reader locates the symbol. Masking them would
        // not tidy the code, it would erase it.
        let matrix = placed(2, Ecc::Medium, 0xFF);

        for pattern in 0..MASK_COUNT {
            let masked = apply(&matrix, Ecc::Medium, pattern);

            for row in 0..matrix.size() {
                for column in 0..matrix.size() {
                    // The format strips are function modules that this step
                    // legitimately writes, so they are excluded.
                    let format_strip = row == 8 || column == 8;
                    if matrix.is_function(row, column) && !format_strip {
                        assert_eq!(
                            masked.get(row, column),
                            matrix.get(row, column),
                            "mask {pattern} changed ({row}, {column})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn masking_twice_with_the_same_pattern_restores_the_original_data() {
        // XOR is its own inverse, which is what lets a reader undo the mask.
        let matrix = placed(3, Ecc::Quartile, 0x6C);

        for pattern in 0..MASK_COUNT {
            let once = apply(&matrix, Ecc::Quartile, pattern);
            let twice = apply(&once, Ecc::Quartile, pattern);

            for row in 0..matrix.size() {
                for column in 0..matrix.size() {
                    if !matrix.is_function(row, column) {
                        assert_eq!(twice.get(row, column), matrix.get(row, column));
                    }
                }
            }
        }
    }

    #[test]
    fn a_run_of_five_costs_three_and_each_further_module_costs_one_more() {
        assert_eq!(run_penalty(4), 0);
        assert_eq!(run_penalty(5), 3);
        assert_eq!(run_penalty(6), 4);
        assert_eq!(run_penalty(11), 9);
    }

    #[test]
    fn a_perfectly_balanced_symbol_carries_no_imbalance_penalty() {
        // The rule is the reason a mask is chosen at all when the data happens
        // to be uniform, so its zero point has to be exactly at half.
        let mut matrix = Matrix::new(Version::new(1).unwrap());
        let size = matrix.size();
        for row in 0..size {
            for column in 0..size {
                matrix.set(row, column, (row * size + column) % 2 == 0);
            }
        }

        // 441 modules cannot split evenly, so the closest achievable split is
        // still inside the first 5% band.
        assert_eq!(imbalance(&matrix), 0);
    }

    #[test]
    fn an_all_dark_symbol_is_penalised_by_every_rule() {
        let mut matrix = Matrix::new(Version::new(1).unwrap());
        for row in 0..matrix.size() {
            for column in 0..matrix.size() {
                matrix.set(row, column, true);
            }
        }

        assert!(runs(&matrix) > 0);
        assert!(blocks(&matrix) > 0);
        assert_eq!(imbalance(&matrix), 100, "50 percentage points off balance");
    }

    #[test]
    fn a_finder_imitation_next_to_a_light_margin_costs_forty() {
        // The most expensive rule, because a reader that locks onto a false
        // finder decodes the wrong region rather than degrading gracefully.
        let mut matrix = Matrix::new(Version::new(1).unwrap());
        for row in 0..matrix.size() {
            for column in 0..matrix.size() {
                matrix.set(row, column, false);
            }
        }

        let baseline = finder_imitations(&matrix);
        for (column, dark) in [true, false, true, true, true, false, true].into_iter().enumerate() {
            matrix.set(10, column + 4, dark);
        }

        assert!(finder_imitations(&matrix) >= baseline + 40);
    }

    #[test]
    fn the_mask_with_the_lowest_penalty_is_the_one_chosen() {
        let matrix = placed(2, Ecc::Medium, 0x35);
        let (chosen, _) = apply_best(&matrix, Ecc::Medium);

        let best = (0..MASK_COUNT)
            .map(|pattern| penalty(&apply(&matrix, Ecc::Medium, pattern)))
            .min()
            .unwrap();

        assert_eq!(penalty(&apply(&matrix, Ecc::Medium, chosen)), best);
    }

    #[test]
    fn ties_are_broken_by_the_lowest_pattern_number() {
        // Without a deterministic tie-break the same deck would produce
        // different SVGs between builds, and every cache downstream would miss.
        let matrix = placed(4, Ecc::High, 0x00);

        assert_eq!(apply_best(&matrix, Ecc::High).0, apply_best(&matrix, Ecc::High).0);
    }

    #[test]
    fn the_chosen_mask_is_recorded_in_the_format_information() {
        // A reader has no other way to know which mask to undo.
        let matrix = placed(5, Ecc::Low, 0x9A);
        let (pattern, masked) = apply_best(&matrix, Ecc::Low);
        let expected = format::format_bits(Ecc::Low, pattern);

        let written: u32 = (0..6)
            .map(|index| (index, masked.get(8, index)))
            .fold(0u32, |bits, (index, dark)| bits | (u32::from(dark) << index));

        assert_eq!(written, expected & 0b11_1111);
    }
}
