//! A record, read back off disk.
//!
//! The archive is the one target built to be run twice, and running it twice is
//! only useful if the records already written can be found again — the talk
//! index is assembled from all of them, and it has to include the talk from
//! last spring whose deck nobody is going to re-plan today.
//!
//! Reading lives beside writing on purpose. A format described in two modules
//! is a format that drifts, and this one drifts silently: an index that quietly
//! stopped finding `venue` would still render, just with a column missing that
//! nobody notices until they read their own speaking page.
//!
//! Nothing here reconstructs a record. It recovers the *frontmatter* a record
//! was written from and hands it back to [`compose_archive`], so a record read
//! off disk is a record composed by the same rules as one composed from a deck
//! — including what is still pending, which is the whole reason to come back.

use super::{compose_archive, ArchiveRecord};
use crate::types::{DeckMetadata, DeckSource};

/// The record a file holds, or nothing when the file is not one.
///
/// `slug` is the file's own name rather than anything inside it: the file name
/// is the address, and a record whose slug was re-derived from its title would
/// move the moment somebody edited that title — which is exactly the breakage
/// the pinned slug exists to prevent.
pub fn read_record(slug: &str, markdown: &str) -> Option<ArchiveRecord> {
    let mut meta = frontmatter(markdown)?;
    meta.slug = Some(slug.to_string());

    let composed = compose_archive(&DeckSource { meta, ..DeckSource::default() });

    composed.value().cloned()
}

/// The keys a record writes, read back into the metadata they came from.
///
/// Deliberately not a YAML parser. This file was written by
/// [`super::render_record`] and by nothing else, so the shapes it can contain
/// are known: one `key: "value"` per line, plus one `tags: [...]`. A general
/// parser here would accept files this crate never produces and would then have
/// to decide what they mean.
fn frontmatter(markdown: &str) -> Option<DeckMetadata> {
    let body = markdown.strip_prefix("---\n")?;
    let end = body.find("\n---")?;

    let mut meta = DeckMetadata::default();

    for line in body[..end].lines() {
        let Some((key, value)) = line.split_once(": ") else { continue };

        match key {
            "tags" => meta.tags = Some(list(value)),
            "title" => meta.title = scalar(value),
            "event" => meta.event = scalar(value),
            "date" => meta.date = scalar(value),
            "venue" => meta.venue = scalar(value),
            "author" => meta.author = scalar(value),
            "description" => meta.description = scalar(value),
            // The record calls it `deck` because that is what it is to a
            // reader; the deck calls it `url`.
            "deck" => meta.url = scalar(value),
            "recording" => meta.recording = scalar(value),
            "repo" => meta.repo = scalar(value),
            _ => {}
        }
    }

    meta.title.is_some().then_some(meta)
}

/// One quoted scalar, unescaped.
fn scalar(value: &str) -> Option<String> {
    let inner = value.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut text = String::with_capacity(inner.len());
    let mut escaped = false;

    for character in inner.chars() {
        match (escaped, character) {
            (false, '\\') => escaped = true,
            _ => {
                escaped = false;
                text.push(character);
            }
        }
    }

    Some(text)
}

/// `["a", "b"]`, as written.
fn list(value: &str) -> Vec<String> {
    let inner = value.trim().trim_start_matches('[').trim_end_matches(']');

    inner.split(',').filter_map(scalar).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(meta: DeckMetadata) -> ArchiveRecord {
        let source = DeckSource { meta, ..DeckSource::default() };

        compose_archive(&source).value().cloned().expect("a record")
    }

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

    #[test]
    fn a_record_written_and_read_back_is_the_same_record() {
        // The property the talk index rests on. A key that stopped being read
        // would show up as a column quietly missing from somebody's own
        // speaking page.
        let written = record(meta());

        assert_eq!(read_record(&written.slug, &written.markdown), Some(written));
    }

    #[test]
    fn a_record_with_only_a_title_still_reads_back() {
        let written = record(DeckMetadata {
            title: Some("Lightning talk".into()),
            ..DeckMetadata::default()
        });

        assert_eq!(read_record(&written.slug, &written.markdown), Some(written));
    }

    #[test]
    fn the_recording_added_months_later_reads_back_with_it() {
        // The one edit this format exists to survive.
        let written =
            record(DeckMetadata { recording: Some("https://youtu.be/abc123".into()), ..meta() });
        let read = read_record(&written.slug, &written.markdown).expect("a record");

        assert_eq!(read.recording.as_deref(), Some("https://youtu.be/abc123"));
        assert!(read.pending.is_empty());
    }

    #[test]
    fn a_title_carrying_quotes_survives_the_round_trip() {
        let written = record(DeckMetadata {
            title: Some(r#"Slides: a "talk" about talks"#.into()),
            ..meta()
        });
        let read = read_record(&written.slug, &written.markdown).expect("a record");

        assert_eq!(read.title, r#"Slides: a "talk" about talks"#);
    }

    #[test]
    fn the_file_name_is_the_slug_rather_than_anything_derived_from_the_title() {
        // The path is an address somebody has bookmarked. Re-deriving it from
        // an edited title would move a file that is linked to.
        let written = record(DeckMetadata { slug: Some("zero-js".into()), ..meta() });
        let read = read_record("zero-js", &written.markdown).expect("a record");

        assert_eq!(read.path, "talks/zero-js.md");
    }

    #[test]
    fn a_file_that_is_not_a_record_is_not_read_as_an_empty_one() {
        // A README in the archive directory is not a talk that happened.
        assert_eq!(read_record("readme", "# Talks\n\nSome notes.\n"), None);
        assert_eq!(read_record("empty", "---\nevent: \"SlidxConf\"\n---\n"), None);
    }
}
