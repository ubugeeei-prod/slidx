//! Walking a deck source into segments.
//!
//! Two passes over the same segmentation, deliberately. [`slidx_core::split`]
//! gives the byte ranges, which is what a splice needs;
//! [`slidx_core::parse_deck`] gives the slide ids, which is what a catalogue
//! address needs and which cannot be worked out one slide at a time because ids
//! are allocated across the whole deck.
//!
//! Frontmatter is an allow-list of two keys and that is the important part of
//! this module. Everything else up there is vocabulary (`theme`, `layout`,
//! `transition`, `aspect`, `autoSteps`), arithmetic (`budget`, `duration`), an
//! address (`url`, `repo`, `demo`), a proper noun (`author`, `event`, `venue`),
//! or a key belonging to a theme this version of slidx has never heard of. A
//! deny-list would translate the next one somebody adds.

mod block;

use slidx_core::notes::find_notes;
use slidx_core::parser::split;
use slidx_core::scanner::FenceTracker;
use slidx_core::{parse_deck, ByteSpan, DeckParseOptions};

use crate::protect::mask;
use crate::segment::{Segment, SegmentKind};

/// Deck-level frontmatter values that hold prose.
pub(crate) const TRANSLATABLE_KEYS: [&str; 2] = ["title", "description"];

/// Every translatable segment of a deck, in source order.
pub(crate) fn segments(source: &str, options: &DeckParseOptions) -> Vec<Segment> {
    let raw = split(source, &options.separator);
    let deck = parse_deck(source, options);
    let mut found = Vec::new();

    if let Some(matter) = raw.first().filter(|first| first.frontmatter_is_certain) {
        let block = matter.frontmatter.as_ref().expect("certain frontmatter has a block");
        found.extend(meta_segments(source, block.span));
    }

    for (index, segment) in raw.iter().enumerate() {
        let slide = match deck.slides.get(index) {
            Some(slide) => slide.id.clone(),
            None => continue,
        };

        found.extend(slide_segments(source, segment.body_span, &slide, segment.line));
    }

    found
}

/// The deck's own prose: its title and its description, and nothing else.
fn meta_segments(source: &str, block: ByteSpan) -> Vec<Segment> {
    let text = block.slice(source);

    TRANSLATABLE_KEYS
        .iter()
        .filter_map(|key| {
            let entry = slidx_edit::frontmatter::entry(text, key)?;
            let span = entry.value.shifted(block.start);
            let scalar = plain_scalar(span.slice(source))?;

            let masked = mask(&scalar);
            masked.has_words().then(|| Segment {
                context: format!("deck/{key}"),
                kind: SegmentKind::Meta((*key).to_string()),
                span,
                text: masked.text,
                protected: masked.protected,
                slide: "deck".to_string(),
                line: line_of(source, span.start),
            })
        })
        .collect()
}

/// One slide's headings, prose, and notes.
fn slide_segments(source: &str, body: ByteSpan, slide: &str, first_line: u32) -> Vec<Segment> {
    let text = body.slice(source);
    let notes: Vec<_> =
        find_notes(text).into_iter().filter(|note| !inside_a_fence(text, note.span.start)).collect();
    let note_spans: Vec<ByteSpan> = notes.iter().map(|note| note.span).collect();

    let mut found = Vec::new();
    let mut bodies = 0u32;

    for found_block in block::blocks(text, &note_spans) {
        let span = found_block.span.shifted(body.start);
        let masked = mask(span.slice(source));
        if !masked.has_words() {
            continue;
        }

        let (kind, context) = if found_block.is_heading {
            (SegmentKind::Heading, format!("{slide}/heading"))
        } else {
            bodies += 1;
            (SegmentKind::Body, format!("{slide}/body/{bodies}"))
        };

        found.push(Segment {
            context,
            kind,
            span,
            text: masked.text,
            protected: masked.protected,
            slide: slide.to_string(),
            line: first_line + found_block.line - 1,
        });
    }

    for (index, note) in notes.iter().enumerate() {
        let span = note.text_span.shifted(body.start);
        let masked = mask(span.slice(source));
        if !masked.has_words() {
            continue;
        }

        found.push(Segment {
            context: format!("{slide}/notes/{}", index + 1),
            kind: SegmentKind::Notes,
            span,
            text: masked.text,
            protected: masked.protected,
            slide: slide.to_string(),
            line: line_of(source, span.start),
        });
    }

    found.sort_by_key(|segment| segment.span.start);
    found
}

/// The YAML scalar a value span holds, or `None` when it is not one.
///
/// A block scalar, a flow collection or a value spread over several lines is not
/// something this crate rewrites: replacing it would need a YAML writer, and a
/// deck title has never been one. Skipping is the honest answer — the key simply
/// does not appear in the catalogue.
pub(crate) fn plain_scalar(value: &str) -> Option<String> {
    let trimmed = value.trim();

    if trimmed.is_empty() || trimmed.contains('\n') || trimmed.starts_with(['|', '>', '[', '{', '&', '*']) {
        return None;
    }

    match trimmed.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        Some(inner) => Some(inner.replace("\\\"", "\"").replace("\\\\", "\\")),
        None => match trimmed.strip_prefix('\'').and_then(|rest| rest.strip_suffix('\'')) {
            Some(inner) => Some(inner.replace("''", "'")),
            None => Some(trimmed.to_string()),
        },
    }
}

