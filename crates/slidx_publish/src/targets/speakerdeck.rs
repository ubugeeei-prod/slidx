//! Speaker Deck, as a payload.
//!
//! Speaker Deck is a PDF host: the deck is the file, and everything else is the
//! page around it. That makes the failure mode specific — the upload is the
//! slowest step in publishing, and a title two characters over the cap fails
//! *after* the file has gone up.
//!
//! The numbers below are the platform's documented limits, read conservatively
//! on purpose. Being ten characters under costs nothing; being one over costs a
//! re-upload at the end of a long day.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::fields::{
    optional_text, require_artifact, required_text, resolve_slug, resolve_tags, FileField,
    SlugField, TagField, TextField,
};
use crate::types::{ArtifactKind, BlockedReason, Composed, DeckSource};

const PLATFORM: &str = "Speaker Deck";

const TITLE_LIMIT: usize = 100;
const DESCRIPTION_LIMIT: usize = 4000;
const SLUG_LIMIT: usize = 100;
const TAG_COUNT_LIMIT: usize = 20;
const TAG_LENGTH_LIMIT: usize = 30;
const PDF_BYTES_LIMIT: u64 = 100 * 1024 * 1024;

/// What an upload consists of.
///
/// Field names are Speaker Deck's, not slidx's. The whole value of a typed
/// payload is that it can be handed to whatever performs the upload without a
/// second mapping step in between, where a renamed field goes missing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerDeckUpload {
    pub title: String,
    pub description: String,
    /// Path segment under the author's profile.
    pub slug: String,
    pub tags: Vec<String>,
    /// Path to the built PDF. Never read by this crate.
    pub pdf: String,
    /// Talk date, shown on the deck page. ISO-8601, as authored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub date: Option<String>,
}

pub fn compose_speaker_deck(source: &DeckSource) -> Composed<SpeakerDeckUpload> {
    let mut reasons: Vec<BlockedReason> = Vec::new();
    let meta = &source.meta;

    let title = required_text(
        meta.title.as_ref(),
        &TextField { name: "title", limit: TITLE_LIMIT, platform: PLATFORM },
        &mut reasons,
    );
    let description = optional_text(
        meta.description.as_ref(),
        &TextField { name: "description", limit: DESCRIPTION_LIMIT, platform: PLATFORM },
        &mut reasons,
    );
    let slug = resolve_slug(
        meta,
        &SlugField { limit: SLUG_LIMIT, minimum: 1, platform: PLATFORM },
        &mut reasons,
    );
    let tags = resolve_tags(
        meta,
        &TagField { count: TAG_COUNT_LIMIT, length: TAG_LENGTH_LIMIT, platform: PLATFORM },
        &mut reasons,
    );
    let pdf = require_artifact(
        source,
        ArtifactKind::Pdf,
        &FileField {
            byte_limit: PDF_BYTES_LIMIT,
            platform: PLATFORM,
            how_to_build: "set `pdf: true` in the slidx plugin options and build again",
        },
        &mut reasons,
    );

    if !reasons.is_empty() {
        return Composed::Blocked(reasons);
    }

    Composed::Ready(SpeakerDeckUpload {
        title,
        description,
        slug,
        tags,
        pdf,
        date: crate::types::text(meta.date.as_ref()).map(str::to_string),
    })
}

/// One line for a printed plan.
pub fn describe_speaker_deck(upload: &SpeakerDeckUpload) -> String {
    format!(
        "upload {} as \"{}\" (/{}), {} tag(s)",
        upload.pdf,
        upload.title,
        upload.slug,
        upload.tags.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Artifact, DeckMetadata};

    fn meta() -> DeckMetadata {
        DeckMetadata {
            title: Some("Zero-JavaScript Slides".into()),
            description: Some("Why a deck should be plain HTML.".into()),
            event: Some("SlidxConf 2026".into()),
            date: Some("2026-07-29".into()),
            hashtag: Some("slidxconf".into()),
            tags: Some(vec!["rust".into(), "slides".into()]),
            ..DeckMetadata::default()
        }
    }

    fn deck(meta: DeckMetadata) -> DeckSource {
        DeckSource {
            meta,
            artifacts: vec![Artifact {
                kind: ArtifactKind::Pdf,
                path: "dist/deck.pdf".into(),
                bytes: Some(4 * 1024 * 1024),
            }],
            ..DeckSource::default()
        }
    }

    fn fields(composed: &Composed<SpeakerDeckUpload>) -> Vec<&str> {
        composed.reasons().iter().map(|reason| reason.field.as_str()).collect()
    }

    #[test]
    fn a_complete_deck_maps_onto_speaker_decks_own_field_names() {
        let composed = compose_speaker_deck(&deck(meta()));
        let upload = composed.value().expect("a payload");

        assert_eq!(upload.title, "Zero-JavaScript Slides");
        assert_eq!(upload.slug, "zero-javascript-slides");
        assert_eq!(upload.pdf, "dist/deck.pdf");
        assert_eq!(upload.date.as_deref(), Some("2026-07-29"));
    }

    #[test]
    fn a_deck_with_no_date_omits_it_rather_than_inventing_todays() {
        // A plan that read a clock would not be diffable, and a deck given
        // today's date for last month's talk is simply wrong.
        let composed = compose_speaker_deck(&deck(DeckMetadata { date: None, ..meta() }));

        assert_eq!(composed.value().expect("a payload").date, None);
    }

    #[test]
    fn a_description_is_optional_because_the_platform_does_not_require_one() {
        let composed = compose_speaker_deck(&deck(DeckMetadata { description: None, ..meta() }));

        assert_eq!(composed.value().expect("a payload").description, "");
    }

    #[test]
    fn everything_wrong_is_reported_in_one_pass() {
        // Two problems found in one pass is one trip back to the frontmatter,
        // not two.
        let source =
            DeckSource { meta: DeckMetadata { title: None, ..meta() }, ..DeckSource::default() };

        assert_eq!(fields(&compose_speaker_deck(&source)), ["title", "slug", "pdf"]);
    }

    #[test]
    fn the_plan_line_says_what_would_be_uploaded_and_where_it_would_live() {
        let composed = compose_speaker_deck(&deck(meta()));
        let line = describe_speaker_deck(composed.value().expect("a payload"));

        assert_eq!(
            line,
            "upload dist/deck.pdf as \"Zero-JavaScript Slides\" (/zero-javascript-slides), 4 tag(s)"
        );
    }
}
