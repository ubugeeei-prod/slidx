//! The talk's permanent record — the one target whose input is not finished
//! when it first runs.
//!
//! Every other target here composes something that is true the evening of the
//! talk and never changes again. The archive record is different: the slides go
//! up that night, and the recording appears when the conference gets round to
//! publishing it, which is weeks and sometimes never. By then the author has
//! moved on, and the video and the slides live in two places that never learn
//! about each other.
//!
//! So this target is built to be run twice. It composes from whatever exists
//! now, and it distinguishes two things the other targets treat alike:
//!
//! - **Blocked** is a field the author can add right now. Only one thing blocks
//!   here, and it is having nothing at all to name the talk by.
//! - **Pending** is a field the world has not produced yet. The author cannot
//!   make a conference publish a video, so a missing recording is a reason to
//!   come back, not a reason to refuse.
//!
//! The second property that follows from running twice: adding the recording
//! months later must change exactly one line of the record. That is why the
//! recording appears in the frontmatter and nowhere else — a body that also
//! linked it would make the eventual diff two changes instead of one, and a
//! diff an author cannot skim is a diff they stop reading.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::targets::yaml_string;
use crate::text::file_slug;
use crate::types::{reason, text, BlockedReason, Composed, DeckSource};

/// A talk, as it will be remembered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct ArchiveRecord {
    /// File name stem. Stable, so re-running overwrites rather than piles up.
    pub slug: String,
    pub path: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub event: Option<String>,
    /// ISO-8601, as authored. Kept as text so ordering never needs a clock.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub venue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    /// Where the slides ended up.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub deck: Option<String>,
    /// The recording, once there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub recording: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub repo: Option<String>,
    pub tags: Vec<String>,
    /// What is not here *yet*, and when to come back for it.
    pub pending: Vec<BlockedReason>,
    pub markdown: String,
}

/// Where the records live, relative to the deck.
const ARCHIVE_DIRECTORY: &str = "talks";

pub fn compose_archive(source: &DeckSource) -> Composed<ArchiveRecord> {
    let meta = &source.meta;

    let title = text(meta.title.as_ref()).or_else(|| text(meta.event.as_ref()));

    // The only thing that blocks. A record with no title and no event cannot be
    // filed, cannot be listed, and cannot be found again — it is not a record
    // of anything.
    let Some(title) = title else {
        return Composed::Blocked(vec![reason(
            "title",
            "nothing names this talk — add `title:` to the deck frontmatter, or `event:`",
        )]);
    };

    let mut pending: Vec<BlockedReason> = Vec::new();
    let slug = resolve_archive_slug(
        title,
        text(meta.slug.as_ref()),
        text(meta.date.as_ref()),
        &mut pending,
    );
    let date = resolve_date(text(meta.date.as_ref()), &mut pending);

    let deck = text(meta.url.as_ref());
    if deck.is_none() {
        pending.push(reason(
            "url",
            "add `url:` once the deck is published — usually the same evening",
        ));
    }

    let recording = text(meta.recording.as_ref());
    if recording.is_none() {
        pending
            .push(reason("recording", "add `recording:` when the conference publishes the video"));
    }

    let record = ArchiveRecord {
        path: format!("{ARCHIVE_DIRECTORY}/{slug}.md"),
        slug,
        title: title.to_string(),
        event: text(meta.event.as_ref()).map(str::to_string),
        date: date.map(str::to_string),
        venue: text(meta.venue.as_ref()).map(str::to_string),
        author: text(meta.author.as_ref()).map(str::to_string),
        description: text(meta.description.as_ref()).map(str::to_string),
        deck: deck.map(str::to_string),
        recording: recording.map(str::to_string),
        repo: text(meta.repo.as_ref()).map(str::to_string),
        tags: meta.tags.clone().unwrap_or_default(),
        pending,
        markdown: String::new(),
    };

    Composed::Ready(ArchiveRecord { markdown: render_record(&record), ..record })
}

