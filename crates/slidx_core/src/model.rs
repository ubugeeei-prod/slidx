//! The deck model.
//!
//! One deck is one talk. The model deliberately carries more than slides:
//! the event it is for, how long the slot is, and how long each slide is
//! budgeted. That metadata is written once, at proposal time, and is then the
//! only source for the title slide, the timer, the social card, the published
//! description, and the archive entry — so none of them can drift apart.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::diagnostic::Diagnostics;
use crate::steps::{StepSource, StepTimeline};

/// Slide geometry.
///
/// Venues are 16:9 often enough to be the default and 4:3 often enough that
/// assuming otherwise gets slides cropped on stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AspectRatio {
    #[default]
    #[serde(rename = "16:9")]
    Wide,
    #[serde(rename = "16:10")]
    Golden,
    #[serde(rename = "4:3")]
    Classic,
}

impl AspectRatio {
    /// Reference pixel size used for layout, PDF pages, and OG rendering.
    ///
    /// Fixed at 1920×1080-class dimensions because published decks are graded
    /// at that resolution and anything smaller rasterises visibly.
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Wide => (1920, 1080),
            Self::Golden => (1920, 1200),
            Self::Classic => (1440, 1080),
        }
    }

    pub fn as_token(self) -> &'static str {
        match self {
            Self::Wide => "16:9",
            Self::Golden => "16:10",
            Self::Classic => "4:3",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "16:9" | "16/9" => Some(Self::Wide),
            "16:10" | "16/10" => Some(Self::Golden),
            "4:3" | "4/3" => Some(Self::Classic),
            _ => None,
        }
    }
}

/// Where and when this talk is given.
///
/// Everything here is optional: a deck for an internal brown bag needs none of
/// it, and a conference deck gets a correct social card for free by filling it
/// in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TalkMeta {
    pub event: Option<String>,
    /// ISO-8601 date, kept as text so a deck never depends on a clock.
    pub date: Option<String>,
    pub venue: Option<String>,
    /// Hashtag without the leading `#`.
    pub hashtag: Option<String>,
    /// Canonical URL of the published deck.
    pub url: Option<String>,
    /// Repository shown on the closing slide and in the resources page.
    pub repo: Option<String>,
}

impl TalkMeta {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Deck-level configuration and metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    /// Theme package name or built-in theme id.
    pub theme: Option<String>,
    /// Default slide-to-slide transition.
    pub transition: Option<String>,
    pub aspect: AspectRatio,
    /// Length of the speaking slot, in seconds.
    ///
    /// Drives the presenter countdown and the build-time budget check that
    /// catches a 40-minute deck booked into a 20-minute slot.
    pub duration_seconds: Option<u32>,
    pub talk: TalkMeta,
    /// The frontmatter as written, so theme and plugin options survive
    /// round-tripping through the editor.
    #[serde(skip_serializing_if = "JsonValue::is_null")]
    pub raw: JsonValue,
}

impl DeckMeta {
    /// Title to show when the author has not written one.
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or("Untitled deck")
    }
}

/// One slide.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slide {
    /// URL-safe identifier, unique within the deck.
    pub id: String,
    /// Zero-based position.
    pub index: u32,
    pub title: Option<String>,
    /// Markdown body, with notes and step markers already resolved.
    pub content: String,
    pub notes: Vec<String>,
    pub layout: Option<String>,
    pub transition: Option<String>,
    /// Seconds this slide is budgeted, checked against the deck duration.
    pub budget_seconds: Option<u32>,
    /// Safe to skip when running behind. Presenter view marks these.
    pub optional: bool,
    pub steps: StepSource,
    pub timeline: StepTimeline,
    /// One-based line in the source file, for diagnostics and editor jumps.
    pub source_line: u32,
    #[serde(skip_serializing_if = "JsonValue::is_null")]
    pub frontmatter: JsonValue,
}

impl Slide {
    /// Title to show in outlines and presenter view.
    pub fn display_title(&self) -> String {
        self.title.clone().unwrap_or_else(|| format!("Slide {}", self.index + 1))
    }

    /// Number of presenter advances this slide costs.
    pub fn stop_count(&self) -> usize {
        self.timeline.len()
    }

    /// Speaker notes joined into one block.
    pub fn notes_text(&self) -> String {
        self.notes.join("\n\n")
    }

    /// Rough spoken length of the notes, in seconds.
    ///
    /// Uses 150 words per minute for Latin scripts and 300 characters per
    /// minute for CJK, the conventional presentation-pacing figures. It is an
    /// estimate offered before any rehearsal exists, not a measurement.
    pub fn estimated_seconds(&self) -> u32 {
        estimate_speaking_seconds(&self.notes_text())
    }
}

