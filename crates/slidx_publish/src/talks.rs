//! Every talk, on one page.
//!
//! The index is the reason the per-talk record is worth writing. An author who
//! has given thirty talks has thirty decks in thirty repositories and no list
//! of them, and assembles one by hand the week they need a speaker bio. The
//! records already carry everything that list needs, so building it is a
//! collection job rather than an authoring one.
//!
//! Two ordering decisions, both about not losing anything:
//!
//! **Most recent first.** That is what a speaking page is read for. Because
//! dates are zero-padded ISO-8601 text, sorting is a string comparison and
//! involves no clock and no time zone.
//!
//! **An undated talk still appears.** It goes after the dated ones, in the
//! order it was given in. Dropping it would lose a talk, and inventing a date
//! to sort it by would put a fabrication in a permanent record.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::targets::archive::{is_orderable_date, ArchiveRecord};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct TalkIndexOptions {
    /// Heading of the page.
    #[ts(optional)]
    pub title: Option<String>,
    #[ts(optional)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TalkIndex {
    pub title: String,
    pub path: String,
    /// Most recent first; undated last, in the order they were given.
    pub talks: Vec<ArchiveRecord>,
    /// How many records still have no recording.
    ///
    /// The one number worth surfacing: it is a to-do list of conferences to
    /// chase, and it is the only thing about an archive that changes after the
    /// fact.
    pub awaiting_recording: usize,
    pub markdown: String,
}

const DEFAULT_TITLE: &str = "Talks";
const DEFAULT_PATH: &str = "talks/index.md";

pub fn build_talk_index(records: &[ArchiveRecord], options: &TalkIndexOptions) -> TalkIndex {
    let talks = order(records);
    let title = named(options.title.as_deref(), DEFAULT_TITLE);

    TalkIndex {
        path: named(options.path.as_deref(), DEFAULT_PATH),
        awaiting_recording: talks.iter().filter(|talk| talk.recording.is_none()).count(),
        markdown: render(&title, &talks),
        title,
        talks,
    }
}

fn named(given: Option<&str>, fallback: &str) -> String {
    given.map(str::trim).filter(|value| !value.is_empty()).unwrap_or(fallback).to_string()
}

/// Dated descending, then undated in input order.
///
/// A stable sort, so two talks on the same day keep the order they arrived in —
/// a morning and an afternoon slot at the same conference read correctly rather
/// than swapping between runs.
fn order(records: &[ArchiveRecord]) -> Vec<ArchiveRecord> {
    let mut dated: Vec<ArchiveRecord> =
        records.iter().filter(|record| is_dated(record)).cloned().collect();
    dated.sort_by(|left, right| right.date.cmp(&left.date));

    let undated = records.iter().filter(|record| !is_dated(record)).cloned();

    dated.into_iter().chain(undated).collect()
}

/// A date that can be ordered, rather than merely present.
///
/// A malformed date sorts wrongly and silently, so a record carrying one is
/// listed as undated instead. The record itself already reports the problem as
/// pending, which is where the author is told to fix it.
fn is_dated(record: &ArchiveRecord) -> bool {
    record.date.as_deref().is_some_and(is_orderable_date)
}

fn render(title: &str, talks: &[ArchiveRecord]) -> String {
    let mut lines = vec![format!("# {title}")];
    let mut heading: Option<String> = None;

    for talk in talks {
        let group = if is_dated(talk) {
            talk.date.as_deref().unwrap_or_default().chars().take(4).collect()
        } else {
            "Undated".to_string()
        };

        if heading.as_deref() != Some(group.as_str()) {
            lines.push(String::new());
            lines.push(format!("## {group}"));
            lines.push(String::new());
            heading = Some(group);
        }

        lines.push(format!("- {}", entry(talk)));
    }

    format!("{}\n", lines.join("\n"))
}

