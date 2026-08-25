//! The module grid, and the fixed patterns a reader looks for first.
//!
//! A reader finds a code before it decodes one: the three finder patterns give
//! it the corners, the timing patterns give it the module pitch, and the
//! alignment patterns let it correct for the perspective of a camera held at an
//! angle to a screen. None of that is payload, and all of it has to be exactly
//! where the spec says.
//!
//! The grid therefore tracks two planes — what each module is, and whether it
//! belongs to a function pattern. The second is what stops data placement and
//! masking from writing over the patterns that make the code findable.

pub(crate) mod format;
pub(crate) mod place;

use crate::version::Version;

/// A square grid of modules, half-built: function patterns drawn, data not yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Matrix {
    version: Version,
    size: usize,
    dark: Vec<bool>,
    /// Modules belonging to a function pattern or reserved for one, which data
    /// placement skips and masking must leave alone.
    function: Vec<bool>,
}

impl Matrix {
    /// Builds the grid with every function pattern drawn and the format and
    /// version areas reserved.
    pub(crate) fn new(version: Version) -> Self {
        let size = version.size();
        let mut matrix = Self {
            version,
            size,
            dark: vec![false; size * size],
            function: vec![false; size * size],
        };

        matrix.draw_finders();
        matrix.draw_timing();
        matrix.draw_alignment();
        matrix.reserve_format_areas();

        matrix
    }

    pub(crate) fn version(&self) -> Version {
        self.version
    }

    pub(crate) fn size(&self) -> usize {
        self.size
    }

    pub(crate) fn get(&self, row: usize, column: usize) -> bool {
        self.dark[row * self.size + column]
    }

    pub(crate) fn set(&mut self, row: usize, column: usize, dark: bool) {
        self.dark[row * self.size + column] = dark;
    }

    pub(crate) fn is_function(&self, row: usize, column: usize) -> bool {
        self.function[row * self.size + column]
    }

    /// Writes a module that belongs to a function pattern, marking it so that
    /// nothing downstream treats it as data.
    fn set_function(&mut self, row: usize, column: usize, dark: bool) {
        let index = row * self.size + column;
        self.dark[index] = dark;
        self.function[index] = true;
    }

    pub(crate) fn into_modules(self) -> Vec<bool> {
        self.dark
    }

    /// The three 7×7 patterns that tell a reader where the code is, plus the
    /// light separator that keeps each one from merging into the data around it.
    fn draw_finders(&mut self) {
        let last = self.size - 7;
        for (top, left) in [(0usize, 0usize), (0, last), (last, 0)] {
            self.draw_finder(top as isize, left as isize);
        }
    }

    fn draw_finder(&mut self, top: isize, left: isize) {
        // The pattern is concentric rings, so a module's colour depends only on
        // its Chebyshev distance from the centre: a 3×3 dark core, a light
        // ring, a dark ring, then the separator.
        for row_offset in -1..=7isize {
            for column_offset in -1..=7isize {
                let (row, column) = (top + row_offset, left + column_offset);
                if row < 0
                    || column < 0
                    || row >= self.size as isize
                    || column >= self.size as isize
                {
                    continue;
                }

                let distance = (row_offset - 3).abs().max((column_offset - 3).abs());
                self.set_function(row as usize, column as usize, distance != 2 && distance != 4);
            }
        }
    }

    /// The alternating line along row and column 6 that a reader counts modules
    /// against. Without it a slightly warped image drifts out of alignment part
    /// way across the symbol.
    fn draw_timing(&mut self) {
        for position in 8..self.size - 8 {
            let dark = position.is_multiple_of(2);
            self.set_function(6, position, dark);
            self.set_function(position, 6, dark);
        }
    }

    fn draw_alignment(&mut self) {
        let centers = self.version.alignment_centers();
        let last = centers.len().saturating_sub(1);

        for (row_index, &row) in centers.iter().enumerate() {
            for (column_index, &column) in centers.iter().enumerate() {
                // Three of the pairings land on a finder pattern, which owns
                // that corner; the spec omits the alignment pattern there.
                let on_finder = (row_index, column_index) == (0, 0)
                    || (row_index, column_index) == (0, last)
                    || (row_index, column_index) == (last, 0);
                if on_finder {
                    continue;
                }

                self.draw_alignment_at(row, column);
            }
        }
    }

