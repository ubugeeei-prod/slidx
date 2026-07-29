//! The open documents, and what has been derived from each.
//!
//! # Incremental in two places
//!
//! **Text.** The server advertises incremental sync, so an edit arrives as a
//! range and a replacement rather than the whole file. That is what keeps a
//! keystroke from re-sending an eighty-slide deck.
//!
//! **Analysis.** An edit invalidates the analysis and computes nothing. The
//! parse happens the first time something asks for it, and the result is held
//! until the next edit, so a burst of typing followed by one request costs one
//! parse. The server arranges for that burst to be drained before it asks —
//! see [`crate::server::Server::flush`].
//!
//! # The last trustworthy outline
//!
//! A document also remembers the most recent analysis whose outline could be
//! believed. While a code fence is half-typed the current parse sees one
//! enormous slide, and an outline pane that empties every time an author opens
//! a fence is worse than one that is a few seconds stale. The stale one is
//! served against the *current* line index, so every range it produces is a
//! position that exists in the document on screen.

use std::collections::HashMap;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::analysis::{analyze, Analysis};
use crate::position::{LineIndex, PositionEncoding, Range};

/// One edit, as `textDocument/didChange` states it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChange {
    /// Absent for a full-document replacement, which a client may send even
    /// when incremental sync was negotiated.
    #[serde(default)]
    pub range: Option<Range>,
    pub text: String,
}

/// One open file, its text, and what has been derived from it.
#[derive(Debug)]
pub struct TextDocument {
    pub uri: String,
    pub version: i64,
    text: String,
    index: LineIndex,
    analysis: Option<Rc<Analysis>>,
    trusted: Option<Rc<Analysis>>,
    parses: u64,
}

