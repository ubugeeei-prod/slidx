//! Walking the codeword stream into the grid.
//!
//! The path is a boustrophedon over two-module-wide columns, right to left,
//! reversing direction at each column pair. It is not a raster scan, and the
//! reason matters: consecutive codewords end up physically adjacent, so a
//! reader that loses a region loses whole codewords rather than one bit from
//! each of eight, which is what Reed–Solomon is able to repair.

use crate::bits;
use crate::matrix::Matrix;

/// The module positions the payload occupies, in the order it occupies them.
///
/// Separated from [`fill`] because the walk is the part with the interesting
/// failure modes — a revisited module, a skipped one, a column pair running the
/// wrong way — and none of them are visible in a finished grid.
pub(crate) fn positions(matrix: &Matrix) -> Vec<(usize, usize)> {
    let size = matrix.size();
    let mut path = Vec::with_capacity(size * size);
    let mut upward = true;
    let mut right = size - 1;

    loop {
        // Column 6 is the vertical timing pattern. It is skipped entirely
        // rather than stepped over, which shifts every column pair to its left
        // by one.
        if right == 6 {
            right = 5;
        }

        for step in 0..size {
            let row = if upward { size - 1 - step } else { step };

            for column in [right, right - 1] {
                if !matrix.is_function(row, column) {
                    path.push((row, column));
                }
            }
        }

        if right <= 1 {
            break;
        }

        right -= 2;
        upward = !upward;
    }

    path
}

/// Fills every non-function module with the codeword stream.
///
/// Positions past the end of the stream are the version's remainder bits and
/// are left light, which is what the spec requires of them.
pub(crate) fn fill(matrix: &mut Matrix, codewords: &[u8]) {
    for (index, (row, column)) in positions(matrix).into_iter().enumerate() {
        matrix.set(row, column, bits::bit_of(codewords, index));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::Ecc;
    use crate::version::Version;

    fn grid(version: u8) -> Matrix {
        Matrix::new(Version::new(version).unwrap())
    }

    fn filled(version: u8, codewords: &[u8]) -> Matrix {
        let mut matrix = grid(version);
        fill(&mut matrix, codewords);
        matrix
    }

    /// Which two-module column a position belongs to, counting from the right.
    /// Column 6 is not part of any pair, so everything left of it shifts by one.
    fn column_pair(size: usize, column: usize) -> usize {
        let adjusted = if column < 6 { column + 1 } else { column };
        (size - 1 - adjusted) / 2
    }

    #[test]
    fn the_walk_starts_in_the_bottom_right_corner() {
        // Readers walk the same path from the same corner; starting anywhere
        // else produces a grid that decodes to noise.
        let matrix = grid(1);
        let path = positions(&matrix);
        let size = matrix.size();

        assert_eq!(path[0], (size - 1, size - 1));
        assert_eq!(path[1], (size - 1, size - 2));
        assert_eq!(path[2], (size - 2, size - 1));
        assert_eq!(path[3], (size - 2, size - 2));
    }

    #[test]
    fn every_data_module_is_visited_exactly_once() {
        // A path that revisits a module writes two bits into one place and
        // silently drops one; a path that misses one leaves a module light that
        // should have carried a bit. Neither is visible in the finished grid.
        for version in Version::ALL {
            let matrix = Matrix::new(version);
            let path = positions(&matrix);

            let mut seen = vec![false; matrix.size() * matrix.size()];
            for &(row, column) in &path {
                let index = row * matrix.size() + column;
                assert!(!seen[index], "version {} revisits ({row}, {column})", version.value());
                assert!(!matrix.is_function(row, column), "the walk wrote over a function pattern");
                seen[index] = true;
            }

            assert_eq!(path.len(), matrix.data_module_count(), "version {}", version.value());
        }
    }

    #[test]
    fn each_column_pair_runs_opposite_to_the_one_before_it() {
        // Alternating is what keeps consecutive codewords adjacent. A walk that
        // always ran upward would still visit every module, so only the
        // direction distinguishes a correct grid from an undecodable one.
        let matrix = grid(4);
        let size = matrix.size();

        let mut rows_by_pair: Vec<Vec<usize>> = Vec::new();
        for &(row, column) in &positions(&matrix) {
            let pair = column_pair(size, column);
            while rows_by_pair.len() <= pair {
                rows_by_pair.push(Vec::new());
            }
            rows_by_pair[pair].push(row);
        }

        for (index, rows) in rows_by_pair.iter().enumerate() {
            let descending = rows.first() > rows.last();

            assert_eq!(descending, index % 2 == 0, "column pair {index} runs the wrong way");
        }
    }

    #[test]
    fn the_vertical_timing_column_is_never_written_to() {
        // Column 6 carries the timing pattern for the whole height of the
        // symbol. Treating it as an ordinary column shifts every column to its
        // left by one and corrupts the tail of the stream.
        let matrix = filled(2, &[0xFF; 44]);

        assert!(positions(&grid(2)).iter().all(|&(_, column)| column != 6));
        for row in 8..matrix.size() - 8 {
            assert_eq!(matrix.get(row, 6), row % 2 == 0, "timing survived placement");
        }
    }

    #[test]
    fn a_stream_shorter_than_the_grid_leaves_the_remainder_bits_light() {
        // Version 2 has seven module positions with no codeword behind them,
        // and the spec fills them with zeroes rather than leaving them arbitrary.
        let matrix = filled(2, &[0xFF; 44]);
        let light = positions(&grid(2))
            .into_iter()
            .filter(|&(row, column)| !matrix.get(row, column))
            .count();

        assert_eq!(light, 7);
    }

    #[test]
    fn placement_is_deterministic() {
        let codewords = vec![0x5A; 26];

        assert_eq!(filled(1, &codewords), filled(1, &codewords));
    }

    #[test]
    fn every_supported_version_fills_without_running_off_the_grid() {
        // The column walk decrements by two and has to stop cleanly at column
        // zero; an off-by-one panics on a subtraction rather than producing a
        // wrong answer, so this is a crash test as much as a correctness one.
        for version in Version::ALL {
            for ecc in Ecc::ALL {
                let total = version.layout(ecc).total_codewords();
                filled(version.value(), &vec![0xA5; total]);
            }
        }
    }
}
