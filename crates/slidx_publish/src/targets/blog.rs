//! The write-up, started from what was already said.
//!
//! A speaker has already written the prose version of the talk: it is in the
//! speaker notes, one paragraph per slide, in order. The reason the blog post
//! usually never gets written is not that it is hard, it is that it starts from
//! an empty file at the end of a long day.
//!
//! So this is a scaffold and says so. Slide titles become section headings, and
//! notes become the body under them — a draft with the author's own sentences
//! in the author's own order, which is a thing to edit rather than a thing to
//! start. Nothing is rewritten, summarised, or generated: every word in the
//! output is a word the author already wrote.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::targets::yaml_string;
use crate::text::{file_slug, tidy_block};
use crate::types::{reason, BlockedReason, Composed, DeckSource};

/// One slide's worth of draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BlogSection {
    pub heading: String,
    /// The slide's notes, joined. Never edited.
    pub body: String,
    /// Slide the section came from, so an editor can jump back.
    pub slide: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BlogScaffold {
    /// Suggested file name, dated so drafts sort by talk.
    pub path: String,
    pub title: String,
    pub sections: Vec<BlogSection>,
    /// The whole file, frontmatter included.
    pub markdown: String,
}

pub fn compose_blog(source: &DeckSource) -> Composed<BlogScaffold> {
    let mut reasons: Vec<BlockedReason> = Vec::new();
    let title = source.meta.title.as_deref().unwrap_or_default().trim();

    if title.is_empty() {
        reasons
            .push(reason("title", "a draft needs a title — add `title:` to the deck frontmatter"));
    }

    let sections = sections_of(source);

    // A scaffold of empty headings is worse than no scaffold: it looks like
    // work that has been done. Say the notes are missing instead.
    if sections.is_empty() {
        reasons.push(reason(
            "notes",
            "the deck has no speaker notes — a draft is assembled from them, so there is \
             nothing to assemble",
        ));
    }

    if !reasons.is_empty() {
        return Composed::Blocked(reasons);
    }

    Composed::Ready(BlogScaffold {
        path: path_for(source, title),
        markdown: render(source, title, &sections),
        title: title.to_string(),
        sections,
    })
}

/// One section per slide that has notes.
///
/// Slides without notes are skipped rather than emitted as bare headings. A
/// title slide, a section divider, and a slide that is one image all belong to
/// the talk and none of them belongs to the write-up.
fn sections_of(source: &DeckSource) -> Vec<BlogSection> {
    let mut sections = Vec::new();

    for slide in source.ordered_slides() {
        let body = tidy_block(&slide.notes.clone().unwrap_or_default().join("\n\n"));
        if body.is_empty() {
            continue;
        }

        let heading = slide.title.as_deref().unwrap_or_default().trim();

        sections.push(BlogSection {
            // A slide with no heading still has a place in the draft, and
            // "Slide 4" is a placeholder the author will replace — which is
            // what a scaffold is.
            heading: if heading.is_empty() {
                format!("Slide {}", slide.index + 1)
            } else {
                heading.to_string()
            },
            body,
            slide: slide.index,
        });
    }

    sections
}

fn render(source: &DeckSource, title: &str, sections: &[BlogSection]) -> String {
    let meta = &source.meta;

    // A fixed order, and only the keys the deck has a value for. A frontmatter
    // block full of empty strings is something the author has to delete before
    // the draft is publishable.
    let mut front: Vec<String> = [
        ("title", Some(title.to_string())),
        ("date", meta.date.clone()),
        ("event", meta.event.clone()),
        ("slides", meta.url.clone()),
    ]
    .into_iter()
    .filter_map(|(key, value)| {
        let value = value.unwrap_or_default().trim().to_string();
        (!value.is_empty()).then(|| format!("{key}: {}", yaml_string(&value)))
    })
    .collect();

    if let Some(tags) = meta.tags.as_ref().filter(|tags| !tags.is_empty()) {
        let written: Vec<String> = tags.iter().map(|tag| yaml_string(tag)).collect();
        front.push(format!("tags: [{}]", written.join(", ")));
    }

    let mut blocks = vec![format!("---\n{}\n---", front.join("\n"))];

    // The deck's description is already a one-paragraph summary of the talk,
    // which is exactly what the top of the post needs.
    let description = meta.description.as_deref().unwrap_or_default().trim();
    if !description.is_empty() {
        blocks.push(description.to_string());
    }

    for section in sections {
        blocks.push(format!("## {}", section.heading));
        blocks.push(section.body.clone());
    }

    format!("{}\n", blocks.join("\n\n"))
}

