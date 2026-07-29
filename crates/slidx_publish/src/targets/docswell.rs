//! Docswell, as a payload.
//!
//! The same deck, the same PDF, and deliberately not the same shape. Docswell
//! calls the blurb an overview, addresses a deck by a path with a *minimum*
//! length, and takes a shorter list of shorter tags than Speaker Deck does.
//!
//! Sharing one payload type between the two would mean one set of limits, which
//! would have to be the intersection — and an author would silently lose 3000
//! characters of a Speaker Deck description to a cap that belongs to the other
//! site. Two modules, two sets of numbers, each stated where its fields are.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::fields::{
    optional_text, require_artifact, required_text, resolve_slug, resolve_tags, FileField,
    SlugField, TagField, TextField,
};
use crate::types::{text, ArtifactKind, BlockedReason, Composed, DeckSource};

const PLATFORM: &str = "Docswell";

const TITLE_LIMIT: usize = 100;
const OVERVIEW_LIMIT: usize = 1000;
const PATH_LIMIT: usize = 50;
/// Docswell will not address a deck by one or two characters.
const PATH_MINIMUM: usize = 3;
const TAG_COUNT_LIMIT: usize = 10;
const TAG_LENGTH_LIMIT: usize = 20;
const FILE_BYTES_LIMIT: u64 = 100 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DocswellUpload {
    pub title: String,
    /// Docswell's name for the description.
    pub overview: String,
    /// Path segment under the author's namespace.
    pub path: String,
    pub tags: Vec<String>,
    /// Path to the built PDF. Never read by this crate.
    pub file: String,
    /// Where the talk was given, shown under the title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub presented_at: Option<String>,
}

pub fn compose_docswell(source: &DeckSource) -> Composed<DocswellUpload> {
    let mut reasons: Vec<BlockedReason> = Vec::new();
    let meta = &source.meta;

    let title = required_text(
        meta.title.as_ref(),
        &TextField { name: "title", limit: TITLE_LIMIT, platform: PLATFORM },
        &mut reasons,
    );
    let overview = optional_text(
        meta.description.as_ref(),
        &TextField { name: "description", limit: OVERVIEW_LIMIT, platform: PLATFORM },
        &mut reasons,
    );
    let path = resolve_slug(
        meta,
        &SlugField { limit: PATH_LIMIT, minimum: PATH_MINIMUM, platform: PLATFORM },
        &mut reasons,
    );
    let tags = resolve_tags(
        meta,
        &TagField { count: TAG_COUNT_LIMIT, length: TAG_LENGTH_LIMIT, platform: PLATFORM },
        &mut reasons,
    );
    let file = require_artifact(
        source,
        ArtifactKind::Pdf,
        &FileField {
            byte_limit: FILE_BYTES_LIMIT,
            platform: PLATFORM,
            how_to_build: "set `pdf: true` in the slidx plugin options and build again",
        },
        &mut reasons,
    );

    if !reasons.is_empty() {
        return Composed::Blocked(reasons);
    }

    Composed::Ready(DocswellUpload {
        title,
        overview,
        path,
        tags,
        file,
        presented_at: text(meta.event.as_ref()).map(str::to_string),
    })
}

/// One line for a printed plan.
pub fn describe_docswell(upload: &DocswellUpload) -> String {
    format!(
        "upload {} as \"{}\" (/{}), {} tag(s)",
        upload.file,
        upload.title,
        upload.path,
        upload.tags.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::speakerdeck::compose_speaker_deck;
    use crate::types::{Artifact, DeckMetadata};

    fn meta() -> DeckMetadata {
        DeckMetadata {
            title: Some("Zero-JavaScript Slides".into()),
            description: Some("Why a deck should be plain HTML.".into()),
            event: Some("SlidxConf 2026".into()),
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
                bytes: None,
            }],
            ..DeckSource::default()
        }
    }

    fn fields(composed: &Composed<DocswellUpload>) -> Vec<&str> {
        composed.reasons().iter().map(|reason| reason.field.as_str()).collect()
    }

    #[test]
    fn a_complete_deck_maps_onto_docswells_own_field_names() {
        let composed = compose_docswell(&deck(meta()));
        let upload = composed.value().expect("a payload");

        assert_eq!(upload.overview, "Why a deck should be plain HTML.");
        assert_eq!(upload.path, "zero-javascript-slides");
        assert_eq!(upload.presented_at.as_deref(), Some("SlidxConf 2026"));
    }

    #[test]
    fn a_deck_naming_no_event_omits_the_venue_line() {
        let composed = compose_docswell(&deck(DeckMetadata { event: None, ..meta() }));

        assert_eq!(composed.value().expect("a payload").presented_at, None);
    }

    #[test]
    fn an_overview_speaker_deck_would_have_accepted_is_still_reported_here() {
        // The tempting refactor this file exists to fail: one shared payload
        // would need one set of limits, and the author would silently lose
        // three thousand characters to a cap belonging to another site.
        let source = deck(DeckMetadata { description: Some("a".repeat(2000)), ..meta() });

        assert!(compose_speaker_deck(&source).is_ready());
        assert_eq!(fields(&compose_docswell(&source)), ["description"]);
    }

    #[test]
    fn a_tag_list_speaker_deck_would_have_accepted_is_still_reported_here() {
        let tags: Vec<String> = (0..12).map(|index| format!("tag-{index}")).collect();
        let source = deck(DeckMetadata { tags: Some(tags), ..meta() });

        assert!(compose_speaker_deck(&source).is_ready());
        assert_eq!(fields(&compose_docswell(&source)), ["tags"]);
    }

    #[test]
    fn a_derived_path_is_fitted_to_the_shorter_limit_on_a_word_boundary() {
        let title = "How We Made A Presentation Framework That Ships No JavaScript At All";
        let composed =
            compose_docswell(&deck(DeckMetadata { title: Some(title.into()), ..meta() }));

        assert_eq!(
            composed.value().expect("a payload").path,
            "how-we-made-a-presentation-framework-that-ships"
        );
    }

    #[test]
    fn a_title_too_short_to_address_a_deck_by_is_reported_rather_than_padded() {
        // Two characters is a valid title and not a valid Docswell path.
        // Padding it would invent an address; saying so does not.
        let composed = compose_docswell(&deck(DeckMetadata { title: Some("Go".into()), ..meta() }));

        assert_eq!(fields(&composed), ["slug"]);
    }

    #[test]
    fn a_message_names_docswell_rather_than_the_other_platform() {
        let composed =
            compose_docswell(&deck(DeckMetadata { description: Some("a".repeat(2000)), ..meta() }));
        let messages: String =
            composed.reasons().iter().map(|reason| reason.message.clone()).collect();

        assert!(messages.contains("Docswell"), "{messages}");
        assert!(!messages.contains("Speaker Deck"), "{messages}");
    }
}
