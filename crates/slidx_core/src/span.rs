//! Byte ranges into a deck source.
//!
//! A [`SourceSpan`](crate::SourceSpan) points a human at a line. This points a
//! program at bytes, which is a different job: the visual editor changes a deck
//! by splicing a range of the file the author wrote, and a line number is not
//! precise enough to splice with.
//!
//! Spans are half-open and measured in bytes rather than characters, so they
//! index straight into the source without a scan. Every accessor is total —
//! a stale span, or one that arrived over the WASM boundary from an editor
//! holding an older copy of the file, yields nothing rather than a panic.

use serde::{Deserialize, Serialize};

/// A half-open byte range `start..end` in a source string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// A zero-width span, which is where an insertion goes.
    pub fn empty(at: usize) -> Self {
        Self { start: at, end: at }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// The text this span names, or `""` when it does not name any.
    ///
    /// Total by design: a span may outlive the source it was computed from —
    /// an editor can hold one across a keystroke — and the answer to a stale
    /// span is nothing, never a crash.
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        source.get(self.start..self.end).unwrap_or("")
    }

    /// True when `other` lies entirely inside this span.
    pub fn contains(&self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// This span shifted into a document that contains `source` at `offset`.
    ///
    /// Marks and headings are found inside a slide's body; the splice that
    /// changes one has to be expressed against the whole file.
    pub fn shifted(&self, offset: usize) -> Self {
        Self { start: self.start + offset, end: self.end + offset }
    }
}

impl From<ByteSpan> for std::ops::Range<usize> {
    fn from(span: ByteSpan) -> Self {
        span.start..span.end
    }
}

impl From<std::ops::Range<usize>> for ByteSpan {
    fn from(range: std::ops::Range<usize>) -> Self {
        Self { start: range.start, end: range.end }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_slices_the_text_it_names() {
        assert_eq!(ByteSpan::new(2, 5).slice("abcdefg"), "cde");
    }

    #[test]
    fn a_span_that_outran_its_source_slices_to_nothing() {
        // The editor holds spans across keystrokes. A stale one has to be
        // survivable, because the alternative is a panic in a live preview.
        assert_eq!(ByteSpan::new(2, 500).slice("abc"), "");
        assert_eq!(ByteSpan::new(500, 2).slice("abc"), "");
    }

    #[test]
    fn a_span_that_splits_a_character_slices_to_nothing() {
        assert_eq!(ByteSpan::new(0, 1).slice("あ"), "");
    }

    #[test]
    fn an_empty_span_names_an_insertion_point() {
        let span = ByteSpan::empty(4);
        assert!(span.is_empty());
        assert_eq!(span.len(), 0);
        assert_eq!(span.start, 4);
    }

    #[test]
    fn a_reversed_span_reports_no_length_rather_than_underflowing() {
        assert_eq!(ByteSpan::new(9, 2).len(), 0);
    }

    #[test]
    fn containment_is_inclusive_of_the_edges() {
        let outer = ByteSpan::new(0, 10);
        assert!(outer.contains(ByteSpan::new(0, 10)));
        assert!(outer.contains(ByteSpan::new(3, 4)));
        assert!(!outer.contains(ByteSpan::new(3, 11)));
    }

    #[test]
    fn shifting_moves_a_span_into_the_enclosing_document() {
        assert_eq!(ByteSpan::new(1, 3).shifted(10), ByteSpan::new(11, 13));
    }
}