/// Estimates how long a block of speaker notes takes to say aloud.
pub fn estimate_speaking_seconds(text: &str) -> u32 {
    let cjk = text.chars().filter(|c| is_cjk(*c)).count();
    let words = text.split_whitespace().filter(|word| !word.chars().all(is_cjk)).count();

    let cjk_seconds = cjk as f64 / 5.0; // 300 chars per minute
    let word_seconds = words as f64 / 2.5; // 150 words per minute

    (cjk_seconds + word_seconds).round() as u32
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF   // kana
        | 0x3400..=0x4DBF // CJK extension A
        | 0x4E00..=0x9FFF // CJK unified
        | 0xF900..=0xFAFF // compatibility
        | 0xFF66..=0xFF9F // half-width kana
        | 0xAC00..=0xD7AF // hangul
    )
}

/// A parsed deck: metadata, slides, and everything that looked wrong.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deck {
    pub meta: DeckMeta,
    pub slides: Vec<Slide>,
    pub diagnostics: Diagnostics,
}

impl Deck {
    pub fn slide(&self, index: usize) -> Option<&Slide> {
        self.slides.get(index)
    }

    pub fn find(&self, id: &str) -> Option<&Slide> {
        self.slides.iter().find(|slide| slide.id == id)
    }

    /// Total presenter advances across the deck.
    pub fn stop_count(&self) -> usize {
        self.slides.iter().map(Slide::stop_count).sum()
    }

    /// Sum of per-slide budgets, when every slide declares one.
    ///
    /// Returns `None` if any slide is unbudgeted, because a partial sum
    /// compared against the slot length would be misleading.
    pub fn budgeted_seconds(&self) -> Option<u32> {
        self.slides.iter().map(|slide| slide.budget_seconds).sum()
    }

    /// Estimated spoken length from speaker notes.
    pub fn estimated_seconds(&self) -> u32 {
        self.slides.iter().map(Slide::estimated_seconds).sum()
    }

    /// Slides marked as safe to drop when running late.
    pub fn optional_slides(&self) -> Vec<&Slide> {
        self.slides.iter().filter(|slide| slide.optional).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_ratios_round_trip_through_their_tokens() {
        for aspect in [AspectRatio::Wide, AspectRatio::Golden, AspectRatio::Classic] {
            assert_eq!(AspectRatio::parse(aspect.as_token()), Some(aspect));
        }
    }

    #[test]
    fn aspect_ratios_accept_a_slash_spelling() {
        assert_eq!(AspectRatio::parse("16/9"), Some(AspectRatio::Wide));
        assert_eq!(AspectRatio::parse("nonsense"), None);
    }

    #[test]
    fn every_aspect_renders_at_publication_resolution() {
        for aspect in [AspectRatio::Wide, AspectRatio::Golden, AspectRatio::Classic] {
            let (width, height) = aspect.dimensions();
            assert!(
                width >= 1440 && height >= 1080,
                "{} is too small to publish",
                aspect.as_token()
            );
        }
    }

    #[test]
    fn latin_notes_estimate_at_a_conversational_pace() {
        let words = "word ".repeat(150);
        let seconds = estimate_speaking_seconds(&words);
        assert!((55..=65).contains(&seconds), "150 words should be about a minute, got {seconds}");
    }

    #[test]
    fn japanese_notes_estimate_by_character_count() {
        let text = "あ".repeat(300);
        let seconds = estimate_speaking_seconds(&text);
        assert!((55..=65).contains(&seconds), "300 kana should be about a minute, got {seconds}");
    }

    #[test]
    fn empty_notes_estimate_at_zero() {
        assert_eq!(estimate_speaking_seconds(""), 0);
    }

    #[test]
    fn a_budget_total_needs_every_slide_to_declare_one() {
        let mut deck = Deck::default();
        deck.slides.push(Slide { budget_seconds: Some(60), ..Slide::default() });
        assert_eq!(deck.budgeted_seconds(), Some(60));

        deck.slides.push(Slide::default());
        assert_eq!(deck.budgeted_seconds(), None, "a partial total would mislead");
    }

    #[test]
    fn slides_without_a_heading_still_display_something() {
        let slide = Slide { index: 4, ..Slide::default() };
        assert_eq!(slide.display_title(), "Slide 5");
        assert_eq!(DeckMeta::default().display_title(), "Untitled deck");
    }

    #[test]
    fn optional_slides_are_collectable_for_presenter_view() {
        let mut deck = Deck::default();
        deck.slides.push(Slide { id: "a".into(), ..Slide::default() });
        deck.slides.push(Slide { id: "b".into(), optional: true, ..Slide::default() });

        let optional = deck.optional_slides();
        assert_eq!(optional.len(), 1);
        assert_eq!(optional[0].id, "b");
    }

    #[test]
    fn slides_are_addressable_by_id() {
        let mut deck = Deck::default();
        deck.slides.push(Slide { id: "intro".into(), ..Slide::default() });
        assert!(deck.find("intro").is_some());
        assert!(deck.find("missing").is_none());
    }
}
