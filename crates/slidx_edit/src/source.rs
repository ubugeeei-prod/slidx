//! Where each part of a deck lives in the file.
//!
//! Every operation starts here: it names a slide, and this says which bytes
//! that slide is. The distinctions it draws all exist because `---` is
//! overloaded in Markdown, and an edit has to know which of its jobs a given
//! one is doing.
//!
//! # What a slide's bytes are
//!
//! A slide's [`content`](SlideSource::content) runs from the first byte of its
//! own frontmatter block — including the `---` that opens it, because that
//! same line is the separator ending the slide before — to the last non-blank
//! byte of its body. The blank lines between two slides belong to neither, and
//! so does the deck's own frontmatter block.
//!
//! That last exclusion is the one worth stating: the parser reads the deck's
//! frontmatter as the first slide's, so the two share a block, and an edit
//! that deleted the first slide must not take the deck's title with it.

use slidx_core::parser::{split, Segment};
use slidx_core::scanner::is_separator_of;
use slidx_core::{parse_deck, ByteSpan, DeckParseOptions};

use crate::op::{EditError, SlideRef};

/// One slide, located in the file it was read from.
#[derive(Debug, Clone)]
pub(crate) struct SlideSource {
    /// The slide's own bytes, frontmatter block included, blank edges excluded.
    pub content: ByteSpan,
    /// The Markdown body alone, blank edges excluded.
    pub body: ByteSpan,
    /// The YAML text of this slide's frontmatter, when it has a block.
    pub frontmatter: Option<ByteSpan>,
    /// True when the block is also the deck's, which is only ever the first
    /// slide's and only when the file opens with one.
    pub frontmatter_is_deck: bool,
}

impl SlideSource {
    /// True when the slide has a frontmatter block of its own — one that moves
    /// with it, rather than the deck's.
    pub(crate) fn owns_frontmatter(&self) -> bool {
        self.frontmatter.is_some() && !self.frontmatter_is_deck
    }
}

/// A deck source with every slide located in it.
#[derive(Debug)]
pub(crate) struct DeckSource<'a> {
    pub source: &'a str,
    pub separator: &'a str,
    pub slides: Vec<SlideSource>,
}

impl<'a> DeckSource<'a> {
    pub(crate) fn read(source: &'a str, options: &'a DeckParseOptions) -> Self {
        let slides = split(source, &options.separator)
            .iter()
            .map(|segment| locate(source, segment))
            .collect();

        Self { source, separator: &options.separator, slides }
    }

    pub(crate) fn count(&self) -> usize {
        self.slides.len()
    }

    /// The index a reference names.
    ///
    /// Resolving an id costs a parse, because ids are slugs allocated across
    /// the whole deck and cannot be worked out one slide at a time. Indexes
    /// cost nothing, which is why the canvas uses them.
    pub(crate) fn resolve(&self, slide: &SlideRef) -> Result<usize, EditError> {
        let found = match slide {
            SlideRef::Index(index) => (*index < self.slides.len()).then_some(*index),
            SlideRef::Id(id) => parse_deck(self.source, &DeckParseOptions::default())
                .slides
                .iter()
                .position(|parsed| parsed.id == *id),
        };

        found.ok_or_else(|| EditError::NoSuchSlide { slide: slide.clone() })
    }

    pub(crate) fn at(&self, index: usize) -> &SlideSource {
        &self.slides[index]
    }

    /// The YAML text of the deck's own frontmatter block, when the file opens
    /// with one.
    pub(crate) fn deck_frontmatter(&self) -> Option<ByteSpan> {
        self.slides.first().filter(|slide| slide.frontmatter_is_deck)?.frontmatter
    }

    /// The bytes between one slide and the next: blank lines, and usually the
    /// separator.
    ///
    /// "Usually" because a slide that carries frontmatter owns the separator
    /// as its opening delimiter, and the gap before it is only whitespace.
    /// Anything that rearranges slides has to ask rather than assume.
    pub(crate) fn gap(&self, before: usize) -> ByteSpan {
        ByteSpan::new(self.slides[before].content.end, self.slides[before + 1].content.start)
    }

    /// True when the text ending a slide and starting the next carries the
    /// separator line.
    ///
    /// A gap is blank lines and at most the one separator, never prose, so
    /// scanning it needs no sense of what a fenced block is.
    pub(crate) fn gap_separates(&self, before: usize) -> bool {
        self.gap(before)
            .slice(self.source)
            .lines()
            .any(|line| is_separator_of(line, self.separator))
    }

    /// True when a slide's own bytes *begin* with the separator line, which is
    /// what a slide with its own frontmatter looks like.
    ///
    /// The first line and no other. A talk about Markdown has slides whose
    /// bodies are full of `---` inside fences, and a slide that merely
    /// contains one has not moved its separator anywhere.
    pub(crate) fn opens_with_separator(&self, index: usize) -> bool {
        let content = self.at(index).content.slice(self.source);
        let first = content.lines().next().unwrap_or_default();

        is_separator_of(first, self.separator)
    }

    /// The line ending this file is written with.
    ///
    /// An author on Windows has a file of CRLF lines, and inserting LF ones
    /// would put a `^M` in the very diff this crate exists to keep clean.
    pub(crate) fn newline(&self) -> &'static str {
        if self.source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
    }

    /// A blank line, as this file spells one.
    pub(crate) fn blank(&self) -> String {
        self.newline().repeat(2)
    }

    /// The separator as it is written between two slides.
    pub(crate) fn separator_block(&self) -> String {
        format!("{}{}{}", self.blank(), self.separator, self.blank())
    }
}