/// The file name.
///
/// [`file_slug`] rather than `ascii_slug`: this file lives on the author's own
/// disk, so a Japanese talk gets a Japanese file name instead of being reduced
/// to nothing. A title that yields no slug at all — punctuation, or an emoji —
/// falls back to the date, and says so, because a file called `-.md` is a file
/// nobody finds twice.
fn resolve_archive_slug(
    title: &str,
    pinned: Option<&str>,
    date: Option<&str>,
    pending: &mut Vec<BlockedReason>,
) -> String {
    if let Some(pinned) = pinned {
        return pinned.to_string();
    }

    let derived = file_slug(title);
    if !derived.is_empty() {
        return derived;
    }

    let fallback =
        date.map_or_else(|| "talk".to_string(), |date| format!("talk-{}", file_slug(date)));

    pending.push(reason(
        "slug",
        format!("the title yields no file name — add `slug:`, or this is filed as {fallback}"),
    ));

    fallback
}

/// A date the index can order by.
///
/// Checked rather than trusted, because the failure is silent: `2026-7-9` sorts
/// as text *before* `2026-11-01`, so a talk index with one sloppy date puts
/// November ahead of July and nobody reads it carefully enough to notice.
fn resolve_date<'a>(date: Option<&'a str>, pending: &mut Vec<BlockedReason>) -> Option<&'a str> {
    let Some(date) = date else {
        pending.push(reason("date", "add `date:` so the talk sorts into the index"));
        return None;
    };

    if !is_orderable_date(date) {
        pending.push(reason(
            "date",
            format!("`{date}` is not an ISO-8601 date — write it as YYYY-MM-DD so it sorts"),
        ));
    }

    Some(date)
}

/// True when a date sorts correctly as plain text.
///
/// Zero-padded ISO-8601 is the one format where lexical order and chronological
/// order agree, which is what lets the index sort without parsing a date and
/// without a time zone entering the picture.
pub fn is_orderable_date(date: &str) -> bool {
    let bytes = date.as_bytes();

    let shaped = bytes.len() >= 10
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
        // A time may follow, separated the way ISO-8601 or a spreadsheet writes
        // it. Anything else glued to the date is not a date at all.
        && matches!(bytes.get(10), None | Some(b'T') | Some(b' '));

    if !shaped {
        return false;
    }

    let number = |range: std::ops::Range<usize>| date[range].parse::<u32>().unwrap_or(0);

    (1..=12).contains(&number(5..7)) && (1..=31).contains(&number(8..10))
}

/// The record as a file a static site can read without knowing about slidx.
fn render_record(record: &ArchiveRecord) -> String {
    let mut front: Vec<String> = [
        ("title", Some(record.title.clone())),
        ("event", record.event.clone()),
        ("date", record.date.clone()),
        ("venue", record.venue.clone()),
        ("author", record.author.clone()),
        ("description", record.description.clone()),
        ("deck", record.deck.clone()),
        ("recording", record.recording.clone()),
        ("repo", record.repo.clone()),
    ]
    .into_iter()
    // A key omitted rather than emitted empty. `recording: ""` reads to a site
    // template as "there is no recording", which is a different claim from
    // "not yet" and the one thing this target exists to keep straight.
    .filter_map(|(key, value)| value.map(|value| format!("{key}: {}", yaml_string(&value))))
    .collect();

    if !record.tags.is_empty() {
        let written: Vec<String> = record.tags.iter().map(|tag| yaml_string(tag)).collect();
        front.push(format!("tags: [{}]", written.join(", ")));
    }

    let body: Vec<String> = [Some(format!("# {}", record.title)), record.description.clone()]
        .into_iter()
        .flatten()
        .collect();

    format!("---\n{}\n---\n\n{}\n", front.join("\n"), body.join("\n\n"))
}

