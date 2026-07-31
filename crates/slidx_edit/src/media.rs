//! Inserting dropped image and video files as ordinary Markdown blocks.
//!
//! The browser sends semantic data, never Markdown. This module owns the image
//! syntax, the video HTML, escaping, block spacing, and optional region
//! placement so a drop remains one minimal edit that round-trips through the
//! same writer as every other visual operation.

use serde::{Deserialize, Serialize};
use slidx_core::{find_blocks, parse_deck, ByteSpan, DeckParseOptions, FoundBlock};

use crate::edit::EditBuilder;
use crate::op::{EditError, SlideRef};
use crate::source::DeckSource;

/// The two file kinds the visual editor can drop onto a slide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaKind {
    Image,
    Video,
}

pub(crate) struct Media<'a> {
    pub kind: MediaKind,
    pub src: &'a str,
    pub alt: &'a str,
}

pub(crate) fn insert(
    deck: &DeckSource<'_>,
    options: &DeckParseOptions,
    slide: &SlideRef,
    at: usize,
    media: Media<'_>,
    region: Option<&str>,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let body_source = body.slice(deck.source);
    let found = find_blocks(body_source);
    let markup = block(deck, options, index, media, region);

    if found.is_empty() {
        let prefix = if body_source.trim().is_empty() { String::new() } else { deck.blank() };
        builder.insert(body.end, format!("{prefix}{markup}"));
        return Ok(());
    }

    if at >= found.len() {
        builder.insert(body.start + outer(found.last().expect("the list is not empty")).end, {
            format!("{}{markup}", deck.blank())
        });
    } else {
        builder.insert(body.start + outer(&found[at]).start, format!("{markup}{}", deck.blank()));
    }

    Ok(())
}

fn block(
    deck: &DeckSource<'_>,
    options: &DeckParseOptions,
    slide: usize,
    media: Media<'_>,
    region: Option<&str>,
) -> String {
    let parsed = parse_deck(deck.source, options);
    let layout = parsed
        .slides
        .get(slide)
        .map_or_else(slidx_theme::layout::default_layout, slidx_theme::layout::of);
    let placed = region
        .filter(|name| layout.has_region(name) && *name != layout.fallback().name)
        .map(|name| format!("{{.{name}}}{}", deck.newline()))
        .unwrap_or_default();

    let rendered = match media.kind {
        MediaKind::Image => {
            format!("![{}](<{}>)", markdown_label(media.alt), markdown_destination(media.src))
        }
        MediaKind::Video => {
            let label = if media.alt.trim().is_empty() {
                String::new()
            } else {
                format!(" aria-label=\"{}\"", html_attribute(media.alt))
            };
            format!(
                "<video controls preload=\"metadata\" src=\"{}\"{label}></video>",
                html_attribute(media.src)
            )
        }
    };

    format!("{placed}{rendered}")
}

fn outer(found: &FoundBlock) -> ByteSpan {
    let start = found.attribute_line.map_or(found.block.span.start, |line| line.start);
    ByteSpan::new(start, found.block.span.end.max(start))
}

fn markdown_label(value: &str) -> String {
    value.replace('\\', "\\\\").replace('[', "\\[").replace(']', "\\]").replace(['\r', '\n'], " ")
}

fn markdown_destination(value: &str) -> String {
    value.replace('%', "%25").replace('<', "%3C").replace('>', "%3E").replace(['\r', '\n'], "")
}

fn html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{apply, EditOp};

    fn inserted(
        source: &str,
        at: usize,
        kind: MediaKind,
        src: &str,
        alt: &str,
        region: Option<&str>,
    ) -> String {
        let op = EditOp::InsertMedia {
            slide: 0.into(),
            at,
            kind,
            src: src.into(),
            alt: alt.into(),
            region: region.map(String::from),
        };
        apply(source, &DeckParseOptions::default(), &op).unwrap()
    }

    #[test]
    fn an_image_can_be_inserted_between_blocks_and_into_a_region() {
        let source = "---\nlayout: split\n---\n\n# One\n\nLast.\n";

        assert_eq!(
            inserted(
                source,
                1,
                MediaKind::Image,
                "assets/photo (1).png",
                "Q3 [chart]",
                Some("right"),
            ),
            "---\nlayout: split\n---\n\n# One\n\n{.right}\n![Q3 \\[chart\\]](<assets/photo (1).png>)\n\nLast.\n"
        );
    }

    #[test]
    fn a_video_appends_with_escaped_attributes() {
        assert_eq!(
            inserted(
                "# One\n",
                99,
                MediaKind::Video,
                "assets/a&b.mp4",
                "Demo \"take\"",
                None,
            ),
            "# One\n\n<video controls preload=\"metadata\" src=\"assets/a&amp;b.mp4\" aria-label=\"Demo &quot;take&quot;\"></video>\n"
        );
    }

    #[test]
    fn the_default_region_stays_implicit() {
        assert_eq!(
            inserted("", 0, MediaKind::Image, "assets/hero.png", "Hero", Some("body")),
            "![Hero](<assets/hero.png>)"
        );
    }

    #[test]
    fn destinations_cannot_break_out_of_their_markdown_delimiter() {
        assert_eq!(
            inserted("", 0, MediaKind::Image, "assets/a>\n<script>.png", "A\nB", None),
            "![A B](<assets/a%3E%3Cscript%3E.png>)"
        );
    }
}
