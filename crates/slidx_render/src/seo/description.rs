//! Where a page's description comes from.
//!
//! A search result and a link preview both show two lines of text under a
//! title, and those two lines are the only part of a slide most people will
//! ever read. They have to come from somewhere, and the somewhere is ranked by
//! how much the author meant a stranger to read it:
//!
//! 1. **`description:` in the slide's own frontmatter** — words written *to be*
//!    a description. Nothing derived can beat that, and an author who dislikes
//!    what the other two produce has one line to write.
//! 2. **The speaker notes** — the author's own prose about this slide, in
//!    sentences, which is exactly the shape a description wants. Notes are
//!    already publishable text in slidx: the blog scaffold `slidx publish`
//!    writes is built out of them.
//! 3. **The slide's first paragraph** — what the audience read off the screen.
//!
//! A heading is deliberately not on that list. "Thanks!" is a title, not a
//! description, and the title is already in `<title>` — a snippet that repeats
//! it says the same thing twice and tells a reader nothing new.
//!
//! And nothing here is built out of markup. A description assembled from
//! `**bold**`, a table row or an image's alt text is not a sentence, and the
//! first thing anyone would learn about the page is that its author ships
//! syntax. So a paragraph is reduced to its words, and a block that is not
//! prose is skipped rather than flattened.
//!
//! There is no character cap. Every platform that shows a description clips it
//! to its own width, `slidx_publish` already owns the one cap in this
//! repository that a platform actually enforces, and a second one here would be
//! a second answer to the same question. What is bounded is the *unit*: one
//! paragraph, because a paragraph is one thought.

use slidx_core::mark::strip_marks;
use slidx_core::scanner::{heading_text, list_item_indent, FenceTracker};
use slidx_core::{frontmatter, Slide};

/// The description for one slide's page, or nothing.
///
/// Nothing is a real answer. A slide holding one word and an image has no
/// sentence to offer, and borrowing the deck's description would produce a
/// result whose snippet describes something other than what is behind the
/// link. The deck's own description is already on the deck's own page, which is
/// slide one.
pub fn describe(slide: &Slide) -> Option<String> {
    declared(slide).or_else(|| from_notes(slide)).or_else(|| from_prose(slide))
}

/// What the author wrote in `description:`.
///
/// Slide one's frontmatter block is the deck's, so a deck-level `description:`
/// describes the deck's front page and nothing else — which is what it is.
fn declared(slide: &Slide) -> Option<String> {
    let written = frontmatter::string(&slide.frontmatter, "description")?;
    non_empty(collapse(&written))
}

fn from_notes(slide: &Slide) -> Option<String> {
    slide.notes.iter().find_map(|note| non_empty(plain(first_paragraph(note))))
}

/// The first paragraph the audience could read on the slide.
///
/// Walks blocks rather than lines so that a paragraph split over three
/// hand-wrapped source lines arrives as one sentence. Fenced code is skipped
/// through [`FenceTracker`], which is the one place in slidx that knows where a
/// fence ends — a second implementation of that is how a `---` inside a diff
/// splits a slide in half.
fn from_prose(slide: &Slide) -> Option<String> {
    let mut fences = FenceTracker::new();
    let mut block: Vec<&str> = Vec::new();

    for line in slide.content.lines() {
        let outside_code = fences.is_prose(line);

        // A blank line ends a paragraph, and so does a fence: CommonMark lets a
        // fenced block interrupt one, so the paragraph above it is finished.
        if !outside_code || line.trim().is_empty() {
            if let Some(found) = prose_of(&block) {
                return Some(found);
            }
            block.clear();
            continue;
        }

        block.push(line);
    }

    prose_of(&block)
}

/// One block as a sentence, or nothing when the block is not prose.
fn prose_of(block: &[&str]) -> Option<String> {
    let first = block.first()?;

    // Four spaces is an indented code block, and everything else here is a
    // construct whose text reads as a fragment rather than a sentence: a
    // heading, a bullet, a table row, a quotation of someone else's words, or
    // raw HTML — which includes the step anchors the compiler plants.
    let indented = first.len() - first.trim_start_matches(' ').len() >= 4;
    let structural = heading_text(first).is_some()
        || list_item_indent(first).is_some()
        || first.trim_start().starts_with(['|', '>', '<']);

    if indented || structural {
        return None;
    }

    non_empty(plain(&block.join(" ")))
}

