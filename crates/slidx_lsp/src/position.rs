//! Converting between the editor's coordinates and Rust's.
//!
//! # Why this is its own module
//!
//! LSP counts a column in **UTF-16 code units**. Rust counts a string in
//! **bytes**. For ASCII the two agree, which is exactly what makes this
//! dangerous: every conversion looks correct until a deck contains Japanese,
//! and then every diagnostic on that line lands in the wrong column. The
//! author of this project writes Japanese, so the failure is not hypothetical.
//!
//! The three units that matter, on one line:
//!
//! | text | bytes | UTF-16 units | code points |
//! |---|---|---|---|
//! | `a` | 1 | 1 | 1 |
//! | `あ` | 3 | 1 | 1 |
//! | `🎤` | 4 | 2 | 1 |
//!
//! An emoji outside the basic plane is a surrogate *pair* in UTF-16, so it is
//! the case that separates a real conversion from one that only counts
//! characters.
//!
//! Every position that crosses the protocol boundary goes through here, and
//! nothing else in the crate is allowed to index a line by column.

use serde::{Deserialize, Serialize};

/// Which unit a client counts columns in.
///
/// Negotiated at `initialize`. UTF-16 is the protocol default and the only
/// encoding every client is required to support, so it is what an
/// unnegotiated session gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionEncoding {
    Utf8,
    #[default]
    Utf16,
    Utf32,
}

impl PositionEncoding {
    /// The wire token, as it appears in `positionEncoding`.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
            Self::Utf16 => "utf-16",
            Self::Utf32 => "utf-32",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "utf-8" => Some(Self::Utf8),
            "utf-16" => Some(Self::Utf16),
            "utf-32" => Some(Self::Utf32),
            _ => None,
        }
    }

    /// How many units `c` occupies in this encoding.
    fn width(self, c: char) -> u32 {
        match self {
            Self::Utf8 => c.len_utf8() as u32,
            Self::Utf16 => c.len_utf16() as u32,
            Self::Utf32 => 1,
        }
    }

    /// Length of a whole line, in this encoding's units.
    pub fn measure(self, line: &str) -> u32 {
        match self {
            Self::Utf8 => line.len() as u32,
            _ => line.chars().map(|c| self.width(c)).sum(),
        }
    }

    /// Byte offset within `line` of the given column.
    ///
    /// A column that lands inside a multi-unit character rounds down to that
    /// character's start, and one past the end clamps to the end. Both are
    /// reachable from a client that measured against a slightly different
    /// revision of the text, and neither may panic.
    pub fn byte_of_column(self, line: &str, column: u32) -> usize {
        if let Self::Utf8 = self {
            return clamp_to_boundary(line, column as usize);
        }

        let mut counted = 0u32;
        for (offset, c) in line.char_indices() {
            let next = counted + self.width(c);
            if column < next {
                return offset;
            }
            counted = next;
        }
        line.len()
    }

    /// Column of the given byte offset within `line`.
    pub fn column_of_byte(self, line: &str, byte: usize) -> u32 {
        let byte = clamp_to_boundary(line, byte);
        match self {
            Self::Utf8 => byte as u32,
            _ => line[..byte].chars().map(|c| self.width(c)).sum(),
        }
    }
}