/// One talk, as a line.
///
/// A link appears only when its URL does. An empty `[video]()` is a link that
/// navigates to the page it is on, which is worse than the absence it was
/// standing in for.
fn entry(talk: &ArchiveRecord) -> String {
    let where_given: Vec<&str> =
        [talk.event.as_deref(), talk.venue.as_deref()].into_iter().flatten().collect();

    let mut parts: Vec<String> = Vec::new();
    if let Some(date) = talk.date.as_deref() {
        parts.push(date.to_string());
    }
    parts.push(format!("**{}**", talk.title));
    if !where_given.is_empty() {
        parts.push(where_given.join(", "));
    }
    for (label, url) in [("slides", &talk.deck), ("video", &talk.recording), ("code", &talk.repo)] {
        if let Some(url) = url.as_deref().filter(|url| !url.is_empty()) {
            parts.push(format!("[{label}]({url})"));
        }
    }

    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::archive::compose_archive;
    use crate::types::{DeckMetadata, DeckSource};

    fn record(title: &str, date: Option<&str>, recording: Option<&str>) -> ArchiveRecord {
        let source = DeckSource {
            meta: DeckMetadata {
                title: Some(title.into()),
                event: Some("SlidxConf 2026".into()),
                date: date.map(str::to_string),
                url: Some("https://slidx.dev/talks/zero-js".into()),
                recording: recording.map(str::to_string),
                ..DeckMetadata::default()
            },
            ..DeckSource::default()
        };

        compose_archive(&source).value().cloned().expect("a record")
    }

    fn titles(records: &[ArchiveRecord]) -> Vec<String> {
        build_talk_index(records, &TalkIndexOptions::default())
            .talks
            .into_iter()
            .map(|talk| talk.title)
            .collect()
    }

    #[test]
    fn the_index_leads_with_the_most_recent_talk() {
        let kyoto = record("Kyoto talk", Some("2026-07-29"), None);
        let tokyo = record("Tokyo talk", Some("2025-11-02"), None);
        let osaka = record("Osaka talk", Some("2026-02-14"), None);

        assert_eq!(titles(&[tokyo, kyoto, osaka]), ["Kyoto talk", "Osaka talk", "Tokyo talk"]);
    }

    #[test]
    fn an_undated_talk_is_kept_after_the_dated_ones_in_the_order_it_was_given() {
        // Dropping it would lose a talk. Guessing a date would invent one.
        let kyoto = record("Kyoto talk", Some("2026-07-29"), None);
        let tokyo = record("Tokyo talk", Some("2025-11-02"), None);
        let undated = record("Undated talk", None, None);

        assert_eq!(titles(&[undated, tokyo, kyoto]), ["Kyoto talk", "Tokyo talk", "Undated talk"]);
    }

    #[test]
    fn a_date_the_index_cannot_order_by_is_listed_as_undated_rather_than_mis_sorted() {
        let sloppy = record("Sloppy", Some("2026-7-9"), None);
        let dated = record("Dated", Some("2025-01-01"), None);

        assert_eq!(titles(&[sloppy, dated]), ["Dated", "Sloppy"]);
    }

    #[test]
    fn two_talks_given_the_same_day_keep_the_order_they_arrived_in() {
        let morning = record("Morning", Some("2026-07-29"), None);
        let afternoon = record("Afternoon", Some("2026-07-29"), None);

        assert_eq!(titles(&[morning, afternoon]), ["Morning", "Afternoon"]);
    }

    #[test]
    fn the_page_groups_by_year_because_that_is_how_a_speaking_page_reads() {
        let kyoto = record("Kyoto talk", Some("2026-07-29"), None);
        let tokyo = record("Tokyo talk", Some("2025-11-02"), None);
        let markdown = build_talk_index(&[kyoto, tokyo], &TalkIndexOptions::default()).markdown;

        assert!(markdown.contains("## 2026"), "{markdown}");
        assert!(markdown.find("## 2026") < markdown.find("## 2025"), "{markdown}");
    }

    #[test]
    fn the_recordings_still_outstanding_are_counted_because_that_is_the_chase() {
        let done = record("Done", Some("2026-07-29"), Some("https://youtu.be/abc123"));
        let kyoto = record("Kyoto talk", Some("2026-07-29"), None);
        let tokyo = record("Tokyo talk", Some("2025-11-02"), None);

        assert_eq!(
            build_talk_index(&[done, kyoto, tokyo], &TalkIndexOptions::default())
                .awaiting_recording,
            2
        );
    }

    #[test]
    fn only_a_link_that_exists_is_written() {
        // An empty `[video]()` navigates to the page it is on, which is worse
        // than the absence it was standing in for.
        let kyoto = record("Kyoto talk", Some("2026-07-29"), None);
        let markdown = build_talk_index(&[kyoto], &TalkIndexOptions::default()).markdown;

        assert!(markdown.contains("[slides](https://slidx.dev/talks/zero-js)"), "{markdown}");
        assert!(!markdown.contains("()"), "{markdown}");
    }

    #[test]
    fn an_author_who_has_given_no_talks_yet_still_gets_a_page() {
        let index = build_talk_index(&[], &TalkIndexOptions::default());

        assert!(index.talks.is_empty());
        assert_eq!(index.markdown, "# Talks\n");
        assert_eq!(index.path, "talks/index.md");
    }

    #[test]
    fn a_caller_may_name_the_page_and_say_where_it_goes() {
        let options = TalkIndexOptions {
            title: Some("Speaking".into()),
            path: Some("content/speaking.md".into()),
        };
        let index = build_talk_index(&[], &options);

        assert_eq!(index.title, "Speaking");
        assert!(index.markdown.starts_with("# Speaking"));
        assert_eq!(index.path, "content/speaking.md");
    }
}