/// Up to the first blank line.
fn first_paragraph(text: &str) -> &str {
    let mut end = 0usize;

    for line in text.lines() {
        if line.trim().is_empty() && end > 0 {
            break;
        }
        end = text.len().min(
            // The line's own end, found by where it starts in the source.
            offset_of(text, line) + line.len(),
        );
    }

    text[..end].trim()
}

/// Where `line` starts inside `text`, which `str::lines` does not report.
fn offset_of(text: &str, line: &str) -> usize {
    // Both come from the same allocation, so pointer arithmetic is the offset.
    line.as_ptr() as usize - text.as_ptr() as usize
}

/// A block of Markdown reduced to the words in it.
fn plain(text: &str) -> String {
    let text = strip_marks(text);
    let text = without_comments(&text);
    let text = without_tags(&text);
    let text = without_links(&text);

    collapse(&without_emphasis(&text))
}

fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find("<!--") {
        out.push_str(&rest[..at]);
        rest = match rest[at..].find("-->") {
            Some(end) => &rest[at + end + 3..],
            // An unclosed comment swallows the rest of the block, exactly as a
            // browser would treat it.
            None => "",
        };
    }

    out.push_str(rest);
    out
}

/// Drops HTML elements, keeping the text between them.
///
/// An `<img>` therefore contributes nothing, which is the point: alt text
/// describes a picture to someone who cannot see it, not the page to someone
/// deciding whether to open it.
fn without_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        rest = match rest[at..].find('>') {
            Some(end) => &rest[at + end + 1..],
            None => "",
        };
    }

    out.push_str(rest);
    out
}

/// Turns `[text](url)` into `text` and drops `![alt](url)` entirely.
fn without_links(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find('[') {
        let image = rest[..at].ends_with('!');
        out.push_str(&rest[..at - usize::from(image)]);

        let Some(close) = rest[at..].find(']') else { break };
        let label = &rest[at + 1..at + close];
        rest = &rest[at + close + 1..];

        // The destination, when there is one. `[text]` on its own is a
        // shortcut reference link and has no brackets to drop.
        if let Some(stripped) = destination(rest) {
            rest = stripped;
        }

        if !image {
            out.push_str(label);
        }
    }

    out.push_str(rest);
    out
}

/// Skips the `(url)` or `[id]` that follows a link label.
fn destination(rest: &str) -> Option<&str> {
    for (open, close) in [('(', ')'), ('[', ']')] {
        if let Some(tail) = rest.strip_prefix(open) {
            return tail.find(close).map(|end| &tail[end + 1..]);
        }
    }

    None
}

/// Removes emphasis, strong, strikethrough and code markers.
///
/// `_` is only dropped where Markdown would read it as emphasis — at a word
/// boundary. Removing every one of them would quietly rewrite `retry_after` as
/// `retryafter`, which is worse than an underscore reaching a search result.
fn without_emphasis(text: &str) -> String {
    let characters: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());

    for (at, character) in characters.iter().enumerate() {
        let flanked = |offset: isize| {
            usize::try_from(at as isize + offset)
                .ok()
                .and_then(|index| characters.get(index))
                .is_some_and(|neighbour| neighbour.is_alphanumeric())
        };

        let drop = match character {
            '*' | '~' | '`' => true,
            '_' => !(flanked(-1) && flanked(1)),
            _ => false,
        };

        if !drop {
            out.push(*character);
        }
    }

    out
}