/// Moves an arbitrary byte index onto the nearest character boundary at or
/// below it, so a stale client offset truncates rather than panicking.
fn clamp_to_boundary(line: &str, byte: usize) -> usize {
    let mut byte = byte.min(line.len());
    while !line.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// A zero-based position, as the protocol states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

/// A half-open span between two positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

/// Line starts for one document, so a position lookup is a binary search
/// rather than a scan from the top of the file.
///
/// Rebuilt whenever the text changes. That is O(n) per edit, which is the one
/// unavoidable whole-document cost of an edit — an eighty-slide deck is tens
/// of kilobytes, and scanning it for newlines is not what makes an editor feel
/// slow.
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset of the first character of each line. Always non-empty.
    starts: Vec<usize>,
    length: usize,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0usize];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, byte)| *byte == b'\n')
                .map(|(offset, _)| offset + 1),
        );

        Self { starts, length: text.len() }
    }

    /// Number of lines, counting a trailing newline as ending the last line
    /// rather than starting an empty one the author cannot put a cursor on.
    pub fn line_count(&self) -> u32 {
        self.starts.len() as u32
    }

    /// The text of one zero-based line, without its terminator.
    ///
    /// `\r\n` is stripped so a column measured by a client on Windows counts
    /// the same units as one measured anywhere else.
    pub fn line_text<'a>(&self, text: &'a str, line: u32) -> &'a str {
        let Some(start) = self.starts.get(line as usize).copied() else {
            return "";
        };
        let end = self.starts.get(line as usize + 1).copied().unwrap_or(self.length);

        text[start..end].trim_end_matches('\n').trim_end_matches('\r')
    }

    /// Byte offset of a protocol position.
    pub fn offset_of(&self, text: &str, position: Position, encoding: PositionEncoding) -> usize {
        let Some(start) = self.starts.get(position.line as usize).copied() else {
            return self.length;
        };

        start + encoding.byte_of_column(self.line_text(text, position.line), position.character)
    }

    /// Protocol position of a byte offset.
    pub fn position_of(&self, text: &str, offset: usize, encoding: PositionEncoding) -> Position {
        let offset = offset.min(self.length);
        let line = match self.starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(next) => next - 1,
        };

        let column =
            encoding.column_of_byte(self.line_text(text, line as u32), offset - self.starts[line]);

        Position::new(line as u32, column)
    }

    /// The whole of a one-based source line, as a range.
    ///
    /// This is the shape a slidx diagnostic has: [`slidx_core::SourceSpan`]
    /// carries a line and no column, because the parser reports what an author
    /// wrote on a line rather than which byte of it was wrong. Underlining the
    /// line is the honest rendering of that.
    pub fn line_range(&self, text: &str, line: u32, encoding: PositionEncoding) -> Range {
        let line = line.saturating_sub(1).min(self.line_count().saturating_sub(1));
        let width = encoding.measure(self.line_text(text, line));

        Range::new(Position::new(line, 0), Position::new(line, width))
    }

    /// A range spanning whole lines, both one-based and inclusive.
    pub fn lines_range(
        &self,
        text: &str,
        first: u32,
        last: u32,
        encoding: PositionEncoding,
    ) -> Range {
        Range::new(
            self.line_range(text, first, encoding).start,
            self.line_range(text, last, encoding).end,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JA: &str = "# 高速なデッキ\n- あ\n";
    /// A microphone, outside the basic plane: two UTF-16 units, four bytes.
    const ASTRAL: &str = "talk 🎤 end";

    #[test]
    fn a_japanese_line_is_measured_in_code_units_not_bytes() {
        let line = "# 高速なデッキ";

        assert_eq!(PositionEncoding::Utf16.measure(line), 8, "two ASCII plus six kanji and kana");
        assert_eq!(PositionEncoding::Utf8.measure(line), 20, "each of the six costs three bytes");
        assert_eq!(PositionEncoding::Utf32.measure(line), 8);
    }

    #[test]
    fn an_astral_emoji_is_two_utf16_units_and_one_code_point() {
        assert_eq!(PositionEncoding::Utf16.measure(ASTRAL), 11);
        assert_eq!(PositionEncoding::Utf32.measure(ASTRAL), 10);
        assert_eq!(PositionEncoding::Utf8.measure(ASTRAL), 13);
    }

    #[test]
    fn a_utf16_column_after_japanese_text_finds_the_right_byte() {
        let line = "# 高速なデッキ";

        // Column 2 is the first kanji, which starts at byte 2.
        assert_eq!(PositionEncoding::Utf16.byte_of_column(line, 2), 2);
        // Column 4 is two kanji further in: 2 + 3 + 3.
        assert_eq!(PositionEncoding::Utf16.byte_of_column(line, 4), 8);
        assert_eq!(PositionEncoding::Utf16.column_of_byte(line, 8), 4);
    }

    #[test]
    fn a_column_landing_inside_a_surrogate_pair_rounds_to_its_start() {
        // A client that split the pair would otherwise slice a char in half.
        assert_eq!(PositionEncoding::Utf16.byte_of_column(ASTRAL, 6), 5);
        assert_eq!(PositionEncoding::Utf16.byte_of_column(ASTRAL, 5), 5);
        assert_eq!(PositionEncoding::Utf16.byte_of_column(ASTRAL, 7), 9);
    }

    #[test]
    fn a_column_past_the_end_of_a_line_clamps_instead_of_panicking() {
        // Reachable whenever a client measured against a revision the server
        // has already replaced.
        assert_eq!(PositionEncoding::Utf16.byte_of_column("ab", 99), 2);
        assert_eq!(PositionEncoding::Utf8.byte_of_column("あ", 2), 0, "rounds to the boundary");
        assert_eq!(PositionEncoding::Utf16.column_of_byte("あ", 99), 1);
    }

    #[test]
    fn positions_and_offsets_round_trip_through_non_ascii_text() {
        let index = LineIndex::new(JA);

        for encoding in [PositionEncoding::Utf8, PositionEncoding::Utf16, PositionEncoding::Utf32] {
            for offset in (0..JA.len()).filter(|offset| JA.is_char_boundary(*offset)) {
                let position = index.position_of(JA, offset, encoding);
                assert_eq!(
                    index.offset_of(JA, position, encoding),
                    offset,
                    "{encoding:?} lost byte {offset}"
                );
            }
        }
    }

    #[test]
    fn a_line_is_located_without_its_terminator() {
        let index = LineIndex::new("one\ntwo\r\nthree");

        assert_eq!(index.line_text("one\ntwo\r\nthree", 0), "one");
        assert_eq!(index.line_text("one\ntwo\r\nthree", 1), "two", "a CR is not part of the line");
        assert_eq!(index.line_text("one\ntwo\r\nthree", 2), "three");
        assert_eq!(index.line_count(), 3);
    }

    #[test]
    fn a_trailing_newline_does_not_invent_a_line() {
        assert_eq!(LineIndex::new("one\n").line_count(), 2, "an author can type on line two");
        assert_eq!(LineIndex::new("").line_count(), 1);
    }

    #[test]
    fn a_one_based_diagnostic_line_underlines_that_whole_line() {
        let index = LineIndex::new(JA);
        let range = index.line_range(JA, 1, PositionEncoding::Utf16);

        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(0, 8), "the whole heading, in UTF-16 units");
    }

    #[test]
    fn a_diagnostic_past_the_end_of_the_file_still_points_somewhere() {
        // A span can outlive the line it described by one keystroke.
        let index = LineIndex::new("one\ntwo");
        let range = index.line_range("one\ntwo", 99, PositionEncoding::Utf16);

        assert_eq!(range.start.line, 1);
    }

    #[test]
    fn a_slide_spans_from_its_first_line_to_its_last() {
        let text = "# One\n\n---\n\n# Two\n";
        let index = LineIndex::new(text);
        let range = index.lines_range(text, 1, 2, PositionEncoding::Utf16);

        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(1, 0), "line two is empty");
    }

    #[test]
    fn encodings_round_trip_through_their_wire_tokens() {
        for encoding in [PositionEncoding::Utf8, PositionEncoding::Utf16, PositionEncoding::Utf32] {
            assert_eq!(PositionEncoding::parse(encoding.as_token()), Some(encoding));
        }
        assert_eq!(PositionEncoding::parse("utf-7"), None);
    }
}
