//! One translatable piece of a deck, and where it is.
//!
//! A segment is addressed by [`context`](Segment::context) rather than by its
//! byte range, and that is the whole design of the identifier. A catalogue
//! outlives the source it was extracted from: a translator works on it for a
//! week while the author fixes a typo on slide one, and every offset moves. So
//! the address is the slide's id, the kind of thing it is, and which one of
//! those it is on that slide — none of which an unrelated edit disturbs.
//!
//! Slide ids rather than slide numbers, for the same reason: inserting a slide
//! renumbers every slide after it and would orphan the whole translation from
//! that point on.

use slidx_core::ByteSpan;

/// What part of a slide a segment is.
///
/// Kept apart because they are not interchangeable on the way back in. A body
/// block is spliced where it was found; a heading additionally forces the slide
/// to pin the id it used to derive; a frontmatter value has to be re-quoted as
/// YAML or a translated title containing a colon breaks the block it lives in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentKind {
    /// A value in the deck's own frontmatter, named by its key.
    Meta(String),
    /// The slide's first heading, which is where its id comes from.
    Heading,
    /// A paragraph, list item, table row, or quote in the slide body.
    Body,
    /// One speaker-notes comment.
    Notes,
}

impl SegmentKind {
    /// The word that goes in a segment's address.
    pub fn as_token(&self) -> &str {
        match self {
            Self::Meta(key) => key,
            Self::Heading => "heading",
            Self::Body => "body",
            Self::Notes => "notes",
        }
    }
}

/// One run of prose, located in the deck it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Stable address, `slide-id/kind` or `slide-id/kind/n`.
    pub context: String,
    pub kind: SegmentKind,
    /// The bytes this text occupies, in the deck source it was read from.
    pub span: ByteSpan,
    /// The text, with every protected region replaced by `%1`, `%2`, … .
    pub text: String,
    /// What each placeholder stands for, in order.
    pub protected: Vec<String>,
    /// The id of the slide this belongs to, so a translator is told where they
    /// are and so a heading can pin it.
    pub slide: String,
    /// One-based line in the deck source, for a `#:` reference a person can
    /// jump to.
    pub line: u32,
}

impl Segment {
    /// What a translator is told about this string, beyond the string itself.
    ///
    /// Every line of it earns its place by answering a question a translator
    /// would otherwise have to guess at: which slide is this on, what kind of
    /// thing is it, and what is the markup I am being asked not to touch.
    pub fn notes(&self) -> Vec<String> {
        let mut notes = vec![match &self.kind {
            SegmentKind::Meta(key) => format!("The deck's `{key}`."),
            SegmentKind::Heading => format!("Heading of slide `{}`.", self.slide),
            SegmentKind::Body => format!("Body of slide `{}`.", self.slide),
            SegmentKind::Notes => format!("Speaker notes for slide `{}` — not shown to the audience.", self.slide),
        }];

        for (index, text) in self.protected.iter().enumerate() {
            notes.push(format!("%{} is `{}` — keep it, move it if the grammar needs to.", index + 1, text));
        }

        if matches!(self.kind, SegmentKind::Heading) {
            notes.push(
                "The slide's URL is pinned to the original heading, so translating this does not \
                 move the slide."
                    .to_string(),
            );
        }

        notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(kind: SegmentKind, protected: &[&str]) -> Segment {
        Segment {
            context: "intro/body/1".to_string(),
            kind,
            span: ByteSpan::new(0, 1),
            text: "x".to_string(),
            protected: protected.iter().map(|text| text.to_string()).collect(),
            slide: "intro".to_string(),
            line: 1,
        }
    }

    #[test]
    fn a_translator_is_told_which_slide_and_which_part_of_it() {
        let notes = segment(SegmentKind::Body, &[]).notes();
        assert!(notes[0].contains("intro"), "{notes:?}");
    }

    #[test]
    fn notes_are_marked_as_the_half_the_audience_never_sees() {
        // A translator who thinks they are translating a slide will write for a
        // slide, and speaker notes are spoken rather than read.
        assert!(segment(SegmentKind::Notes, &[]).notes()[0].contains("not shown"));
    }

    #[test]
    fn every_placeholder_is_explained_by_what_it_stands_for() {
        // `%1` on its own is a puzzle. A translator has to be able to tell a
        // mark key from a code span to place it in a sentence.
        let notes = segment(SegmentKind::Body, &["{#latency}", "`retry`"]).notes();

        assert!(notes.iter().any(|note| note.starts_with("%1 is `{#latency}`")), "{notes:?}");
        assert!(notes.iter().any(|note| note.starts_with("%2 is")), "{notes:?}");
    }

    #[test]
    fn a_heading_says_that_translating_it_does_not_move_the_slide() {
        // The single most alarming thing about translating a deck, answered
        // where the person doing it is looking.
        let notes = segment(SegmentKind::Heading, &[]).notes();
        assert!(notes.iter().any(|note| note.contains("does not move the slide")), "{notes:?}");
    }

    #[test]
    fn a_frontmatter_segment_is_named_by_its_key_rather_than_by_a_slide() {
        assert_eq!(SegmentKind::Meta("title".into()).as_token(), "title");
        assert!(segment(SegmentKind::Meta("title".into()), &[]).notes()[0].contains("`title`"));
    }
}