/// One line, single-spaced.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn non_empty(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn described(source: &str) -> Option<String> {
        let deck = parse_deck(source, &DeckParseOptions::default());
        describe(&deck.slides[0])
    }

    #[test]
    fn what_the_author_wrote_wins_over_anything_derived() {
        let source = "---\ndescription: Why decks should be pages.\n---\n\n# One\n\nSomething else.\n\n<!-- notes: and a note -->\n";

        assert_eq!(described(source).as_deref(), Some("Why decks should be pages."));
    }

    #[test]
    fn the_speaker_notes_are_preferred_to_the_words_on_the_slide() {
        // The notes are prose about the slide; the slide is often three words
        // and a picture.
        let source = "# Results\n\n3.2x\n\n<!-- notes:\nThe rewrite paid for itself in a fortnight.\n-->\n";

        assert_eq!(
            described(source).as_deref(),
            Some("The rewrite paid for itself in a fortnight.")
        );
    }

    #[test]
    fn only_the_first_paragraph_of_a_long_note_becomes_the_description() {
        let source = "# One\n\n<!-- notes:\nThe opening claim.\n\nAnd a second thought nobody asked for.\n-->\n";

        assert_eq!(described(source).as_deref(), Some("The opening claim."));
    }

    #[test]
    fn a_slide_with_no_notes_falls_back_to_its_own_first_paragraph() {
        let source = "# Making Decks Fast\n\nA framework for the whole life of a talk.\n";

        assert_eq!(
            described(source).as_deref(),
            Some("A framework for the whole life of a talk.")
        );
    }

    #[test]
    fn a_hand_wrapped_paragraph_arrives_as_one_sentence() {
        // Authors wrap their prose. A description broken at the author's column
        // width would carry a newline into an attribute.
        let source = "# One\n\nThe parser is fast\nbecause it does not\nallocate.\n";

        assert_eq!(
            described(source).as_deref(),
            Some("The parser is fast because it does not allocate.")
        );
    }

    #[test]
    fn a_heading_is_never_the_description() {
        // It is already the title, and a snippet that repeats the title tells a
        // reader nothing they did not have.
        assert_eq!(described("# Thanks!\n"), None);
    }

    #[test]
    fn a_bullet_list_is_skipped_in_favour_of_the_prose_after_it() {
        let source = "# Agenda\n\n- one\n- two\n\nWe will start with the parser.\n";

        assert_eq!(described(source).as_deref(), Some("We will start with the parser."));
    }

    #[test]
    fn a_table_a_quote_and_raw_html_are_not_prose() {
        for block in ["| a | b |\n| - | - |", "> Someone else said this.", "<div>markup</div>"] {
            assert_eq!(described(&format!("# One\n\n{block}\n")), None, "{block}");
        }
    }

    #[test]
    fn code_is_never_read_as_prose_however_much_of_it_looks_like_english() {
        // A fence full of comments is still code, and the tracker is the one
        // place that knows where the fence ends.
        let source = "# One\n\n```rust\n// this reads like a sentence\nlet x = 1;\n```\n\nThe real prose.\n";

        assert_eq!(described(source).as_deref(), Some("The real prose."));
    }

    #[test]
    fn a_paragraph_directly_above_a_fence_is_still_a_paragraph() {
        let source = "# One\n\nHere is the retry policy.\n```rust\nfn retry() {}\n```\n";

        assert_eq!(described(source).as_deref(), Some("Here is the retry policy."));
    }

    #[test]
    fn a_description_carries_words_rather_than_markup() {
        let source = "# One\n\nThe result was **3.2x faster** than the `previous` build.\n";

        assert_eq!(
            described(source).as_deref(),
            Some("The result was 3.2x faster than the previous build.")
        );
    }

    #[test]
    fn a_link_contributes_its_words_and_not_its_url() {
        let source = "# One\n\nSee [the benchmark](https://example.com/bench) for the numbers.\n";

        assert_eq!(described(source).as_deref(), Some("See the benchmark for the numbers."));
    }

    #[test]
    fn an_image_contributes_nothing_at_all() {
        // Alt text describes a picture to someone who cannot see it. It is not
        // a description of the page to someone deciding whether to open it.
        let source = "# One\n\n![a chart of build times](./chart.png) and the point it makes.\n";

        assert_eq!(described(source).as_deref(), Some("and the point it makes."));
    }

    #[test]
    fn a_mark_contributes_its_text_without_its_attributes() {
        let source = "# One\n\nLatency dropped to [38ms]{#latency .accent} at last.\n";

        assert_eq!(described(source).as_deref(), Some("Latency dropped to 38ms at last."));
    }

    #[test]
    fn an_identifier_with_an_underscore_in_it_survives() {
        // Stripping every `_` would rewrite the author's words rather than
        // their markup.
        let source = "# One\n\nThe retry_after header decides.\n";

        assert_eq!(described(source).as_deref(), Some("The retry_after header decides."));
    }

    #[test]
    fn emphasis_written_with_underscores_still_loses_its_markers() {
        let source = "# One\n\nThis is _really_ the point.\n";

        assert_eq!(described(source).as_deref(), Some("This is really the point."));
    }

    #[test]
    fn a_step_anchor_does_not_reach_the_description() {
        // Markers are compiled into the content before anything reads it, so
        // the anchors are in the text this walks.
        let source = "One point <!-- step -->\n";

        assert_eq!(described(source).as_deref(), Some("One point"));
    }

    #[test]
    fn a_slide_with_nothing_to_say_describes_nothing() {
        assert_eq!(described("# One\n\n![a photo](./photo.png)\n"), None);
        assert_eq!(described("---\nlayout: split\n---\n\n"), None);
    }
}
