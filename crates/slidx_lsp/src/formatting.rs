//! `textDocument/formatting`, as a list of `TextEdit`s.
//!
//! # Why this is not "format the document"
//!
//! The obvious implementation of an LSP formatter is one edit replacing the
//! whole file. Every editor accepts it, and it is what a formatter that
//! reserialises a document has to send.
//!
//! It would also throw away the thing [`slidx_fmt`] is for. A whole-file
//! replacement moves the cursor, collapses the undo history into one step,
//! discards folds and marks, and — in a client that trusts the edit rather than
//! diffing it — rewrites every line of a file where three characters changed.
//! The formatter already knows exactly which bytes it is changing, so the
//! protocol is told exactly that.
//!
//! # What it costs
//!
//! One conversion, and it is the one this crate exists to get right: a splice is
//! a byte range and a `TextEdit` is a pair of line/column positions counted in
//! the client's encoding. A deck with Japanese in it lands every edit in the
//! wrong column if that is wrong, so it goes through [`crate::position`] like
//! everything else.

use serde::{Deserialize, Serialize};
use slidx_core::DeckParseOptions;

use crate::position::{LineIndex, PositionEncoding, Range};

/// One replacement, in the protocol's coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

/// The edits that bring a document into canonical form.
///
/// Empty for a document that is already formatted, which is what a client needs
/// to leave the buffer — and its undo stack — untouched.
pub fn format(text: &str, index: &LineIndex, encoding: PositionEncoding) -> Vec<TextEdit> {
    slidx_fmt::plan(text, &DeckParseOptions::default())
        .splices()
        .iter()
        .map(|splice| TextEdit {
            range: Range::new(
                index.position_of(text, splice.span.start, encoding),
                index.position_of(text, splice.span.end, encoding),
            ),
            new_text: splice.text.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edits(text: &str) -> Vec<TextEdit> {
        format(text, &LineIndex::new(text), PositionEncoding::Utf16)
    }

    /// Applies the edits the way a client does, last first so earlier ranges
    /// stay valid.
    fn applied(text: &str) -> String {
        let index = LineIndex::new(text);
        let encoding = PositionEncoding::Utf16;
        let mut out = text.to_string();

        for edit in edits(text).into_iter().rev() {
            let start = index.offset_of(text, edit.range.start, encoding);
            let end = index.offset_of(text, edit.range.end, encoding);
            out.replace_range(start..end, &edit.new_text);
        }

        out
    }

    #[test]
    fn a_document_already_formatted_produces_no_edits() {
        // Not one edit that changes nothing. An editor handed a whole-file
        // replacement moves the cursor and collapses the undo stack, and doing
        // that on every save of an already-clean file is a bug the author feels
        // rather than sees.
        assert!(edits("---\ntitle: T\n---\n\n# One\n").is_empty());
    }

    #[test]
    fn one_edit_names_only_the_construct_that_changes() {
        let found = edits("# One\n\n- a <!--step-->\n");

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].range.start.line, 2);
        assert_eq!(found[0].range.start.character, 4);
        assert_eq!(found[0].new_text, "<!-- step -->");
    }

    #[test]
    fn applying_the_edits_gives_what_the_formatter_gives() {
        // The protocol path and the file path have to agree, or `slidx fmt` in
        // CI fights the editor over the same file.
        for source in [
            "---\ntheme: minimal\ntitle: T\n---\n\n- a <!--step-->\n\n<!--note: x-->\n",
            "# One\n\n[x]{.a #k}\n\n  ---  \n\n# Two\n",
            "# 見出し\n\n結果は [3.2倍速く]{color=danger #結果} なった。\n",
        ] {
            assert_eq!(
                applied(source),
                slidx_fmt::format(source, &DeckParseOptions::default()),
                "{source:?}"
            );
        }
    }

    #[test]
    fn an_edit_on_a_japanese_line_is_measured_in_code_units() {
        // The failure this crate's `position` module exists to prevent: an edit
        // placed by byte offset lands mid-character and the client either
        // rejects it or corrupts the line.
        let source = "# 見出し\n\n結果は [3.2倍速く]{color=danger #結果} なった。\n";
        let found = edits(source);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].range.start.line, 2);
        // `結果は [3.2倍速く]` is 12 UTF-16 code units, and 24 bytes. An edit
        // placed at 24 would land in the middle of `なった`.
        assert_eq!(found[0].range.start.character, 12);
        assert_eq!(found[0].new_text, "{#結果 color=danger}");
    }

    #[test]
    fn an_emoji_outside_the_basic_plane_is_two_code_units() {
        // A surrogate pair is what separates a real conversion from one that
        // counts characters, so it gets its own case.
        let source = "# One\n\n🎤 [x]{.a #k}\n";
        let found = edits(source);

        // Two units for the emoji, then ` [x]`. Counted as characters it would
        // be 5, and counted in bytes 8.
        assert_eq!(found[0].range.start.character, 6);
    }

    #[test]
    fn several_constructs_produce_several_edits_in_source_order() {
        let found = edits("# One\n\n- a <!--step-->\n- b <!--step-->\n");

        assert_eq!(found.len(), 2);
        assert!(found[0].range.start.line < found[1].range.start.line);
    }

    #[test]
    fn a_text_edit_is_spelled_the_way_the_protocol_spells_it() {
        // `newText`, not `new_text`. A client that silently ignores an unknown
        // field would apply an empty replacement and delete the range.
        let json = serde_json::to_value(&edits("# One\n\n- a <!--step-->\n")[0]).unwrap();

        assert!(json.get("newText").is_some(), "{json}");
        assert!(json.get("range").is_some());
    }
}