    fn draw_alignment_at(&mut self, center_row: usize, center_column: usize) {
        for row_offset in -2..=2isize {
            for column_offset in -2..=2isize {
                let distance = row_offset.abs().max(column_offset.abs());
                self.set_function(
                    (center_row as isize + row_offset) as usize,
                    (center_column as isize + column_offset) as usize,
                    distance != 1,
                );
            }
        }
    }

    /// Reserves the strips that format and version information are written into
    /// once the mask is known.
    ///
    /// They are reserved before the data is placed rather than written after,
    /// because data placement has to skip them: it fills the grid by position,
    /// and a strip that is not yet marked would take payload bits that the
    /// format bits then overwrite.
    fn reserve_format_areas(&mut self) {
        for offset in 0..9 {
            // Column and row 6 belong to the timing patterns, which run through
            // the format strips rather than being interrupted by them.
            if offset != 6 {
                self.set_function(8, offset, false);
                self.set_function(offset, 8, false);
            }
        }

        for offset in 0..8 {
            self.set_function(8, self.size - 1 - offset, false);
            self.set_function(self.size - 1 - offset, 8, false);
        }

        if self.version.carries_version_information() {
            for index in 0..18 {
                let far = self.size - 11 + index % 3;
                let near = index / 3;
                self.set_function(far, near, false);
                self.set_function(near, far, false);
            }
        }
    }

