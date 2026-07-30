//! What this session did, so it can be taken back.
//!
//! Every mutation returns the edit that reverses it, so an undo stack is a list
//! rather than a second model of the document. That is [`slidx_edit`]'s design
//! and this module is the whole of what it costs here.
//!
//! ## Why an agent needs this more than a person does
//!
//! A person editing a deck sees each change land. An agent makes six and is then
//! told the third was wrong — and has no reliable memory of what the file said
//! before, because it never held the bytes, only the operations. Guessing at a
//! rollback is how an agent turns one bad edit into a rewritten file.
//!
//! So the stack is last in, first out, and an entry is only ever applied to the
//! deck it came from: an inverse names byte ranges in the source it was measured
//! against, and those ranges stop meaning anything once anything else has been
//! spliced. Undoing out of order would land bytes in the middle of a word, so an
//! entry that is not on top is not offered.

use std::path::PathBuf;

use slidx_edit::Edit;

/// One change this session made, and how to take it back.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The tool that made it, for the report an undo prints.
    pub tool: &'static str,
    /// The deck it was made to. An inverse is only valid against this one.
    pub deck: PathBuf,
    pub separator: String,
    /// The deck's slide files as they were *before* the change, in deck order.
    ///
    /// Remembered rather than re-read, because removing a slide deletes the file
    /// it was alone in. Reading the directory again would find that file gone,
    /// and the restored slide would land in whichever file came after it. The
    /// remembered path keeps its place, which is where the slide goes back.
    pub files: Vec<PathBuf>,
    pub inverse: Edit,
}

/// The stack of changes this session made.
#[derive(Debug, Default)]
pub struct History {
    entries: Vec<Entry>,
}

impl History {
    /// Remembers a change.
    ///
    /// An edit that changed nothing is not remembered: undoing it would be a
    /// call that reported success and did nothing, which reads as a bug.
    pub fn record(&mut self, entry: Entry) {
        if entry.inverse.is_empty() {
            return;
        }

        self.entries.push(entry);
    }

    /// The most recent change, removed from the stack.
    pub fn take_last(&mut self) -> Option<Entry> {
        self.entries.pop()
    }

    /// How many changes are on the stack.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// What the stack holds, most recent last.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::ByteSpan;

    fn edit(text: &str) -> Edit {
        serde_json::from_value(serde_json::json!([{
            "span": { "start": 0, "end": 1 },
            "text": text,
        }]))
        .expect("an edit")
    }

    fn entry(tool: &'static str, text: &str) -> Entry {
        Entry {
            tool,
            deck: PathBuf::from("/deck/slides"),
            separator: "---".into(),
            files: vec![PathBuf::from("/deck/slides/0001.md")],
            inverse: edit(text),
        }
    }

    #[test]
    fn the_most_recent_change_is_the_one_that_comes_back_first() {
        // An inverse names byte ranges in the source it was measured against.
        // Undoing out of order would land bytes in the middle of a word.
        let mut history = History::default();
        history.record(entry("set_heading", "first"));
        history.record(entry("set_notes", "second"));

        assert_eq!(history.take_last().expect("an entry").tool, "set_notes");
        assert_eq!(history.take_last().expect("an entry").tool, "set_heading");
        assert!(history.take_last().is_none());
    }

    #[test]
    fn an_edit_that_changed_nothing_is_not_worth_undoing() {
        // Otherwise `undo` reports success and does nothing, which reads as a
        // bug in the server rather than as an operation that was redundant.
        let mut history = History::default();
        history.record(Entry {
            tool: "set_heading",
            deck: PathBuf::from("/deck/slides"),
            separator: "---".into(),
            files: Vec::new(),
            inverse: Edit::default(),
        });

        assert!(history.is_empty());
    }

    #[test]
    fn an_entry_remembers_which_deck_it_belongs_to() {
        // A session may hold several decks open, and an inverse measured against
        // one of them means nothing against another.
        let mut history = History::default();
        history.record(entry("set_body", "x"));

        assert_eq!(history.entries()[0].deck, PathBuf::from("/deck/slides"));
        assert_eq!(history.entries()[0].files, [PathBuf::from("/deck/slides/0001.md")]);
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn an_inverse_is_stored_as_the_value_it_is_rather_than_recomputed() {
        // The point of the whole design: an edit is data, so an undo stack is a
        // list. Recomputing one would need the source it was measured against,
        // which is exactly what has changed.
        let mut history = History::default();
        history.record(entry("set_heading", "One"));

        let stored = history.take_last().expect("an entry").inverse;
        assert_eq!(stored.splices()[0].span, ByteSpan::new(0, 1));
        assert_eq!(stored.splices()[0].text, "One");
    }
}
