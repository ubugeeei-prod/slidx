//! Turning a deck's metadata and build output into one platform's fields, under
//! that platform's caps.
//!
//! The caps themselves are not here. Each target declares its own, next to the
//! payload they constrain, because a limit stated anywhere other than where the
//! field is documented is a limit that drifts. What lives here is the policy
//! every target shares, which is one sentence:
//!
//! **What the author wrote is passed through or reported; what slidx derived is
//! fitted.**
//!
//! So a 120-character title blocks the step and names `title` — shortening it
//! would publish a sentence the author did not write. A derived slug that is
//! too long is cut on a hyphen, and a suggested tag that does not fit in what
//! is left is dropped, because neither was ever asked for.
//!
//! Reasons accumulate rather than short-circuit. An author fixing a deck at
//! 11pm should learn about all three missing fields at once, not discover the
//! second one after fixing the first.

use crate::text::{ascii_slug, count_characters, fit_slug, normalize_tag, unique_tags};
use crate::types::{reason, ArtifactKind, BlockedReason, DeckMetadata, DeckSource};

/// An upload's file, as a platform documents what it accepts.
#[derive(Debug, Clone, Copy)]
pub struct FileField {
    /// Largest upload the platform takes, in bytes.
    pub byte_limit: u64,
    pub platform: &'static str,
    /// How the build is told to produce it, named in the message.
    pub how_to_build: &'static str,
}

/// The path of a built artifact, if there is one within the size cap.
///
/// The file is never opened. A size the caller measured is checked because an
/// upload rejected for being 4MB over is a failure discovered at the end of the
/// slowest step in the process; a size the caller did not measure is not
/// guessed at, because reading the file would make planning an IO operation.
pub fn require_artifact(
    source: &DeckSource,
    kind: ArtifactKind,
    file: &FileField,
    reasons: &mut Vec<BlockedReason>,
) -> String {
    let field = kind.as_field();

    let Some(artifact) = source.artifact(kind) else {
        reasons.push(reason(
            field,
            format!("{} needs the built {field} — {}", file.platform, file.how_to_build),
        ));
        return String::new();
    };

    if artifact.bytes.is_some_and(|bytes| bytes > file.byte_limit) {
        reasons.push(reason(
            field,
            format!(
                "{} is {}MB; {} accepts {}MB — compress the images or split the deck",
                artifact.path,
                megabytes(artifact.bytes.unwrap_or_default()),
                file.platform,
                megabytes(file.byte_limit),
            ),
        ));
    }

    artifact.path.clone()
}