impl TextDocument {
    pub fn new(uri: impl Into<String>, version: i64, text: String) -> Self {
        Self {
            uri: uri.into(),
            version,
            index: LineIndex::new(&text),
            text,
            analysis: None,
            trusted: None,
            parses: 0,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn index(&self) -> &LineIndex {
        &self.index
    }

    /// How many analyses this document has cost.
    ///
    /// Exposed because "typing does not re-parse per character" is a claim
    /// about behaviour, and a claim about behaviour needs a test.
    pub fn parse_count(&self) -> u64 {
        self.parses
    }

    /// Applies one edit, invalidating whatever was derived from the old text.
    ///
    /// A range that does not fit the document is applied as far as it fits
    /// rather than rejected: the alternative is a server that stops updating
    /// and gives no sign, which is indistinguishable from a crash.
    pub fn apply(&mut self, change: &ContentChange, encoding: PositionEncoding) {
        match change.range {
            Some(range) => {
                let start = self.index.offset_of(&self.text, range.start, encoding);
                let end = self.index.offset_of(&self.text, range.end, encoding).max(start);
                self.text.replace_range(start..end, &change.text);
            }
            None => self.text = change.text.clone(),
        }

        self.index = LineIndex::new(&self.text);
        self.analysis = None;
    }

    /// The analysis of the current text, parsing it if that has not happened.
    pub fn analysis(&mut self) -> Rc<Analysis> {
        if let Some(analysis) = &self.analysis {
            return Rc::clone(analysis);
        }

        let fresh = Rc::new(analyze(&self.text));
        self.parses += 1;

        if fresh.outline_is_trustworthy() {
            self.trusted = Some(Rc::clone(&fresh));
        }

        self.analysis = Some(Rc::clone(&fresh));
        fresh
    }

    /// The analysis an outline may be built from.
    ///
    /// The current one whenever it can be believed, and otherwise the last one
    /// that could. Falls back to the current analysis when there has never
    /// been a trustworthy one — a file that opened mid-fence has nothing
    /// better to offer, and one slide is still better than none.
    pub fn outline_analysis(&mut self) -> Rc<Analysis> {
        let current = self.analysis();
        if current.outline_is_trustworthy() {
            return current;
        }

        self.trusted.clone().unwrap_or(current)
    }
}

/// Every document the client has opened.
#[derive(Debug, Default)]
pub struct DocumentStore {
    documents: HashMap<String, TextDocument>,
}

impl DocumentStore {
    pub fn open(&mut self, uri: impl Into<String>, version: i64, text: String) {
        let uri = uri.into();
        self.documents.insert(uri.clone(), TextDocument::new(uri, version, text));
    }

    /// Applies a batch of edits in the order the client sent them.
    ///
    /// Returns false for a document that was never opened, which is a client
    /// bug rather than something to invent state for.
    pub fn change(
        &mut self,
        uri: &str,
        version: i64,
        changes: &[ContentChange],
        encoding: PositionEncoding,
    ) -> bool {
        let Some(document) = self.documents.get_mut(uri) else {
            return false;
        };

        for change in changes {
            document.apply(change, encoding);
        }
        document.version = version;
        true
    }

    pub fn close(&mut self, uri: &str) {
        self.documents.remove(uri);
    }

    pub fn get(&self, uri: &str) -> Option<&TextDocument> {
        self.documents.get(uri)
    }

    pub fn get_mut(&mut self, uri: &str) -> Option<&mut TextDocument> {
        self.documents.get_mut(uri)
    }

    pub fn is_open(&self, uri: &str) -> bool {
        self.documents.contains_key(uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;

    const URI: &str = "file:///deck.md";

    fn store_with(text: &str) -> DocumentStore {
        let mut store = DocumentStore::default();
        store.open(URI, 1, text.to_string());
        store
    }

    fn edit(range: Range, text: &str) -> ContentChange {
        ContentChange { range: Some(range), text: text.to_string() }
    }

    fn at(line: u32, character: u32) -> Position {
        Position::new(line, character)
    }

    #[test]
    fn an_edit_replaces_only_the_range_it_names() {
        let mut store = store_with("# One\n\n- a\n");
        let change = edit(Range::new(at(2, 2), at(2, 3)), "b");

        assert!(store.change(URI, 2, &[change], PositionEncoding::Utf16));
        assert_eq!(store.get(URI).unwrap().text(), "# One\n\n- b\n");
        assert_eq!(store.get(URI).unwrap().version, 2);
    }

    #[test]
    fn an_insertion_is_an_edit_over_an_empty_range() {
        let mut store = store_with("# One\n");
        let change = edit(Range::new(at(0, 5), at(0, 5)), " and a half");

        store.change(URI, 2, &[change], PositionEncoding::Utf16);
        assert_eq!(store.get(URI).unwrap().text(), "# One and a half\n");
    }

    #[test]
    fn an_edit_after_japanese_text_lands_where_the_client_meant_it() {
        // The whole reason position encoding is its own module: a column
        // counted in bytes would splice this in the middle of a kanji.
        let mut store = store_with("# 高速なデッキ\n");
        let change = edit(Range::new(at(0, 8), at(0, 8)), "！");

        store.change(URI, 2, &[change], PositionEncoding::Utf16);
        assert_eq!(store.get(URI).unwrap().text(), "# 高速なデッキ！\n");
    }

    #[test]
    fn a_batch_of_edits_applies_in_order() {
        // Each range is stated against the document as the one before it left
        // it, so applying them out of order corrupts the text.
        let mut store = store_with("abc\n");
        let changes = vec![
            edit(Range::new(at(0, 0), at(0, 1)), "x"),
            edit(Range::new(at(0, 1), at(0, 2)), "y"),
        ];

        store.change(URI, 2, &changes, PositionEncoding::Utf16);
        assert_eq!(store.get(URI).unwrap().text(), "xyc\n");
    }

    #[test]
    fn a_change_with_no_range_replaces_the_whole_document() {
        let mut store = store_with("# One\n");
        let change = ContentChange { range: None, text: "# Two\n".into() };

        store.change(URI, 2, &[change], PositionEncoding::Utf16);
        assert_eq!(store.get(URI).unwrap().text(), "# Two\n");
    }

    #[test]
    fn an_edit_to_a_document_that_was_never_opened_is_refused() {
        let mut store = DocumentStore::default();
        assert!(!store.change(URI, 2, &[], PositionEncoding::Utf16));
    }

    #[test]
    fn a_stale_range_truncates_rather_than_panicking() {
        // Reachable whenever a client measured against a revision the server
        // has already replaced.
        let mut store = store_with("ab\n");
        let change = edit(Range::new(at(0, 1), at(9, 9)), "!");

        store.change(URI, 2, &[change], PositionEncoding::Utf16);
        assert_eq!(store.get(URI).unwrap().text(), "a!");
    }

    #[test]
    fn a_burst_of_edits_costs_one_parse_not_one_per_keystroke() {
        let mut store = store_with("# One\n");

        for character in 0..10u32 {
            let position = at(0, 5 + character);
            store.change(
                URI,
                2,
                &[edit(Range::new(position, position), "x")],
                PositionEncoding::Utf16,
            );
        }

        let document = store.get_mut(URI).unwrap();
        document.analysis();
        assert_eq!(document.parse_count(), 1);
    }

    #[test]
    fn asking_twice_without_an_edit_between_parses_once() {
        let mut store = store_with("# One\n");
        let document = store.get_mut(URI).unwrap();

        document.analysis();
        document.analysis();
        assert_eq!(document.parse_count(), 1);
    }

    #[test]
    fn an_edit_invalidates_what_was_derived_from_the_old_text() {
        let mut store = store_with("# One\n");
        store.get_mut(URI).unwrap().analysis();

        let change = ContentChange { range: None, text: "# One\n\n---\n\n# Two\n".into() };
        store.change(URI, 2, &[change], PositionEncoding::Utf16);

        let document = store.get_mut(URI).unwrap();
        assert_eq!(document.analysis().deck.slides.len(), 2);
        assert_eq!(document.parse_count(), 2);
    }

    #[test]
    fn a_half_typed_fence_falls_back_to_the_last_outline_that_could_be_believed() {
        let mut store = store_with("# One\n\n---\n\n# Two\n");
        assert_eq!(store.get_mut(URI).unwrap().outline_analysis().deck.slides.len(), 2);

        let change =
            ContentChange { range: None, text: "```rust\n\n# One\n\n---\n\n# Two\n".into() };
        store.change(URI, 2, &[change], PositionEncoding::Utf16);

        let document = store.get_mut(URI).unwrap();
        assert_eq!(document.analysis().deck.slides.len(), 1, "the parse really does collapse");
        assert_eq!(
            document.outline_analysis().deck.slides.len(),
            2,
            "but the outline does not empty while the author closes the fence"
        );
    }

    #[test]
    fn closing_the_fence_returns_the_outline_to_the_current_text() {
        let mut store = store_with("```rust\nlet a = 1;\n");
        assert_eq!(store.get_mut(URI).unwrap().outline_analysis().deck.slides.len(), 1);

        let change = ContentChange {
            range: None,
            text: "```rust\nlet a = 1;\n```\n\n---\n\n# Two\n".into(),
        };
        store.change(URI, 2, &[change], PositionEncoding::Utf16);

        assert_eq!(store.get_mut(URI).unwrap().outline_analysis().deck.slides.len(), 2);
    }

    #[test]
    fn a_document_opened_inside_a_fence_still_offers_what_it_has() {
        // There is no earlier outline to fall back to, and one slide beats
        // nothing at all.
        let mut store = store_with("```rust\nlet a = 1;\n");
        assert_eq!(store.get_mut(URI).unwrap().outline_analysis().deck.slides.len(), 1);
    }

    #[test]
    fn closing_a_document_forgets_it() {
        let mut store = store_with("# One\n");
        store.close(URI);

        assert!(!store.is_open(URI));
        assert!(store.get(URI).is_none());
    }
}