/// A file name for the draft, on the author's own disk.
///
/// Unicode is kept: this is a local file, not a URL on someone else's site, and
/// a deck written in Japanese should not become `deck-2.md`. The date leads so
/// a directory of drafts sorts by talk.
fn path_for(source: &DeckSource, title: &str) -> String {
    let slug = [file_slug(title), file_slug(source.meta.event.as_deref().unwrap_or_default())]
        .into_iter()
        .find(|slug| !slug.is_empty())
        .unwrap_or_else(|| "deck".to_string());

    let date = source.meta.date.as_deref().unwrap_or_default().trim();

    if date.is_empty() {
        format!("{slug}.md")
    } else {
        format!("{date}-{slug}.md")
    }
}

/// One line for a printed plan.
pub fn describe_blog(scaffold: &BlogScaffold) -> String {
    format!("write {} from {} slide(s) of notes", scaffold.path, scaffold.sections.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DeckMetadata, DeckSlide};

    fn meta() -> DeckMetadata {
        DeckMetadata {
            title: Some("Zero-JavaScript Slides".into()),
            description: Some(
                "Why a deck should be plain HTML, and what it costs to keep it that way.".into(),
            ),
            event: Some("SlidxConf 2026".into()),
            date: Some("2026-07-29".into()),
            url: Some("https://slidx.dev/talks/zero-js".into()),
            tags: Some(vec!["rust".into(), "slides".into()]),
            ..DeckMetadata::default()
        }
    }

    fn notes() -> Vec<DeckSlide> {
        vec![
            DeckSlide {
                index: 0,
                title: Some("Why plain HTML".into()),
                notes: Some(vec!["A deck is a document.".into()]),
                ..DeckSlide::default()
            },
            DeckSlide {
                index: 1,
                notes: Some(vec!["Nothing to hydrate.".into()]),
                ..DeckSlide::default()
            },
        ]
    }

    fn deck(meta: DeckMetadata, slides: Vec<DeckSlide>) -> DeckSource {
        DeckSource { meta, slides, ..DeckSource::default() }
    }

    fn scaffold(source: &DeckSource) -> BlogScaffold {
        compose_blog(source).value().cloned().expect("a scaffold")
    }

    fn fields(composed: &Composed<BlogScaffold>) -> Vec<&str> {
        composed.reasons().iter().map(|reason| reason.field.as_str()).collect()
    }

    #[test]
    fn the_draft_is_frontmatter_the_decks_summary_and_one_section_per_slide() {
        assert_eq!(
            scaffold(&deck(meta(), notes())).markdown,
            concat!(
                "---\n",
                "title: \"Zero-JavaScript Slides\"\n",
                "date: \"2026-07-29\"\n",
                "event: \"SlidxConf 2026\"\n",
                "slides: \"https://slidx.dev/talks/zero-js\"\n",
                "tags: [\"rust\", \"slides\"]\n",
                "---\n",
                "\n",
                "Why a deck should be plain HTML, and what it costs to keep it that way.\n",
                "\n",
                "## Why plain HTML\n",
                "\n",
                "A deck is a document.\n",
                "\n",
                "## Slide 2\n",
                "\n",
                "Nothing to hydrate.\n",
            )
        );
    }

    #[test]
    fn an_untitled_slide_is_named_by_its_position_as_a_placeholder_to_replace() {
        let sections = scaffold(&deck(meta(), notes())).sections;

        assert_eq!(sections[0].heading, "Why plain HTML");
        assert_eq!(sections[1].heading, "Slide 2");
        assert_eq!(sections.iter().map(|s| s.slide).collect::<Vec<_>>(), [0, 1]);
    }

    #[test]
    fn a_slide_with_no_notes_is_skipped_rather_than_emitted_as_an_empty_heading() {
        // A title slide and a section divider belong to the talk, not to the
        // write-up.
        let mut slides =
            vec![DeckSlide { index: 0, title: Some("Title slide".into()), ..DeckSlide::default() }];
        slides
            .extend(notes().into_iter().map(|slide| DeckSlide { index: slide.index + 1, ..slide }));

        assert_eq!(scaffold(&deck(meta(), slides)).sections.len(), 2);
    }

    #[test]
    fn several_notes_on_one_slide_become_paragraphs() {
        let slides = vec![DeckSlide {
            index: 0,
            title: Some("Why".into()),
            notes: Some(vec!["First point.".into(), "Second point.".into()]),
            ..DeckSlide::default()
        }];

        assert_eq!(
            scaffold(&deck(meta(), slides)).sections[0].body,
            "First point.\n\nSecond point."
        );
    }

    #[test]
    fn the_draft_follows_slide_order_however_the_slides_arrive() {
        let reversed: Vec<DeckSlide> = notes().into_iter().rev().collect();
        let sections = scaffold(&deck(meta(), reversed)).sections;

        assert_eq!(sections.iter().map(|s| s.slide).collect::<Vec<_>>(), [0, 1]);
    }

    #[test]
    fn nothing_the_author_did_not_write_appears_in_a_section() {
        assert_eq!(scaffold(&deck(meta(), notes())).sections[0].body, "A deck is a document.");
    }

    #[test]
    fn a_frontmatter_key_the_deck_has_no_value_for_is_left_out() {
        let bare = DeckMetadata { event: None, url: None, tags: None, ..meta() };
        let markdown = scaffold(&deck(bare, notes())).markdown;

        assert!(!markdown.contains("event:"), "{markdown}");
        assert!(!markdown.contains("slides:"), "{markdown}");
        assert!(!markdown.contains("tags:"), "{markdown}");
    }

    #[test]
    fn the_file_name_leads_with_the_date_so_a_directory_of_drafts_sorts_by_talk() {
        assert_eq!(scaffold(&deck(meta(), notes())).path, "2026-07-29-zero-javascript-slides.md");
    }

    #[test]
    fn a_japanese_title_keeps_its_own_file_name_because_this_is_a_file_not_a_url() {
        let source =
            deck(DeckMetadata { title: Some("日本語のスライド".into()), ..meta() }, notes());

        assert_eq!(scaffold(&source).path, "2026-07-29-日本語のスライド.md");
    }

    #[test]
    fn a_deck_with_no_date_drops_the_prefix_rather_than_inventing_one() {
        let source = deck(DeckMetadata { date: None, ..meta() }, notes());

        assert_eq!(scaffold(&source).path, "zero-javascript-slides.md");
    }

    #[test]
    fn a_deck_whose_slides_carry_no_notes_is_reported_rather_than_scaffolded() {
        let source = deck(
            meta(),
            vec![DeckSlide { index: 0, title: Some("Why".into()), ..DeckSlide::default() }],
        );

        assert_eq!(fields(&compose_blog(&source)), ["notes"]);
        assert_eq!(fields(&compose_blog(&deck(meta(), Vec::new()))), ["notes"]);
    }

    #[test]
    fn a_missing_title_and_missing_notes_are_reported_at_once() {
        let source = deck(DeckMetadata { title: None, ..meta() }, Vec::new());

        assert_eq!(fields(&compose_blog(&source)), ["title", "notes"]);
    }
}