/// One decimal place, which is the precision a person acts on.
///
/// Written without a trailing `.0`, because "100MB" is what the platform's own
/// documentation says and a report that says "100.0MB" reads as a measurement
/// rather than as the limit it is quoting.
fn megabytes(bytes: u64) -> String {
    let value = (bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0;

    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

/// One text field, as a platform documents it.
#[derive(Debug, Clone, Copy)]
pub struct TextField {
    /// Frontmatter key, so a reason can name the fix.
    pub name: &'static str,
    /// Maximum characters the platform accepts.
    pub limit: usize,
    /// Named in the message, because the same field has different caps.
    pub platform: &'static str,
}

/// A field the platform will not accept an upload without.
///
/// Returns an empty string alongside a recorded reason, so a caller can keep
/// collecting the rest of the problems instead of unwinding on the first.
pub fn required_text(
    value: Option<&String>,
    field: &TextField,
    reasons: &mut Vec<BlockedReason>,
) -> String {
    let trimmed = value.map(|value| value.trim()).unwrap_or_default();

    if trimmed.is_empty() {
        reasons.push(reason(
            field.name,
            format!(
                "{} needs a {} — add `{}:` to the deck frontmatter",
                field.platform, field.name, field.name
            ),
        ));
        return String::new();
    }

    within_limit(trimmed, field, reasons)
}

/// A field the platform accepts empty. Still capped when present.
pub fn optional_text(
    value: Option<&String>,
    field: &TextField,
    reasons: &mut Vec<BlockedReason>,
) -> String {
    let trimmed = value.map(|value| value.trim()).unwrap_or_default();

    if trimmed.is_empty() {
        String::new()
    } else {
        within_limit(trimmed, field, reasons)
    }
}

fn within_limit(text: &str, field: &TextField, reasons: &mut Vec<BlockedReason>) -> String {
    let length = count_characters(text);

    if length > field.limit {
        reasons.push(reason(
            field.name,
            format!(
                "{} is {length} characters; {} accepts {} — shorten it",
                field.name, field.platform, field.limit
            ),
        ));
    }

    text.to_string()
}

/// Tag rules, as a platform documents them.
#[derive(Debug, Clone, Copy)]
pub struct TagField {
    pub count: usize,
    pub length: usize,
    pub platform: &'static str,
}

/// The author's tags, plus the ones the talk itself implies.
///
/// The hashtag and the event are the two tags a conference deck always wants
/// and no author remembers to write twice. They are appended only while there
/// is room under the platform's cap: they are slidx's suggestion, so they yield
/// to anything the author chose, and to the cap itself.
///
/// Too many *authored* tags is a different thing entirely, and blocks. Dropping
/// the tail of a list someone wrote by hand publishes a deck tagged with what
/// happened to sort first.
pub fn resolve_tags(
    meta: &DeckMetadata,
    field: &TagField,
    reasons: &mut Vec<BlockedReason>,
) -> Vec<String> {
    let authored = unique_tags(meta.tags.as_deref().unwrap_or_default());

    if let Some(overlong) = authored.iter().find(|tag| count_characters(tag) > field.length) {
        reasons.push(reason(
            "tags",
            format!(
                "tag `{overlong}` is longer than the {} characters {} allows — shorten it",
                field.length, field.platform
            ),
        ));
    }

    if authored.len() > field.count {
        reasons.push(reason(
            "tags",
            format!(
                "the deck has {} tags; {} accepts {} — remove {}",
                authored.len(),
                field.platform,
                field.count,
                authored.len() - field.count
            ),
        ));
    }

    let suggested: Vec<String> = [meta.hashtag.as_ref(), meta.event.as_ref()]
        .into_iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .map(|value| normalize_tag(value))
        .filter(|tag| count_characters(tag) <= field.length)
        .collect();

    let mut tags = authored;

    for tag in unique_tags(&suggested) {
        if tags.len() >= field.count {
            break;
        }
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }

    tags
}

/// Slug rules, as a platform documents them.
#[derive(Debug, Clone, Copy)]
pub struct SlugField {
    pub limit: usize,
    /// Shortest path segment the platform will store.
    pub minimum: usize,
    pub platform: &'static str,
}

/// The path segment the deck will live at.
///
/// An author who pinned a slug gets it verbatim or gets told why it will not
/// work — a URL is an address other people have already written down, and one
/// silently reshaped by us is a link that stops resolving.
///
/// A derived slug is fitted. A title with no Latin characters yields nothing to
/// derive from, which is reported rather than filled with the slide index: an
/// address that means nothing is worse than an address the author chooses.
pub fn resolve_slug(
    meta: &DeckMetadata,
    field: &SlugField,
    reasons: &mut Vec<BlockedReason>,
) -> String {
    let authored = meta.slug.as_deref().map(str::trim).filter(|slug| !slug.is_empty());

    if let Some(authored) = authored {
        let length = count_characters(authored);

        if !is_path_segment(authored) {
            reasons.push(reason(
                "slug",
                format!(
                    "slug `{authored}` is not a path {} accepts — use lowercase letters, \
                     digits, and single hyphens",
                    field.platform
                ),
            ));
        } else if length > field.limit {
            reasons.push(reason(
                "slug",
                format!(
                    "slug `{authored}` is {length} characters; {} accepts {} — shorten it",
                    field.platform, field.limit
                ),
            ));
        } else if length < field.minimum {
            reasons.push(reason(
                "slug",
                format!(
                    "slug `{authored}` is shorter than the {} characters {} requires — \
                     lengthen it",
                    field.minimum, field.platform
                ),
            ));
        }

        return authored.to_string();
    }

    let derived = fit_slug(&ascii_slug(meta.title.as_deref().unwrap_or_default()), field.limit);

    if count_characters(&derived) < field.minimum {
        reasons.push(reason(
            "slug",
            format!(
                "the title yields no {} path of at least {} characters — add `slug:` to the \
                 deck frontmatter",
                field.platform, field.minimum
            ),
        ));
    }

    derived
}

/// Lowercase letters, digits, and single hyphens between them.
///
/// The intersection of what the slide hosts accept in a path. Checked rather
/// than rewritten: see [`resolve_slug`].
fn is_path_segment(slug: &str) -> bool {
    !slug.is_empty()
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && !slug.contains("--")
        && slug.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Artifact, DeckSlide};

    const SPEAKER_DECK: &str = "Speaker Deck";

    fn meta() -> DeckMetadata {
        DeckMetadata {
            title: Some("Zero-JavaScript Slides".into()),
            event: Some("SlidxConf 2026".into()),
            hashtag: Some("slidxconf".into()),
            tags: Some(vec!["rust".into(), "slides".into()]),
            ..DeckMetadata::default()
        }
    }

    fn slug_field() -> SlugField {
        SlugField { limit: 100, minimum: 1, platform: SPEAKER_DECK }
    }

    fn tag_field() -> TagField {
        TagField { count: 20, length: 30, platform: SPEAKER_DECK }
    }

    fn fields(reasons: &[BlockedReason]) -> Vec<&str> {
        reasons.iter().map(|entry| entry.field.as_str()).collect()
    }

    fn messages(reasons: &[BlockedReason]) -> String {
        reasons.iter().map(|entry| entry.message.clone()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn a_missing_required_field_is_named_by_the_frontmatter_key_that_would_fix_it() {
        let mut reasons = Vec::new();
        let field = TextField { name: "title", limit: 100, platform: SPEAKER_DECK };

        assert_eq!(required_text(None, &field, &mut reasons), "");
        assert_eq!(fields(&reasons), ["title"]);
        assert!(messages(&reasons).contains("`title:`"), "{}", messages(&reasons));
    }

    #[test]
    fn a_field_one_character_over_the_cap_is_reported_rather_than_shortened() {
        // Shortening it would publish a sentence the author did not write.
        let mut reasons = Vec::new();
        let field = TextField { name: "title", limit: 100, platform: SPEAKER_DECK };
        let title = "a".repeat(101);

        assert_eq!(required_text(Some(&title), &field, &mut reasons), title);
        assert!(messages(&reasons).contains("101 characters"), "{}", messages(&reasons));
    }

    #[test]
    fn a_field_exactly_at_the_cap_is_accepted() {
        let mut reasons = Vec::new();
        let field = TextField { name: "title", limit: 100, platform: SPEAKER_DECK };

        required_text(Some(&"a".repeat(100)), &field, &mut reasons);
        assert!(reasons.is_empty());
    }

    #[test]
    fn an_optional_field_left_out_reports_nothing_and_composes_as_empty() {
        let mut reasons = Vec::new();
        let field = TextField { name: "description", limit: 4000, platform: SPEAKER_DECK };

        assert_eq!(optional_text(None, &field, &mut reasons), "");
        assert!(reasons.is_empty());
    }

    #[test]
    fn a_cap_counts_the_characters_a_person_sees_rather_than_utf16_units() {
        // The whole reason `text::count_characters` exists: an emoji costs one.
        let mut reasons = Vec::new();
        let field = TextField { name: "title", limit: 2, platform: SPEAKER_DECK };

        optional_text(Some(&"🎤a".to_string()), &field, &mut reasons);
        assert!(reasons.is_empty());
    }

    #[test]
    fn the_authors_tags_come_first_and_the_talks_own_are_appended() {
        let mut reasons = Vec::new();

        assert_eq!(
            resolve_tags(&meta(), &tag_field(), &mut reasons),
            ["rust", "slides", "slidxconf", "slidxconf-2026"]
        );
        assert!(reasons.is_empty());
    }

    #[test]
    fn a_deck_that_names_no_talk_gets_no_suggested_tags() {
        let mut reasons = Vec::new();
        let meta = DeckMetadata { hashtag: None, event: None, ..meta() };

        assert_eq!(resolve_tags(&meta, &tag_field(), &mut reasons), ["rust", "slides"]);
    }

    #[test]
    fn slidx_drops_its_own_suggestions_at_the_cap_rather_than_the_authors_tags() {
        let mut reasons = Vec::new();
        let tags: Vec<String> = (0..20).map(|index| format!("tag-{index}")).collect();
        let meta = DeckMetadata { tags: Some(tags.clone()), ..meta() };

        assert_eq!(resolve_tags(&meta, &tag_field(), &mut reasons), tags);
        assert!(reasons.is_empty());
    }

    #[test]
    fn more_authored_tags_than_the_platform_stores_blocks_rather_than_dropping_the_tail() {
        // Dropping the tail of a hand-written list would publish a deck tagged
        // with whatever happened to sort first.
        let mut reasons = Vec::new();
        let tags: Vec<String> = (0..21).map(|index| format!("tag-{index}")).collect();
        let meta = DeckMetadata { tags: Some(tags), ..meta() };

        resolve_tags(&meta, &tag_field(), &mut reasons);
        assert_eq!(fields(&reasons), ["tags"]);
        assert!(messages(&reasons).contains("remove 1"), "{}", messages(&reasons));
    }

    #[test]
    fn a_suggested_tag_too_long_for_the_platform_is_dropped_rather_than_reported() {
        // It is slidx's suggestion, not the author's line to fix.
        let mut reasons = Vec::new();
        let meta = DeckMetadata { event: Some("a".repeat(31)), ..meta() };

        assert_eq!(
            resolve_tags(&meta, &tag_field(), &mut reasons),
            ["rust", "slides", "slidxconf"]
        );
        assert!(reasons.is_empty());
    }

    #[test]
    fn a_slug_the_author_pinned_is_used_verbatim() {
        // A URL is an address other people have already written down.
        let mut reasons = Vec::new();
        let meta = DeckMetadata { slug: Some("zero-js".into()), ..meta() };

        assert_eq!(resolve_slug(&meta, &slug_field(), &mut reasons), "zero-js");
        assert!(reasons.is_empty());
    }

    #[test]
    fn a_pinned_slug_the_platform_will_not_store_is_reported_rather_than_rewritten() {
        let mut reasons = Vec::new();
        let meta = DeckMetadata { slug: Some("Zero JS!".into()), ..meta() };

        resolve_slug(&meta, &slug_field(), &mut reasons);
        assert_eq!(fields(&reasons), ["slug"]);
    }

    #[test]
    fn a_derived_slug_comes_from_the_title_and_is_fitted_to_the_cap() {
        let mut reasons = Vec::new();
        let field = SlugField { limit: 20, minimum: 1, platform: SPEAKER_DECK };

        assert_eq!(resolve_slug(&meta(), &field, &mut reasons), "zero-javascript");
        assert!(reasons.is_empty());
    }

    #[test]
    fn a_title_with_nothing_latin_in_it_is_reported_rather_than_given_an_invented_address() {
        let mut reasons = Vec::new();
        let meta = DeckMetadata { title: Some("日本語のスライド".into()), ..meta() };

        resolve_slug(&meta, &slug_field(), &mut reasons);
        assert_eq!(fields(&reasons), ["slug"]);
        assert!(messages(&reasons).contains("`slug:`"), "{}", messages(&reasons));
    }

    #[test]
    fn a_platform_with_a_minimum_path_length_says_so_rather_than_padding() {
        let mut reasons = Vec::new();
        let field = SlugField { limit: 50, minimum: 3, platform: "Docswell" };
        let meta = DeckMetadata { title: Some("Go".into()), ..meta() };

        resolve_slug(&meta, &field, &mut reasons);
        assert!(messages(&reasons).contains("at least 3 characters"), "{}", messages(&reasons));
    }

    #[test]
    fn an_unmeasured_artifact_is_accepted_rather_than_opened_to_find_out() {
        // Reading the file would make planning an IO operation, and a plan that
        // touches the disk can fail for reasons that have nothing to do with
        // the deck.
        let mut reasons = Vec::new();
        let source = DeckSource {
            artifacts: vec![Artifact {
                kind: ArtifactKind::Pdf,
                path: "dist/deck.pdf".into(),
                bytes: None,
            }],
            ..DeckSource::default()
        };
        let file = FileField {
            byte_limit: 100 * 1024 * 1024,
            platform: SPEAKER_DECK,
            how_to_build: "set `pdf: true`",
        };

        assert_eq!(
            require_artifact(&source, ArtifactKind::Pdf, &file, &mut reasons),
            "dist/deck.pdf"
        );
        assert!(reasons.is_empty());
    }

    #[test]
    fn an_artifact_over_the_upload_cap_is_reported_in_the_units_the_platform_uses() {
        let mut reasons = Vec::new();
        let source = DeckSource {
            slides: vec![DeckSlide::default()],
            artifacts: vec![Artifact {
                kind: ArtifactKind::Pdf,
                path: "dist/deck.pdf".into(),
                bytes: Some(120 * 1024 * 1024),
            }],
            ..DeckSource::default()
        };
        let file = FileField {
            byte_limit: 100 * 1024 * 1024,
            platform: SPEAKER_DECK,
            how_to_build: "set `pdf: true`",
        };

        require_artifact(&source, ArtifactKind::Pdf, &file, &mut reasons);
        assert!(messages(&reasons).contains("120MB"), "{}", messages(&reasons));
        assert!(messages(&reasons).contains("100MB"), "{}", messages(&reasons));
    }

    #[test]
    fn a_size_between_whole_megabytes_keeps_the_one_decimal_a_person_acts_on() {
        assert_eq!(megabytes(1024 * 1024 * 3 / 2), "1.5");
        assert_eq!(megabytes(100 * 1024 * 1024), "100");
    }

    #[test]
    fn a_missing_artifact_says_how_the_build_is_told_to_produce_one() {
        let mut reasons = Vec::new();
        let file = FileField {
            byte_limit: 1,
            platform: SPEAKER_DECK,
            how_to_build: "set `pdf: true` in the slidx plugin options and build again",
        };

        require_artifact(&DeckSource::default(), ArtifactKind::Pdf, &file, &mut reasons);
        assert_eq!(fields(&reasons), ["pdf"]);
        assert!(messages(&reasons).contains("`pdf: true`"), "{}", messages(&reasons));
    }
}
