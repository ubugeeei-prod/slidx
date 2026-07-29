//! A parsed deck, as the planner takes it.
//!
//! [`slidx_core`] models a deck for rendering: the talk's fields live under
//! `talk`, and anything the parser has no field for stays in `raw`. The planner
//! wants them flat, and wants three keys the model does not name — `tags`,
//! `slug`, and `recording` — because they matter to publishing and to nothing
//! else. Lifting them here rather than adding them to `DeckMeta` keeps a field
//! that only one consumer reads out of the type every consumer loads.
//!
//! Artifacts are named, never produced. slidx does not build a deck — that is
//! `@slidx/vite-plugin`'s job — so a PDF is a path the author points at, and
//! its size is measured here so that planning never has to touch a disk.

use std::fs;
use std::path::{Path, PathBuf};

use slidx_core::Deck;
use slidx_publish::{Artifact, ArtifactKind, DeckMetadata, DeckSlide, DeckSource};

use crate::args::Matches;

/// Where `@slidx/vite-plugin` writes the PDF, at its defaults.
///
/// Looked for rather than required: a deck built without `pdf: true` simply has
/// no PDF, and the upload steps say so and say how to make one. A path given
/// with `--pdf` is a different claim — the author says it is there — so one
/// that is not there is an error rather than an absence.
pub const DEFAULT_PDF: &str = "dist/deck.pdf";

/// Where the plugin writes the deck's social card, once it has been rasterised.
pub const DEFAULT_CARD: &str = "dist/og.png";

/// Everything publishing reads, gathered from a deck and a command line.
pub fn source(deck: &Deck, matches: &Matches) -> Result<DeckSource, String> {
    Ok(DeckSource {
        meta: metadata(deck),
        slides: deck
            .slides
            .iter()
            .map(|slide| DeckSlide {
                index: slide.index,
                title: slide.title.clone(),
                content: Some(slide.content.clone()),
                notes: Some(slide.notes.clone()),
            })
            .collect(),
        artifacts: artifacts(matches)?,
    })
}

fn metadata(deck: &Deck) -> DeckMetadata {
    let meta = &deck.meta;

    DeckMetadata {
        title: meta.title.clone(),
        description: meta.description.clone(),
        author: meta.author.clone(),
        event: meta.talk.event.clone(),
        date: meta.talk.date.clone(),
        venue: meta.talk.venue.clone(),
        hashtag: meta.talk.hashtag.clone(),
        url: meta.talk.url.clone(),
        repo: meta.talk.repo.clone(),
        recording: string(deck, "recording"),
        slug: string(deck, "slug"),
        tags: strings(deck, "tags"),
    }
}

/// A frontmatter key the deck model does not have a field for.
fn string(deck: &Deck, key: &str) -> Option<String> {
    deck.meta.raw.get(key)?.as_str().map(str::to_string)
}

/// A frontmatter list.
///
/// A single string is read as a list of one, because `tags: rust` is what
/// somebody writes when they have one tag and it should mean what it looks
/// like.
fn strings(deck: &Deck, key: &str) -> Option<Vec<String>> {
    let value = deck.meta.raw.get(key)?;

    if let Some(one) = value.as_str() {
        return Some(vec![one.to_string()]);
    }

    Some(value.as_array()?.iter().filter_map(|entry| entry.as_str().map(str::to_string)).collect())
}

fn artifacts(matches: &Matches) -> Result<Vec<Artifact>, String> {
    let mut artifacts = Vec::new();

    for (kind, flag, default) in
        [(ArtifactKind::Pdf, "pdf", DEFAULT_PDF), (ArtifactKind::Card, "card", DEFAULT_CARD)]
    {
        match matches.value(flag) {
            Some(given) => artifacts.push(named(kind, flag, Path::new(given))?),
            None => artifacts.extend(found(kind, Path::new(default))),
        }
    }

    Ok(artifacts)
}

/// A path the author gave. Not being there is a mistake, not an absence.
fn named(kind: ArtifactKind, flag: &str, path: &Path) -> Result<Artifact, String> {
    match fs::metadata(path) {
        Ok(file) if file.is_file() => Ok(artifact(kind, path, Some(file.len()))),
        _ => Err(format!(
            "There is no file at {}.\n\n\
             `--{flag}` names a file the build already produced. slidx does not build a\n\
             deck — `vite build` does — so leave the flag out and slidx will look in\n\
             ./{DEFAULT_PDF}.",
            path.display()
        )),
    }
}