    /// How many module positions are left for the payload.
    ///
    /// Only the tests need this, but they need it badly: it is what pins the
    /// function patterns to the spec's codeword totals.
    #[cfg(test)]
    pub(crate) fn data_module_count(&self) -> usize {
        self.function.iter().filter(|reserved| !**reserved).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Remainder bits per version: module positions left over once the
    /// codewords have been placed, filled with zeroes. From ISO/IEC 18004
    /// table 1, and independent of everything this module computes.
    const REMAINDER_BITS: [usize; 10] = [0, 7, 7, 7, 7, 7, 0, 0, 0, 0];

    fn matrix(version: u8) -> Matrix {
        Matrix::new(Version::new(version).unwrap())
    }

    fn is_finder(matrix: &Matrix, top: usize, left: usize) -> bool {
        (0..7).all(|row| {
            (0..7).all(|column| {
                let distance = (row as isize - 3).abs().max((column as isize - 3).abs());
                matrix.get(top + row, left + column) == (distance != 2)
            })
        })
    }

    #[test]
    fn a_finder_pattern_sits_in_three_of_the_four_corners() {
        // Three, not four: the missing corner is how a reader recovers the
        // symbol's rotation.
        for version in Version::ALL {
            let matrix = Matrix::new(version);
            let last = matrix.size() - 7;

            assert!(is_finder(&matrix, 0, 0), "version {}", version.value());
            assert!(is_finder(&matrix, 0, last), "version {}", version.value());
            assert!(is_finder(&matrix, last, 0), "version {}", version.value());
        }
    }

    #[test]
    fn the_fourth_corner_carries_data_rather_than_a_finder() {
        let matrix = matrix(1);
        let last = matrix.size() - 7;

        assert!(!is_finder(&matrix, last, last));
    }

    #[test]
    fn each_finder_is_ringed_by_a_light_separator() {
        // Without the separator the finder merges into adjacent dark data and
        // stops matching the 1:1:3:1:1 ratio a reader scans for.
        let matrix = matrix(2);

        for offset in 0..8 {
            assert!(!matrix.get(7, offset), "row below the top-left finder");
            assert!(!matrix.get(offset, 7), "column beside the top-left finder");
        }
    }

    #[test]
    fn the_timing_patterns_alternate_across_the_whole_symbol() {
        let matrix = matrix(7);

        for position in 8..matrix.size() - 8 {
            assert_eq!(matrix.get(6, position), position.is_multiple_of(2));
            assert_eq!(matrix.get(position, 6), position.is_multiple_of(2));
        }
    }

    #[test]
    fn the_timing_pattern_meets_each_finder_on_a_dark_module() {
        // The finder's dark outer ring continues the alternation; a parity
        // error here breaks the module count a reader derives from the line.
        let matrix = matrix(4);

        assert!(matrix.get(6, 6));
        assert!(matrix.get(6, matrix.size() - 7));
    }

    #[test]
    fn version_one_has_no_alignment_pattern() {
        // There is nowhere to put one: every alignment coordinate would
        // coincide with a finder, so the middle of the symbol is plain data.
        let matrix = matrix(1);

        assert!(!matrix.is_function(10, 10));
        assert!(!matrix.is_function(12, 12));
    }

    #[test]
    fn an_alignment_pattern_is_a_dark_ring_around_a_dark_centre() {
        let matrix = matrix(2);

        assert!(matrix.get(18, 18), "centre");
        assert!(!matrix.get(17, 18), "light ring");
        assert!(matrix.get(16, 18), "dark ring");
        assert!(matrix.get(16, 16), "corner of the dark ring");
    }

    #[test]
    fn alignment_patterns_are_omitted_where_a_finder_already_owns_the_corner() {
        // Version 7 has three coordinates, so six of the nine pairings carry a
        // pattern. Drawing all nine would put a pattern inside each finder.
        let matrix = matrix(7);
        let centers = Version::new(7).unwrap().alignment_centers();
        assert_eq!(centers, [6, 22, 38]);

        // A pattern at (6, 38) or (38, 6) would reach past its finder into
        // these modules, which are payload in a correct symbol.
        assert!(!matrix.is_function(7, 36), "a pattern was drawn beside the top-right finder");
        assert!(!matrix.is_function(36, 7), "a pattern was drawn beside the bottom-left finder");
        assert!(matrix.is_function(36, 36), "the (38, 38) pattern belongs in the free corner");
        assert!(matrix.get(20, 20), "the (22, 22) pattern is drawn");
    }

    #[test]
    fn an_alignment_pattern_crossing_a_timing_line_agrees_with_it() {
        // The (6, 22) pattern's centre row is the horizontal timing line. The
        // two are consistent in the spec, so whichever is drawn second must not
        // change the modules the other already set.
        let matrix = matrix(7);

        for column in 20..=24 {
            assert_eq!(matrix.get(6, column), column.is_multiple_of(2));
        }
    }

    #[test]
    fn the_data_capacity_of_the_grid_matches_the_codewords_the_spec_assigns_it() {
        // The strongest check available on the function patterns: if any pattern
        // is the wrong size, in the wrong place, or missing, the count of
        // remaining modules stops matching the published codeword totals.
        for version in Version::ALL {
            let matrix = Matrix::new(version);
            let expected = version.layout(crate::options::Ecc::Low).total_codewords() * 8
                + REMAINDER_BITS[version.value() as usize - 1];

            assert_eq!(
                matrix.data_module_count(),
                expected,
                "version {} leaves the wrong number of data modules",
                version.value()
            );
        }
    }

    #[test]
    fn the_format_strips_are_reserved_before_any_data_is_placed() {
        // Data placement fills by position; an unreserved strip would take
        // payload bits that the format bits later overwrite.
        let matrix = matrix(1);

        assert!(matrix.is_function(8, 0));
        assert!(matrix.is_function(0, 8));
        assert!(matrix.is_function(8, matrix.size() - 1));
        assert!(matrix.is_function(matrix.size() - 1, 8));
    }

    #[test]
    fn version_information_areas_are_reserved_only_from_version_seven() {
        let small = matrix(6);
        let large = matrix(7);

        assert!(!small.is_function(small.size() - 11, 0));
        assert!(large.is_function(large.size() - 11, 0));
        assert!(large.is_function(0, large.size() - 11));
    }
}
