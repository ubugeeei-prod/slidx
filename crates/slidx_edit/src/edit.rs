//! What an operation compiles to: byte ranges, and what replaces them.
//!
//! This is the whole reason the crate is shaped the way it is. An [`Edit`] is
//! a list of replacements into the source the author wrote, so the bytes it
//! does not name are not merely *restored* after a round trip — they are never
//! read, never rewritten, and cannot change. That is a stronger guarantee than
//! any serialiser can offer, and it is the one that makes a generated diff
//! reviewable.
//!
//! Splices are disjoint and in source order, so applying them is one pass and
//! [`invert`](Edit::invert) is exact.

use slidx_core::ByteSpan;

/// One byte range, and the text that takes its place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Splice {
    pub span: ByteSpan,
    pub text: String,
}

/// Everything one operation changes.
///
/// Usually one splice. Setting notes can be several, because a slide is
/// allowed more than one notes comment and rewriting the prose between them
/// would be a change nobody asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Edit {
    splices: Vec<Splice>,
}

impl Edit {
    /// The splices, in source order.
    pub fn splices(&self) -> &[Splice] {
        &self.splices
    }

    /// True when the operation asks for something the source already says.
    ///
    /// This is where idempotence lives: a planner does not check whether its
    /// work is redundant, it just describes the result, and a replacement that
    /// matches the text it would replace is dropped before it ever becomes an
    /// edit.
    pub fn is_empty(&self) -> bool {
        self.splices.is_empty()
    }

    /// The source with every splice applied.
    pub fn apply(&self, source: &str) -> String {
        let mut out = String::with_capacity(source.len());
        let mut cursor = 0usize;

        for splice in &self.splices {
            out.push_str(source.get(cursor..splice.span.start).unwrap_or(""));
            out.push_str(&splice.text);
            cursor = splice.span.end.max(cursor);
        }

        out.push_str(source.get(cursor..).unwrap_or(""));
        out
    }

    /// The edit that turns `apply(source)` back into `source`.
    ///
    /// Undo is out of scope for this crate, but it is the reason operations
    /// are data: an editor that keeps the inverse alongside the edit has an
    /// undo stack without a second model of the document.
    pub fn invert(&self, source: &str) -> Self {
        let mut splices = Vec::with_capacity(self.splices.len());
        let mut drift = 0isize;

        for splice in &self.splices {
            let start = (splice.span.start as isize + drift) as usize;
            splices.push(Splice {
                span: ByteSpan::new(start, start + splice.text.len()),
                text: splice.span.slice(source).to_string(),
            });
            drift += splice.text.len() as isize - splice.span.len() as isize;
        }

        Self { splices }
    }
}

/// Collects splices against the source they are measured in.
///
/// Holding the source is what lets the builder drop a replacement that changes
/// nothing, which is how every operation gets to be idempotent without each
/// planner remembering to check.
#[derive(Debug)]
pub(crate) struct EditBuilder<'a> {
    source: &'a str,
    splices: Vec<Splice>,
}

impl<'a> EditBuilder<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self { source, splices: Vec::new() }
    }

    pub(crate) fn replace(&mut self, span: ByteSpan, text: impl Into<String>) {
        let text = text.into();
        if span.slice(self.source) == text {
            return;
        }

        self.splices.push(Splice { span, text });
    }

    pub(crate) fn insert(&mut self, at: usize, text: impl Into<String>) {
        self.replace(ByteSpan::empty(at), text);
    }

    pub(crate) fn delete(&mut self, span: ByteSpan) {
        self.replace(span, "");
    }

    pub(crate) fn build(mut self) -> Edit {
        self.splices.sort_by_key(|splice| splice.span.start);
        debug_assert!(
            self.splices.windows(2).all(|pair| pair[0].span.end <= pair[1].span.start),
            "splices within one operation must not overlap: {:?}",
            self.splices
        );

        Edit { splices: self.splices }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_replaces_only_the_bytes_it_names() {
        let source = "one two three";
        let mut builder = EditBuilder::new(source);
        builder.replace(ByteSpan::new(4, 7), "TWO");

        assert_eq!(builder.build().apply(source), "one TWO three");
    }

    #[test]
    fn several_splices_apply_left_to_right_without_shifting_each_other() {
        let source = "a b c";
        let mut builder = EditBuilder::new(source);
        builder.replace(ByteSpan::new(4, 5), "ccc");
        builder.replace(ByteSpan::new(0, 1), "aaa");

        assert_eq!(builder.build().apply(source), "aaa b ccc");
    }

    #[test]
    fn a_replacement_that_changes_nothing_is_not_an_edit_at_all() {
        // Not "an edit that produces the same bytes" — no edit. The difference
        // shows up as a file whose modification time never moves.
        let source = "# Title";
        let mut builder = EditBuilder::new(source);
        builder.replace(ByteSpan::new(2, 7), "Title");

        assert!(builder.build().is_empty());
    }

    #[test]
    fn inverting_an_edit_restores_the_source_byte_for_byte() {
        let source = "---\ntitle: T\n---\n\n# One\n\n- a\n";
        let mut builder = EditBuilder::new(source);
        builder.replace(ByteSpan::new(4, 12), "title: Longer");
        builder.replace(ByteSpan::new(20, 23), "Two");
        let edit = builder.build();

        let changed = edit.apply(source);
        assert_ne!(changed, source);
        assert_eq!(edit.invert(source).apply(&changed), source);
    }

    #[test]
    fn inverting_an_insertion_deletes_exactly_what_was_inserted() {
        let source = "# One";
        let mut builder = EditBuilder::new(source);
        builder.insert(5, "\n\n---\n\n# Two");
        let edit = builder.build();

        assert_eq!(edit.invert(source).apply(&edit.apply(source)), source);
    }
}
