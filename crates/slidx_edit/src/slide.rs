//! Operations on whole slides: their text, and their order.
//!
//! Ordering is where the splice model earns its keep. A slide is moved by
//! copying the bytes it already occupies to another position and reusing the
//! separators that were already there, so a reordered deck diffs as moved
//! lines rather than as a rewritten file.
//!
//! # The one asymmetry
//!
//! The parser reads the deck's frontmatter as the first slide's, so the two
//! share a block. A slide that carries its own block therefore cannot simply
//! become the first slide of a deck that has one — two blocks at the top of a
//! file parse as one block and an empty slide. Both [`remove`] and [`move_to`]
//! merge instead, which is what "this slide is now the first slide" means when
//! the first slide's frontmatter is the deck's.

use slidx_core::scanner::{heading_span, FenceTracker};
use slidx_core::{find_notes, ByteSpan};

use crate::edit::EditBuilder;
use crate::frontmatter;
use crate::op::{EditError, SlideRef};
use crate::source::DeckSource;

pub(crate) fn set_body(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    body: &str,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    builder.replace(deck.at(index).body, body.trim());

    Ok(())
}

pub(crate) fn set_heading(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    text: &str,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;

    match heading(deck.source, body) {
        Some(span) => builder.replace(span, text),
        // A slide can be a diagram or a quote and have no heading at all.
        // Giving it one is the operation, so it goes above whatever is there.
        None if body.is_empty() => builder.insert(body.start, format!("# {text}")),
        None => builder.insert(body.start, format!("# {text}{}", deck.blank())),
    }

    Ok(())
}

pub(crate) fn insert(
    deck: &DeckSource<'_>,
    at: usize,
    body: &str,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    if at > deck.count() {
        return Err(EditError::NoSuchPosition { at, slides: deck.count() });
    }

    if at == deck.count() {
        let end = deck.at(at - 1).content.end;
        builder.insert(end, format!("{}{}", deck.separator_block(), body.trim()));
        return Ok(());
    }

    let mut text = String::new();

    // The slide currently at `at` may own the separator that precedes it, as
    // its frontmatter's opening delimiter. Then the gap above is blank and the
    // new slide has to bring a separator of its own on both sides.
    if at > 0 && !deck.gap_separates(at - 1) {
        text.push_str(deck.separator);
        text.push_str(&deck.blank());
    }
    text.push_str(body.trim());
    if deck.opens_with_separator(at) {
        text.push_str(&deck.blank());
    } else {
        text.push_str(&deck.separator_block());
    }

    builder.insert(deck.at(at).content.start, text);
    Ok(())
}

/// Copies one slide immediately after itself, without copying a pinned address.
///
/// The slide source already excludes the deck's own frontmatter, even for the
/// opening slide, so its title and publish settings cannot leak into the copy.
/// A later slide's own frontmatter does travel — layout, timing, transitions,
/// steps and every unknown key are part of the slide — except `id:`. A pinned
/// id is a published address and two slides cannot own it; without a pin the
/// parser allocates the duplicate its own collision-safe slug.
pub(crate) fn duplicate(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let located = deck.at(index);
    let copied = without_pinned_id(deck, index);
    let before = if located.owns_frontmatter() { deck.blank() } else { deck.separator_block() };

    builder.insert(located.content.end, format!("{before}{copied}"));
    Ok(())
}

fn without_pinned_id(deck: &DeckSource<'_>, index: usize) -> String {
    let located = deck.at(index);
    let content = located.content.slice(deck.source);
    let Some(matter) = located.frontmatter.filter(|_| located.owns_frontmatter()) else {
        return content.to_string();
    };
    let Some(id) = frontmatter::entry(matter.slice(deck.source), "id") else {
        return content.to_string();
    };

    let mut start = matter.start - located.content.start + id.whole.start;
    let mut end = matter.start - located.content.start + id.whole.end;

    // Take one line ending with the key. If the entry ends the block, take the
    // one before it instead. The rest of the author's frontmatter stays byte
    // identical, including its comments and line-ending style.
    if content[end..].starts_with("\r\n") {
        end += 2;
    } else if content[end..].starts_with('\n') {
        end += 1;
    } else if content[..start].ends_with("\r\n") {
        start -= 2;
    } else if content[..start].ends_with('\n') {
        start -= 1;
    }

    format!("{}{}", &content[..start], &content[end..])
}