/// True when `offset` sits inside a fenced code block.
///
/// `find_notes` is not fence-aware, and a talk about slidx shows a notes comment
/// inside a fence on purpose. Translating that one would rewrite the sample the
/// slide is teaching from.
fn inside_a_fence(body: &str, offset: usize) -> bool {
    let mut fences = FenceTracker::new();
    let mut cursor = 0usize;

    for line in body.split_inclusive('\n') {
        let prose = fences.feed(line.trim_end_matches(['\n', '\r']));
        cursor += line.len();

        if offset < cursor {
            return !prose;
        }
    }

    false
}

/// The one-based line `offset` falls on.
fn line_of(source: &str, offset: usize) -> u32 {
    source[..offset.min(source.len())].bytes().filter(|byte| *byte == b'\n').count() as u32 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(source: &str) -> Vec<Segment> {
        segments(source, &DeckParseOptions::default())
    }

    fn contexts(source: &str) -> Vec<String> {
        extract(source).into_iter().map(|segment| segment.context).collect()
    }

    fn texts(source: &str) -> Vec<String> {
        extract(source).into_iter().map(|segment| segment.text).collect()
    }

    #[test]
    fn a_segments_span_names_the_bytes_it_was_read_from() {
        // Everything downstream is a splice into these bytes. A span that named
        // anything else would translate the wrong part of the file.
        let source = "---\ntitle: Fast Decks\n---\n\n# Fast Decks\n\nA framework.\n";

        for segment in extract(source) {
            let raw = segment.span.slice(source);
            assert!(!raw.is_empty(), "{} spans nothing", segment.context);
        }
    }

    #[test]
    fn only_the_two_frontmatter_keys_that_hold_prose_are_offered() {
        // `theme` is a name the theme layer resolves, `duration` is arithmetic,
        // and `hashtag` is something an audience searches for.
        let source = "---\ntitle: Fast Decks\ndescription: A framework.\ntheme: minimal\nduration: 20m\nhashtag: slidx\nevent: SlidxConf\n---\n\n# One\n";

        assert_eq!(contexts(source), ["deck/title", "deck/description", "one/heading"]);
    }

    #[test]
    fn a_quoted_frontmatter_value_is_offered_without_its_quotes() {
        let source = "---\ntitle: \"Fast: Decks\"\n---\n\n# One\n";

        assert_eq!(texts(source)[0], "Fast: Decks");
    }

    #[test]
    fn a_frontmatter_value_that_is_not_a_plain_scalar_is_left_alone() {
        // A block scalar would need a YAML writer to put back, and no deck title
        // has ever been one.
        let source = "---\ndescription: |\n  a block\n  scalar\n---\n\n# One\n";

        assert_eq!(contexts(source), ["one/heading"]);
    }

    #[test]
    fn a_slide_is_addressed_by_its_id_rather_than_its_number() {
        // Inserting a slide renumbers every slide after it and would orphan the
        // whole translation from that point on.
        let source = "# Intro\n\n---\n\n# Deep Dive\n\nWords.\n";

        assert_eq!(contexts(source), ["intro/heading", "deep-dive/heading", "deep-dive/body/1"]);
    }

    #[test]
    fn notes_are_extracted_with_the_slide_they_belong_to() {
        // A translated slide with untranslated notes is worse than neither.
        let source = "# One\n\n<!-- notes:\nOpen with the outcome.\n-->\n";
        let found = extract(source);

        assert_eq!(contexts(source), ["one/heading", "one/notes/1"]);
        assert_eq!(found[1].text, "Open with the outcome.");
        assert!(matches!(found[1].kind, SegmentKind::Notes));
    }

    #[test]
    fn a_mark_key_is_absent_from_every_segment_it_appears_in() {
        // The whole point. A translator cannot mistype what is not there.
        let source = "Latency dropped to [120ms]{#latency}[38ms]{#latency}.\n";

        for segment in extract(source) {
            assert!(!segment.text.contains("#latency"), "{}", segment.text);
        }
    }

    #[test]
    fn code_in_a_fence_is_never_a_segment() {
        let source = "## Snapshots\n\n```rust\nlet frame = timeline.frame(step)?;\n```\n\nAfter.\n";

        assert_eq!(texts(source), ["Snapshots", "After."]);
    }

    #[test]
    fn segments_come_back_in_source_order_so_a_catalogue_reads_like_the_deck() {
        // A translator works down a deck. A catalogue that jumped from a note
        // back up to a heading would be read out of order or not at all.
        let source = "# One\n\n<!-- notes: say this -->\n\nThen this.\n";
        let found = extract(source);

        assert!(found.windows(2).all(|pair| pair[0].span.start <= pair[1].span.start));
    }

    #[test]
    fn a_segment_reports_the_line_of_the_file_it_came_from() {
        // The `#:` reference a translator clicks. Off by a slide and it sends
        // them to the wrong place in a file they cannot read.
        let source = "---\ntitle: T\n---\n\n# One\n\n---\n\n# Two\n";
        let two = extract(source).into_iter().find(|s| s.text == "Two").expect("slide two");

        assert_eq!(two.line, 9);
    }

    #[test]
    fn a_deck_with_nothing_to_translate_yields_nothing_rather_than_an_empty_entry() {
        assert!(extract("").is_empty());
        assert!(extract("```rust\nlet x = 1;\n```\n").is_empty());
    }

    #[test]
    fn a_slide_frontmatter_block_is_not_a_place_prose_lives() {
        // `layout`, `budget` and `steps` are the whole vocabulary up there, and
        // `title:` on slide four does nothing at all.
        let source = "# One\n\n---\nlayout: statement\nbudget: 90s\n---\n\n# Two\n";

        assert_eq!(contexts(source), ["one/heading", "two/heading"]);
    }
}
