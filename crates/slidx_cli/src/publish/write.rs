//! The half of publishing that needs no account.
//!
//! Four of the six destinations are files on the author's own disk: the blog
//! scaffold, the resources page, the archive record, and the index built from
//! every record beside it. Nothing about them needs a credential, so slidx does
//! them rather than describing them — a plan that only ever prints is a plan
//! somebody has to carry out by hand at the end of a long day, which is the
//! chore this whole milestone exists to remove.
//!
//! The other two need an account, and slidx does not have one. See
//! [`super::hand_off`] for what happens to those.
//!
//! ## Writing is not the same as overwriting
//!
//! Every path here is stable by construction — a blog draft is named for its
//! date and title, a record for its slug — so running the command twice writes
//! the same four files rather than four more. That is deliberate: the archive
//! target is *meant* to be run again months later when the recording appears,
//! and a second run that piled up `talk-1.md` would make the thing it exists
//! for unusable.

use std::fs;
use std::path::{Path, PathBuf};

use slidx_publish::{
    build_talk_index, read_record, ArchiveRecord, PublishStep, ReadyPayload, TalkIndexOptions,
};

/// A file that was written.
///
/// The path as it ended up, `--out` included, because that is the thing
/// somebody opens next and a report that named the target's own relative path
/// would send them to the wrong directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub path: PathBuf,
}

/// Writes what one step describes, if it is one of ours.
///
/// Returns nothing for a step that needs an account — those are not failures,
/// they are somebody else's half of the job.
pub fn perform(step: &PublishStep, out: &Path) -> Option<Result<Vec<Written>, String>> {
    let payload = step.payload()?;

    match payload {
        ReadyPayload::Blog(scaffold) => Some(write(out, &scaffold.path, &scaffold.markdown)),
        ReadyPayload::Resources(page) => Some(write(out, &page.path, &page.markdown)),
        // The record and the index are one write: an index that did not include
        // the record just written would be stale the moment it was produced.
        ReadyPayload::Archive(record) => Some(archive(out, record)),
        ReadyPayload::SpeakerDeck(_) | ReadyPayload::Docswell(_) | ReadyPayload::Social(_) => None,
    }
}

fn write(out: &Path, path: &str, contents: &str) -> Result<Vec<Written>, String> {
    Ok(vec![write_one(out, path, contents)?])
}

fn write_one(out: &Path, path: &str, contents: &str) -> Result<Written, String> {
    let target = out.join(path);

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| unwritable(parent, &error))?;
    }

    fs::write(&target, contents).map_err(|error| unwritable(&target, &error))?;

    Ok(Written { path: target })
}

/// The record, and then the index over every record beside it.
///
/// The index is rebuilt from the directory rather than accumulated in memory,
/// which is what makes it right for a talk given last spring: that record is
/// already on disk, and re-planning its deck to find out about it would mean
/// having every old deck checked out.
fn archive(out: &Path, record: &ArchiveRecord) -> Result<Vec<Written>, String> {
    let written = write_one(out, &record.path, &record.markdown)?;

    let directory = written.path.parent().map(Path::to_path_buf).unwrap_or_default();
    let index = build_talk_index(&records_in(&directory), &TalkIndexOptions::default());

    Ok(vec![written, write_one(out, &index.path, &index.markdown)?])
}

/// Every record in one directory, in file-name order.
///
/// Order does not survive into the page — the index sorts by date — but reading
/// in a fixed order does decide which of two talks on the same day comes first,
/// and a listing whose order came from the filesystem would put them either way
/// on different machines.
fn records_in(directory: &Path) -> Vec<ArchiveRecord> {
    let Ok(entries) = fs::read_dir(directory) else { return Vec::new() };

    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();

    files
        .iter()
        .filter_map(|path| {
            let slug = path.file_stem()?.to_str()?;
            read_record(slug, &fs::read_to_string(path).ok()?)
        })
        .collect()
}