pub(crate) fn remove(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;

    // A deck of one slide keeps its frontmatter and loses its body. There is
    // no such thing as a deck with no slides for the result to become.
    if deck.count() == 1 {
        builder.delete(deck.at(0).content);
        return Ok(());
    }

    if index > 0 {
        // Take the separator above with it, so the slides that remain stay
        // separated by exactly what separated them before.
        builder.delete(ByteSpan::new(deck.at(index - 1).content.end, deck.at(index).content.end));
        return Ok(());
    }

    match (deck.deck_frontmatter(), deck.at(1).frontmatter) {
        // The promoted slide's keys join the deck's block: everything from the
        // deck's last key to the promoted slide's first one is the slide being
        // deleted, plus two delimiters that are now one.
        (Some(deck_matter), Some(promoted)) => {
            builder.replace(ByteSpan::new(deck_matter.end, promoted.start), deck.newline());
        }
        _ => builder.delete(ByteSpan::new(deck.at(0).content.start, deck.at(1).content.start)),
    }

    Ok(())
}

pub(crate) fn move_to(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    to: usize,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let from = deck.resolve(slide)?;
    if to >= deck.count() {
        return Err(EditError::NoSuchPosition { at: to, slides: deck.count() });
    }
    if from == to {
        return Ok(());
    }

    let (low, high) = (from.min(to), from.max(to));
    let mut order: Vec<usize> = (low..=high).collect();
    let moved = order.remove(from - low);
    order.insert(to - low, moved);

    // Only the head of the region can need promoting, and only when the region
    // starts at the top of the deck.
    let promoted = (low == 0 && deck.at(order[0]).owns_frontmatter())
        .then(|| deck.deck_frontmatter())
        .flatten();

    if let (Some(deck_matter), Some(head)) = (promoted, deck.at(order[0]).frontmatter) {
        builder.insert(deck_matter.end, format!("{}{}", deck.newline(), head.slice(deck.source)));
    }

    let mut text = String::new();
    for (position, index) in order.iter().enumerate() {
        // A promoted slide leaves its own block behind in the deck's, so it is
        // emitted as a plain body and needs a separator like any other.
        let promote = promoted.is_some() && position == 0;
        let span = if promote { deck.at(*index).body } else { deck.at(*index).content };

        if position > 0 {
            let opens = !promote && deck.opens_with_separator(*index);
            text.push_str(&reuse_gap(deck, low + position - 1, opens));
        }
        text.push_str(span.slice(deck.source));
    }

    builder.replace(ByteSpan::new(deck.at(low).content.start, deck.at(high).content.end), text);
    Ok(())
}

/// The text that goes between two slides after a move.
///
/// The gap that already sat at this position is reused whenever it still fits,
/// which is what keeps an author's extra blank line — and the whole diff — to
/// the slides that actually moved. It fits when exactly one of the gap and the
/// following slide carries the separator: two would leave an empty slide
/// between them, none would join the two slides into one.
fn reuse_gap(deck: &DeckSource<'_>, position: usize, next_opens_with_separator: bool) -> String {
    if deck.gap_separates(position) != next_opens_with_separator {
        return deck.gap(position).slice(deck.source).to_string();
    }

    if next_opens_with_separator {
        deck.blank()
    } else {
        deck.separator_block()
    }
}

/// The first ATX heading in a slide body, as a span in the whole source.
///
/// Fenced code is skipped because a talk about Markdown is full of headings
/// that are examples, and speaker notes are skipped because a heading written
/// into a note is something only the speaker sees.
fn heading(source: &str, body: ByteSpan) -> Option<ByteSpan> {
    let text = body.slice(source);
    let notes = find_notes(text);
    let mut fences = FenceTracker::new();
    let mut cursor = 0usize;

    while cursor < text.len() {
        let end = text[cursor..].find('\n').map_or(text.len(), |at| cursor + at);
        let line = text[cursor..end].trim_end_matches('\r');

        if fences.feed(line)
            && !notes.iter().any(|note| note.span.contains(ByteSpan::new(cursor, end)))
        {
            if let Some(span) = heading_span(line) {
                return Some(ByteSpan::from(span).shifted(body.start + cursor));
            }
        }

        cursor = end + 1;
    }

    None
}