fn locate(source: &str, segment: &Segment) -> SlideSource {
    let body = trim(source, segment.body_span);

    // A slide that owns its frontmatter owns the delimiter that opens it. The
    // deck's block is not any slide's to move, even though the first slide
    // reads its keys.
    let content = match &segment.frontmatter {
        Some(matter) if !segment.frontmatter_is_certain => {
            ByteSpan::new(matter.block.start, body.end.max(matter.block.end))
        }
        _ => body,
    };

    SlideSource {
        content,
        body,
        frontmatter: segment.frontmatter.as_ref().map(|matter| matter.span),
        frontmatter_is_deck: segment.frontmatter.is_some() && segment.frontmatter_is_certain,
    }
}

/// The span with its blank edges removed.
fn trim(source: &str, span: ByteSpan) -> ByteSpan {
    let text = span.slice(source);
    let start = span.start + (text.len() - text.trim_start().len());

    ByteSpan::new(start, start + text.trim().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(source: &str) -> (DeckSource<'_>, &'static DeckParseOptions) {
        static OPTIONS: std::sync::OnceLock<DeckParseOptions> = std::sync::OnceLock::new();
        let options = OPTIONS.get_or_init(DeckParseOptions::default);

        (DeckSource::read(source, options), options)
    }

    #[test]
    fn a_slides_bytes_stop_at_its_last_word() {
        let source = "# One\n\n---\n\n# Two\n\nBody.\n";
        let (deck, _) = read(source);

        assert_eq!(deck.at(0).content.slice(source), "# One");
        assert_eq!(deck.at(1).content.slice(source), "# Two\n\nBody.");
    }

    #[test]
    fn the_decks_frontmatter_belongs_to_no_slide() {
        // Deleting the first slide must not delete the deck's title with it.
        let source = "---\ntitle: T\n---\n\n# One\n\n---\n\n# Two\n";
        let (deck, _) = read(source);

        assert_eq!(deck.at(0).content.slice(source), "# One");
        assert!(deck.at(0).frontmatter_is_deck);
        assert_eq!(deck.at(0).frontmatter.unwrap().slice(source), "title: T");
    }

    #[test]
    fn a_slides_own_frontmatter_block_is_part_of_the_slide() {
        // The `---` opening the block is the same line that ends the slide
        // before, so the block cannot be left behind when the slide moves.
        let source = "# One\n\n---\nlayout: split\n---\n\n# Two";
        let (deck, _) = read(source);

        assert_eq!(deck.at(1).content.slice(source), "---\nlayout: split\n---\n\n# Two");
        assert!(!deck.at(1).frontmatter_is_deck);
        assert!(deck.opens_with_separator(1));
        assert!(!deck.gap_separates(0), "the separator is inside the slide, not the gap");
    }

    #[test]
    fn the_gap_between_plain_slides_carries_the_separator() {
        let source = "# One\n\n---\n\n# Two";
        let (deck, _) = read(source);

        assert_eq!(deck.gap(0).slice(source), "\n\n---\n\n");
        assert!(deck.gap_separates(0));
        assert!(!deck.opens_with_separator(1));
    }

    #[test]
    fn slides_resolve_by_index_and_by_id() {
        let source = "# Intro\n\n---\n\n# Deep Dive\n";
        let (deck, _) = read(source);

        assert_eq!(deck.resolve(&SlideRef::Index(1)), Ok(1));
        assert_eq!(deck.resolve(&"deep-dive".into()), Ok(1));
    }

    #[test]
    fn naming_a_slide_that_is_not_there_is_an_error_rather_than_a_panic() {
        let source = "# One\n";
        let (deck, _) = read(source);

        assert_eq!(deck.resolve(&99.into()), Err(EditError::NoSuchSlide { slide: 99.into() }));
        assert_eq!(
            deck.resolve(&"nope".into()),
            Err(EditError::NoSuchSlide { slide: "nope".into() })
        );
    }

    #[test]
    fn a_separator_inside_a_fence_does_not_make_a_slide_look_like_it_opens_with_one() {
        // A talk about Markdown is full of `---` in code blocks, and a slide
        // that shows one has not moved its separator anywhere.
        let source = "# Slides\n\n```md\n---\n```\n\n---\n\n# After\n";
        let (deck, _) = read(source);

        assert!(!deck.opens_with_separator(0));
        assert!(deck.gap_separates(0));
    }

    #[test]
    fn an_empty_source_still_locates_one_slide() {
        let (deck, _) = read("");

        assert_eq!(deck.count(), 1);
        assert!(deck.at(0).content.is_empty());
    }

    #[test]
    fn a_custom_separator_is_the_one_that_counts() {
        let options = DeckParseOptions { separator: "===".into(), ..DeckParseOptions::default() };
        let source = "# One\n\n---\n\n# Still One\n\n===\n\n# Two";
        let deck = DeckSource::read(source, &options);

        assert_eq!(deck.count(), 2);
        assert!(deck.gap_separates(0));
        assert_eq!(deck.separator_block(), "\n\n===\n\n");
    }
}