/// One line for a printed plan.
pub fn describe_archive(record: &ArchiveRecord) -> String {
    if record.pending.is_empty() {
        return format!("write {}", record.path);
    }

    let mut fields: Vec<&str> = Vec::new();
    for entry in &record.pending {
        if !fields.contains(&entry.field.as_str()) {
            fields.push(&entry.field);
        }
    }

    format!("write {} — awaiting {}", record.path, fields.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DeckMetadata;

    fn meta() -> DeckMetadata {
        DeckMetadata {
            title: Some("Zero-JavaScript Slides".into()),
            description: Some("Why a deck should be plain HTML.".into()),
            author: Some("ubugeeei".into()),
            event: Some("SlidxConf 2026".into()),
            date: Some("2026-07-29".into()),
            venue: Some("Kyoto".into()),
            url: Some("https://slidx.dev/talks/zero-js".into()),
            repo: Some("https://github.com/ubugeeei-prod/slidx".into()),
            tags: Some(vec!["rust".into(), "slides".into()]),
            ..DeckMetadata::default()
        }
    }

    fn record(meta: DeckMetadata) -> ArchiveRecord {
        let source = DeckSource { meta, ..DeckSource::default() };

        compose_archive(&source).value().cloned().expect("a record")
    }

    fn pending_fields(meta: DeckMetadata) -> Vec<String> {
        record(meta).pending.into_iter().map(|entry| entry.field).collect()
    }

    #[test]
    fn a_record_keeps_the_fields_the_author_already_wrote() {
        let entry = record(meta());

        assert_eq!(entry.title, "Zero-JavaScript Slides");
        assert_eq!(entry.event.as_deref(), Some("SlidxConf 2026"));
        assert_eq!(entry.venue.as_deref(), Some("Kyoto"));
        assert_eq!(entry.deck.as_deref(), Some("https://slidx.dev/talks/zero-js"));
    }

    #[test]
    fn a_record_is_filed_under_a_slug_so_re_running_overwrites_rather_than_piles_up() {
        assert_eq!(record(meta()).path, "talks/zero-javascript-slides.md");
        assert_eq!(
            record(DeckMetadata { slug: Some("zero-js".into()), ..meta() }).path,
            "talks/zero-js.md"
        );
    }

    #[test]
    fn a_non_latin_title_keeps_its_own_file_name_because_the_file_is_the_authors() {
        // Unlike a slide-host URL, this path never leaves the author's disk.
        let entry =
            record(DeckMetadata { title: Some("プレーンな HTML の話".into()), ..meta() });

        assert_eq!(entry.path, "talks/プレーンな-html-の話.md");
    }

    #[test]
    fn a_title_that_yields_no_file_name_falls_back_to_the_date_and_says_so() {
        let entry = record(DeckMetadata { title: Some("!!! ???".into()), ..meta() });

        assert_eq!(entry.path, "talks/talk-2026-07-29.md");
        assert!(entry.pending.iter().any(|item| item.field == "slug"));
    }

    #[test]
    fn a_talk_with_nothing_but_a_title_is_still_recorded_because_it_still_happened() {
        let bare = DeckMetadata { title: Some("Lightning talk".into()), ..DeckMetadata::default() };
        let named_only_by_its_event = DeckMetadata {
            event: Some("SlidxConf 2026".into()),
            date: Some("2026-07-29".into()),
            ..DeckMetadata::default()
        };

        for meta in [bare, named_only_by_its_event] {
            let source = DeckSource { meta, ..DeckSource::default() };
            assert!(compose_archive(&source).is_ready());
        }
    }

    #[test]
    fn only_having_nothing_to_name_the_talk_by_refuses_to_record_it() {
        let source = DeckSource {
            meta: DeckMetadata { author: Some("ubugeeei".into()), ..DeckMetadata::default() },
            ..DeckSource::default()
        };
        let composed = compose_archive(&source);

        assert!(!composed.is_ready());
        assert_eq!(composed.reasons()[0].field, "title");
    }

    #[test]
    fn a_missing_recording_is_a_reason_to_come_back_rather_than_a_reason_to_refuse() {
        // The distinction this target exists for. The author cannot make the
        // conference publish the video.
        let entry = record(meta());

        assert_eq!(entry.recording, None);
        assert!(pending_fields(meta()).contains(&"recording".to_string()));

        let attached = DeckMetadata { recording: Some("https://youtu.be/abc123".into()), ..meta() };
        assert!(!pending_fields(attached).contains(&"recording".to_string()));
    }

    #[test]
    fn the_deck_url_is_pending_too_since_it_is_usually_published_the_same_evening() {
        assert!(pending_fields(DeckMetadata { url: None, ..meta() }).contains(&"url".to_string()));
    }

    #[test]
    fn a_date_the_index_cannot_order_by_is_reported_rather_than_silently_mis_sorted() {
        // `2026-7-9` sorts before `2026-11-01` as text. A talk index that puts
        // November before July is wrong in a way nobody notices.
        let sloppy = DeckMetadata { date: Some("2026-7-9".into()), ..meta() };

        assert!(pending_fields(sloppy).contains(&"date".to_string()));
    }

    #[test]
    fn a_complete_record_has_nothing_pending() {
        let complete = DeckMetadata { recording: Some("https://youtu.be/abc123".into()), ..meta() };

        assert!(record(complete).pending.is_empty());
    }

    #[test]
    fn the_record_is_a_file_a_static_site_can_read_without_knowing_about_slidx() {
        let complete = DeckMetadata { recording: Some("https://youtu.be/abc123".into()), ..meta() };
        let markdown = record(complete).markdown;

        assert!(markdown.starts_with("---\n"), "{markdown}");
        assert!(markdown.contains("title: \"Zero-JavaScript Slides\""), "{markdown}");
        assert!(markdown.contains("recording: \"https://youtu.be/abc123\""), "{markdown}");
        assert!(!record(meta()).markdown.contains("recording:"));
    }

    #[test]
    fn only_the_recording_changes_when_only_the_recording_changed() {
        // The property that makes re-running months later safe: the diff an
        // author sees is the thing they actually did.
        let before = record(meta()).markdown;
        let complete = DeckMetadata { recording: Some("https://youtu.be/abc123".into()), ..meta() };
        let after = record(complete).markdown;

        let added: Vec<&str> =
            after.lines().filter(|line| !before.lines().any(|old| old == *line)).collect();

        assert_eq!(added, ["recording: \"https://youtu.be/abc123\""]);
    }

    #[test]
    fn a_title_with_a_colon_in_it_does_not_become_two_keys() {
        let awkward =
            DeckMetadata { title: Some(r#"Slides: a "talk" about talks"#.into()), ..meta() };

        assert!(
            record(awkward.clone()).markdown.contains(r#"title: "Slides: a \"talk\" about talks""#),
            "{}",
            record(awkward).markdown
        );
    }

    #[test]
    fn a_date_is_orderable_only_when_lexical_order_and_chronology_agree() {
        assert!(is_orderable_date("2026-07-29"));
        assert!(is_orderable_date("2026-07-29T10:00:00Z"));
        assert!(!is_orderable_date("2026-7-9"));
        assert!(!is_orderable_date("29/07/2026"));
        assert!(!is_orderable_date("2026-13-01"));
        assert!(!is_orderable_date("2026-07-291"));
    }

    #[test]
    fn the_plan_line_names_what_is_still_outstanding() {
        assert!(describe_archive(&record(meta())).contains("talks/zero-javascript-slides.md"));
        assert!(describe_archive(&record(meta())).contains("recording"));

        let complete = DeckMetadata { recording: Some("https://youtu.be/abc123".into()), ..meta() };
        assert!(!describe_archive(&record(complete)).contains("awaiting"));
    }
}