/// Addressed to the person who typed the command, not to the kernel.
fn unwritable(path: &Path, error: &std::io::Error) -> String {
    format!(
        "Could not write {}: {error}\n\n\
         `slidx publish` writes the pages that need no account under the current\n\
         directory. Point it somewhere else with `--out <path>`, or read the plan\n\
         without writing anything with `--plan`.",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_publish::{plan_publish, DeckMetadata, DeckSlide, PlanOptions, PublishTarget};

    /// A scratch directory that cleans up after itself.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-publish-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn read(&self, path: &str) -> String {
            fs::read_to_string(self.0.join(path)).unwrap_or_default()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn meta(title: &str, date: &str) -> DeckMetadata {
        DeckMetadata {
            title: Some(title.into()),
            event: Some("SlidxConf 2026".into()),
            date: Some(date.into()),
            url: Some("https://slidx.dev/talks/zero-js".into()),
            ..DeckMetadata::default()
        }
    }

    fn plan(meta: DeckMetadata, targets: Vec<PublishTarget>) -> Vec<PublishStep> {
        plan_publish(&PlanOptions {
            meta,
            slides: vec![DeckSlide {
                index: 0,
                title: Some("Why plain HTML".into()),
                content: Some("[docs](https://slidx.dev/docs)".into()),
                notes: Some(vec!["A deck is a document.".into()]),
            }],
            targets: Some(targets),
            ..PlanOptions::default()
        })
        .steps
    }

    fn perform_all(steps: &[PublishStep], out: &Path) -> Vec<Written> {
        steps
            .iter()
            .filter_map(|step| perform(step, out))
            .flat_map(|result| result.expect("written"))
            .collect()
    }

    #[test]
    fn the_pages_that_need_no_account_are_written_rather_than_described() {
        let scratch = Scratch::new("pages");
        let steps = plan(
            meta("Zero-JavaScript Slides", "2026-07-29"),
            vec![PublishTarget::Blog, PublishTarget::Resources, PublishTarget::Archive],
        );

        perform_all(&steps, scratch.path());

        assert!(scratch
            .read("2026-07-29-zero-javascript-slides.md")
            .contains("A deck is a document."));
        assert!(scratch.read("resources.md").contains("https://slidx.dev/docs"));
        assert!(scratch.read("talks/zero-javascript-slides.md").contains("title:"));
    }

    #[test]
    fn a_destination_that_needs_an_account_is_not_something_this_module_answers() {
        // Not a failure: it is the other half of the job, and slidx does not
        // have a credential to do it with.
        let scratch = Scratch::new("account");
        let steps = plan(meta("Zero-JavaScript Slides", "2026-07-29"), vec![PublishTarget::Social]);

        assert!(perform(&steps[0], scratch.path()).is_none());
    }

    #[test]
    fn the_index_is_written_beside_the_record_rather_than_a_run_later() {
        // An index that did not include the record just written would be stale
        // the moment it was produced.
        let scratch = Scratch::new("index");
        let steps =
            plan(meta("Zero-JavaScript Slides", "2026-07-29"), vec![PublishTarget::Archive]);

        perform_all(&steps, scratch.path());

        assert!(scratch.read("talks/index.md").contains("Zero-JavaScript Slides"));
        assert!(scratch.read("talks/index.md").contains("## 2026"));
    }

    #[test]
    fn a_talk_recorded_last_spring_stays_in_the_index_when_another_is_added() {
        // The property the whole archive target exists for: the old record is
        // already on disk, and re-planning its deck to find out about it would
        // mean having every old deck checked out.
        let scratch = Scratch::new("accumulate");

        perform_all(
            &plan(meta("Spring talk", "2025-04-02"), vec![PublishTarget::Archive]),
            scratch.path(),
        );
        perform_all(
            &plan(meta("Summer talk", "2026-07-29"), vec![PublishTarget::Archive]),
            scratch.path(),
        );

        let index = scratch.read("talks/index.md");
        assert!(index.contains("Spring talk"), "{index}");
        assert!(index.contains("Summer talk"), "{index}");
        assert!(index.find("Summer talk") < index.find("Spring talk"), "{index}");
    }

    #[test]
    fn running_twice_writes_the_same_files_rather_than_more_of_them() {
        // The archive target is meant to be run again when the recording
        // appears, and a second run that piled up `talk-1.md` would make the
        // thing it exists for unusable.
        let scratch = Scratch::new("again");
        let steps =
            plan(meta("Zero-JavaScript Slides", "2026-07-29"), vec![PublishTarget::Archive]);

        let first = perform_all(&steps, scratch.path());
        let second = perform_all(&steps, scratch.path());

        assert_eq!(first, second);
        assert_eq!(fs::read_dir(scratch.path().join("talks")).unwrap().count(), 2);
    }

    #[test]
    fn a_file_in_the_archive_that_is_not_a_record_is_left_out_of_the_index() {
        let scratch = Scratch::new("stray");
        perform_all(
            &plan(meta("A talk", "2026-07-29"), vec![PublishTarget::Archive]),
            scratch.path(),
        );
        fs::write(scratch.path().join("talks/README.md"), "# Notes\n").expect("write");

        perform_all(
            &plan(meta("A talk", "2026-07-29"), vec![PublishTarget::Archive]),
            scratch.path(),
        );

        assert!(!scratch.read("talks/index.md").contains("Notes"));
    }

    #[test]
    fn a_directory_that_cannot_be_written_says_so_in_a_sentence_a_person_can_act_on() {
        // `--out` pointed at a regular file. Chosen over a path nobody has
        // permission to write because that is not the same path on every
        // machine: an absolute one is writable on Windows and as root, and a
        // test that only fails as an unprivileged Unix user asserts nothing on
        // the other two platforms CI runs.
        let scratch = Scratch::new("unwritable");
        let occupied = scratch.path().join("not-a-directory");
        fs::write(&occupied, "already a file\n").expect("write");

        let steps = plan(meta("A talk", "2026-07-29"), vec![PublishTarget::Resources]);
        let error = perform(&steps[0], &occupied).expect("ours to write").expect_err("unwritable");

        assert!(error.contains("--plan"), "{error}");
    }
}