/// The build's own output, if the build produced it.
fn found(kind: ArtifactKind, path: &Path) -> Option<Artifact> {
    let file = fs::metadata(path).ok().filter(fs::Metadata::is_file)?;

    Some(artifact(kind, path, Some(file.len())))
}

/// The size is measured here so that planning never opens a file.
///
/// An upload rejected for being 4MB over the cap is a failure discovered at the
/// end of the slowest step in publishing, so the number is worth one `stat`.
fn artifact(kind: ArtifactKind, path: &Path, bytes: Option<u64>) -> Artifact {
    Artifact { kind, path: PathBuf::from(path).display().to_string(), bytes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn parse(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    fn matches_for(line: &str) -> Matches {
        let argv: Vec<String> =
            format!("publish {line}").split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    const DECK: &str = "---\n\
        title: Zero-JavaScript Slides\n\
        event: SlidxConf 2026\n\
        date: 2026-07-29\n\
        url: https://slidx.dev/talks/zero-js\n\
        tags: [rust, slides]\n\
        slug: zero-js\n\
        recording: https://youtu.be/abc123\n\
        ---\n\n\
        # One\n\n\
        <!-- notes: a deck is a document -->\n";

    #[test]
    fn the_talks_own_fields_are_flattened_out_of_the_deck_model() {
        let meta = metadata(&parse(DECK));

        assert_eq!(meta.title.as_deref(), Some("Zero-JavaScript Slides"));
        assert_eq!(meta.event.as_deref(), Some("SlidxConf 2026"));
        assert_eq!(meta.url.as_deref(), Some("https://slidx.dev/talks/zero-js"));
    }

    #[test]
    fn the_three_keys_only_publishing_reads_come_out_of_the_raw_frontmatter() {
        // They are not in `DeckMeta` because nothing else in the workspace
        // looks at them, and a field every consumer loads for one consumer's
        // sake is a field in the wrong place.
        let meta = metadata(&parse(DECK));

        assert_eq!(meta.slug.as_deref(), Some("zero-js"));
        assert_eq!(meta.recording.as_deref(), Some("https://youtu.be/abc123"));
        assert_eq!(meta.tags, Some(vec!["rust".to_string(), "slides".to_string()]));
    }

    #[test]
    fn one_tag_written_as_a_string_means_a_list_of_one() {
        let deck = parse("---\ntitle: A talk\ntags: rust\n---\n\n# One\n");

        assert_eq!(metadata(&deck).tags, Some(vec!["rust".to_string()]));
    }

    #[test]
    fn a_deck_that_names_none_of_them_reports_nothing_rather_than_empty_lists() {
        // An empty `tags: []` and no `tags:` at all are different sentences,
        // and the second is the one a deck with no tags is saying.
        let meta = metadata(&parse("# One\n"));

        assert_eq!(meta.tags, None);
        assert_eq!(meta.slug, None);
        assert_eq!(meta.recording, None);
    }

    #[test]
    fn the_slides_carry_their_notes_because_the_write_up_is_made_of_them() {
        let source = source(&parse(DECK), &matches_for("")).expect("a source");

        assert_eq!(source.slides[0].notes, Some(vec!["a deck is a document".to_string()]));
    }

    #[test]
    fn a_build_that_produced_nothing_offers_no_artifacts_rather_than_failing() {
        // The upload steps then report the missing PDF and say how to build
        // one, which is more use than refusing to plan at all.
        let source = source(&parse(DECK), &matches_for("")).expect("a source");

        assert!(source.artifacts.is_empty(), "{:?}", source.artifacts);
    }

    #[test]
    fn a_pdf_named_on_the_command_line_and_not_on_disk_is_a_mistake_rather_than_an_absence() {
        // The author said it is there. Planning around it silently would
        // report a missing PDF for a build that made one under another name.
        let error =
            source(&parse(DECK), &matches_for("--pdf /nowhere/deck.pdf")).expect_err("no file");

        assert!(error.contains("/nowhere/deck.pdf"), "{error}");
        assert!(error.contains("vite build"), "{error}");
    }
}
