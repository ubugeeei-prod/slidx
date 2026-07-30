//! Where a removed project goes, and how it comes back.
//!
//! ## Why there is a store here at all
//!
//! Because a deck is often the only copy of work that took weeks. It is written
//! at night, it is not always in a repository, and the repository it is in has
//! usually never been pushed. `rm -rf` on that is unrecoverable, and a tool that
//! made it one command would be a tool that eventually cost somebody a talk.
//!
//! So removing a project **moves** it, and the move is recorded. Nothing is
//! unlinked, and the operation has an inverse that a person can actually run.
//!
//! ## The layout
//!
//! ```text
//! ~/.slidx/archive/
//!   20260730-064512-vueconf/
//!     archive.json     where it came from, when, and what git knew
//!     project/         the project, exactly as it was
//! ```
//!
//! The manifest sits *beside* the project rather than inside it, which is the
//! only arrangement where it cannot collide with a file the author wrote. A
//! project that already had an `archive.json` in it is not a thing to think
//! about twice.
//!
//! The directory name carries the time and the project's own name: the time so
//! the archive sorts oldest-first by name alone, and the name so a person
//! reading `ls` knows what they are looking at without opening anything.
//!
//! ## What the manifest is for
//!
//! Restoring needs one field — where it came from. Everything else is there for
//! the person deciding *whether* to restore, months later: what the deck was
//! called, what event it was for, and whether it had uncommitted changes when it
//! was put away. That last one is the difference between an archived project
//! that git could rebuild and one where this copy is the work.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::git::Repo;
use crate::home::Home;

/// The manifest's file name, beside the project rather than inside it.
pub const MANIFEST: &str = "archive.json";

/// What the project is called inside an archive entry.
pub const PROJECT: &str = "project";

/// What git knew about a project when it was archived.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Paths git reported as changed, staged or not, tracked or not.
    ///
    /// Not zero is the interesting case: it means the archived copy holds work
    /// that is in no commit, so this directory is the only place it exists.
    pub uncommitted: usize,
}

/// Where a project came from, and when it was put away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// The absolute path it was archived from. Restoring puts it back here.
    pub origin: PathBuf,
    /// Unix seconds.
    pub archived: u64,
    /// The directory name it had, which is what a person searches for.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<GitState>,
    /// The version that archived it, so a layout change later has something to
    /// key on rather than a guess.
    pub slidx: String,
}

impl Manifest {
    pub fn of(origin: &Path, title: Option<String>, event: Option<String>) -> Self {
        Self {
            origin: origin.to_path_buf(),
            archived: now(),
            name: name_of(origin),
            title,
            event,
            git: git_state(origin),
            slidx: crate::version().to_string(),
        }
    }

    /// What to call this in a list: the deck's title, or the directory's name.
    pub fn label(&self) -> String {
        self.title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or(&self.name)
            .to_string()
    }

    /// Everything a search should match against.
    pub fn haystack(&self) -> String {
        [Some(self.name.clone()), self.title.clone(), self.event.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// True when the archived copy holds work that is in no commit.
    pub fn holds_uncommitted_work(&self) -> bool {
        self.git.as_ref().is_some_and(|git| git.uncommitted > 0)
    }
}

/// One project in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The entry directory, which holds the manifest and the project.
    pub path: PathBuf,
    pub manifest: Manifest,
}

impl Entry {
    /// The project itself, inside the entry.
    pub fn project(&self) -> PathBuf {
        self.path.join(PROJECT)
    }
}

/// The archive under a slidx home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    root: PathBuf,
}

impl Archive {
    pub fn in_home(home: &Home) -> Self {
        Self { root: home.root().join("archive") }
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Everything in the archive, oldest first.
    ///
    /// An entry whose manifest cannot be read is skipped rather than reported:
    /// the archive is a directory people can reach into, and a stray folder in
    /// it must not stop somebody restoring the project beside it.
    pub fn entries(&self) -> Vec<Entry> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };

        let mut found: Vec<Entry> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .filter_map(|path| {
                let text = fs::read_to_string(path.join(MANIFEST)).ok()?;
                let manifest: Manifest = serde_json::from_str(&text).ok()?;

                Some(Entry { path, manifest })
            })
            .collect();

        found.sort_by_key(|entry| entry.manifest.archived);
        found
    }

    /// Moves a project in, and records where it came from.
    ///
    /// The manifest is written **after** the project is in place, so an entry
    /// that has one is an entry that is complete. A crash between the two leaves
    /// a directory the listing ignores rather than a manifest pointing at
    /// nothing.
    pub fn put(&self, project: &Path, manifest: Manifest) -> Result<Entry, String> {
        let entry = self.free_path(&manifest);

        fs::create_dir_all(&entry)
            .map_err(|error| format!("could not make {}: {error}", entry.display()))?;

        relocate(project, &entry.join(PROJECT))?;

        let text = serde_json::to_string_pretty(&manifest)
            .map_err(|error| format!("could not describe the archived project: {error}"))?;
        fs::write(entry.join(MANIFEST), format!("{text}\n"))
            .map_err(|error| format!("could not write the manifest: {error}"))?;

        Ok(Entry { path: entry, manifest })
    }

    /// Puts a project back where it came from, and empties the entry.
    ///
    /// The destination is checked first: a project restored on top of something
    /// else would merge two talks, and no manifest could unpick that afterwards.
    pub fn restore(&self, entry: &Entry) -> Result<PathBuf, String> {
        let origin = &entry.manifest.origin;

        if origin.exists() {
            return Err(format!(
                "{} is there again, so slidx will not restore on top of it.",
                origin.display()
            ));
        }

        if let Some(parent) = origin.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("could not make {}: {error}", parent.display()))?;
        }

        relocate(&entry.project(), origin)?;

        // Only the manifest is left. Removing the entry is what makes the
        // archive a record of what is *in* it rather than of everything that
        // ever passed through.
        let _ = fs::remove_dir_all(&entry.path);

        Ok(origin.clone())
    }

    /// An entry directory that does not exist yet.
    ///
    /// Two projects archived in the same second is unlikely and not impossible —
    /// a script, a loop — and the second one must not land inside the first.
    fn free_path(&self, manifest: &Manifest) -> PathBuf {
        let base = format!("{}-{}", stamp(manifest.archived), manifest.name);
        let mut candidate = self.root.join(&base);
        let mut suffix = 2;

        while candidate.exists() {
            candidate = self.root.join(format!("{base}-{suffix}"));
            suffix += 1;
        }

        candidate
    }
}

/// Moves a directory, copying it where a rename cannot cross the filesystem.
///
/// A deck on an external disk and a home directory on the internal one is an
/// ordinary arrangement, and `rename` refuses across that boundary. The copy is
/// verified before the original is removed — never the other way round, because
/// a copy that fails halfway must not have already deleted its source.
fn relocate(from: &Path, to: &Path) -> Result<(), String> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }

    if let Err(error) = copy_tree(from, to) {
        // Whatever was copied is incomplete and points nowhere. Leaving it
        // would put a half-project in the archive that a listing would offer.
        let _ = fs::remove_dir_all(to);
        return Err(error);
    }

    fs::remove_dir_all(from)
        .map_err(|error| format!("copied {}, and could not remove it: {error}", from.display()))
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| format!("could not make {}: {error}", to.display()))?;

    let entries = fs::read_dir(from)
        .map_err(|error| format!("could not read {}: {error}", from.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("could not read {}: {error}", from.display()))?;
        let source = entry.path();
        let target = to.join(entry.file_name());

        // Symlinks are followed rather than recreated. A deck linking a shared
        // title slide into several talks is a reasonable thing to have done, and
        // an archived copy has to still hold the slide.
        if source.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            fs::copy(&source, &target)
                .map_err(|error| format!("could not copy {}: {error}", source.display()))?;
        }
    }

    Ok(())
}

fn git_state(project: &Path) -> Option<GitState> {
    let repo = Repo::discover(project)?;

    Some(GitState {
        head: repo.head(),
        branch: repo.branch(),
        uncommitted: repo.changes(project).map(|changes| changes.len()).unwrap_or(0),
    })
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| {
        // A path with no last component is `/` or `.`, which somebody reached
        // by archiving the directory they were standing in.
        "project".to_string()
    })
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or(0)
}

/// `20260730-064512`, in UTC.
///
/// UTC rather than local time, because the name is what the archive sorts by and
/// a local timestamp changes meaning when the machine crosses a zone — two
/// entries a minute apart could then sort an hour the wrong way round.
pub fn stamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    let rest = seconds % 86_400;

    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// The civil date a count of days since the Unix epoch lands on.
///
/// Howard Hinnant's `civil_from_days`, which is the standard closed-form
/// conversion: it treats March as the first month so a leap day falls at the end
/// of a year, and is exact for every date this will ever see. Written out rather
/// than pulled in, because a date crate in a binary people are asked to pipe
/// into a shell is a dependency for one filename.
fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 { shifted } else { shifted - 146_096 } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 { month_prime + 3 } else { month_prime - 9 };

    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-archive-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        /// A project with a deck and a nested asset directory in it.
        fn project(&self, name: &str) -> PathBuf {
            let root = self.0.join(name);
            fs::create_dir_all(root.join("slides/images")).expect("directories");
            fs::write(root.join("slides/0001.md"), "# One\n").expect("write");
            fs::write(root.join("slides/images/diagram.png"), "not really a png").expect("write");

            root
        }

        fn archive(&self) -> Archive {
            Archive::at(self.0.join("archive"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn manifest(origin: &Path) -> Manifest {
        Manifest { archived: 1_785_389_775, ..Manifest::of(origin, Some("A talk".into()), None) }
    }

    #[test]
    fn archiving_moves_the_project_rather_than_unlinking_it() {
        // The property the whole command exists for. A deck is often the only
        // copy of work that took weeks.
        let scratch = Scratch::new("move");
        let project = scratch.project("vueconf");
        let archive = scratch.archive();

        let entry = archive.put(&project, manifest(&project)).expect("archived");

        assert!(!project.exists(), "the project was left behind");
        assert!(entry.project().join("slides/0001.md").is_file());
        assert_eq!(
            fs::read_to_string(entry.project().join("slides/0001.md")).expect("read"),
            "# One\n"
        );
    }

    #[test]
    fn everything_under_the_project_comes_with_it_including_the_assets() {
        // A deck is not only Markdown. An archive holding the slides and not the
        // images is one that restores a broken talk.
        let scratch = Scratch::new("assets");
        let project = scratch.project("vueconf");

        let entry = scratch.archive().put(&project, manifest(&project)).expect("archived");

        assert!(entry.project().join("slides/images/diagram.png").is_file());
    }

    #[test]
    fn the_manifest_records_where_the_project_came_from() {
        // The one field restoring needs. Everything else is for the person
        // deciding months later whether to restore at all.
        let scratch = Scratch::new("manifest");
        let project = scratch.project("vueconf");

        let entry = scratch.archive().put(&project, manifest(&project)).expect("archived");
        let text = fs::read_to_string(entry.path.join(MANIFEST)).expect("read");
        let read: Manifest = serde_json::from_str(&text).expect("json");

        assert_eq!(read.origin, project);
        assert_eq!(read.name, "vueconf");
        assert_eq!(read.title.as_deref(), Some("A talk"));
        assert_eq!(read.slidx, crate::version());
    }

    #[test]
    fn the_manifest_sits_beside_the_project_rather_than_inside_it() {
        // Inside, it could collide with a file the author wrote, and a project
        // that already had an `archive.json` is not a thing to think about
        // twice.
        let scratch = Scratch::new("beside");
        let project = scratch.project("vueconf");

        let entry = scratch.archive().put(&project, manifest(&project)).expect("archived");

        assert!(entry.path.join(MANIFEST).is_file());
        assert!(!entry.project().join(MANIFEST).exists());
    }

    #[test]
    fn a_restored_project_is_where_it_was_with_everything_in_it() {
        // The reversal has to be real, or the archive is a slower delete.
        let scratch = Scratch::new("restore");
        let project = scratch.project("vueconf");
        let archive = scratch.archive();

        let entry = archive.put(&project, manifest(&project)).expect("archived");
        let back = archive.restore(&entry).expect("restored");

        assert_eq!(back, project);
        assert_eq!(fs::read_to_string(project.join("slides/0001.md")).expect("read"), "# One\n");
        assert!(project.join("slides/images/diagram.png").is_file());
        assert!(!entry.path.exists(), "the archive entry was left behind");
    }

    #[test]
    fn restoring_onto_something_that_is_there_again_is_refused() {
        // Two talks merged into one directory could not be unpicked afterwards,
        // and no manifest would help.
        let scratch = Scratch::new("occupied");
        let project = scratch.project("vueconf");
        let archive = scratch.archive();

        let entry = archive.put(&project, manifest(&project)).expect("archived");
        fs::create_dir_all(&project).expect("a new project in its place");

        let message = archive.restore(&entry).expect_err("refused");

        assert!(message.contains("will not restore on top of it"), "{message}");
        assert!(entry.project().exists(), "the archived copy was lost anyway");
    }

    #[test]
    fn a_project_restored_into_a_parent_that_is_gone_gets_its_parent_back() {
        // `~/talks` can be gone by the time somebody restores, and refusing
        // there would be refusing over a directory anybody could make.
        let scratch = Scratch::new("parent");
        let project = scratch.project("nested/deeper/vueconf");
        let archive = scratch.archive();

        let entry = archive.put(&project, manifest(&project)).expect("archived");
        fs::remove_dir_all(scratch.0.join("nested")).expect("remove the parent");

        assert!(archive.restore(&entry).is_ok());
        assert!(project.join("slides/0001.md").is_file());
    }

    #[test]
    fn the_archive_lists_what_is_in_it_oldest_first() {
        let scratch = Scratch::new("list");
        let archive = scratch.archive();

        for (name, at) in [("older", 100), ("newer", 200)] {
            let project = scratch.project(name);
            let manifest = Manifest { archived: at, ..manifest(&project) };
            archive.put(&project, manifest).expect("archived");
        }

        let names: Vec<String> =
            archive.entries().into_iter().map(|entry| entry.manifest.name).collect();

        assert_eq!(names, ["older", "newer"]);
    }

    #[test]
    fn a_directory_in_the_archive_with_no_manifest_is_ignored_rather_than_fatal() {
        // The archive is a directory people can reach into. A stray folder must
        // not stop somebody restoring the project beside it.
        let scratch = Scratch::new("stray");
        let project = scratch.project("vueconf");
        let archive = scratch.archive();

        archive.put(&project, manifest(&project)).expect("archived");
        fs::create_dir_all(archive.root().join("something-somebody-dropped-here"))
            .expect("a stray directory");

        assert_eq!(archive.entries().len(), 1);
    }

    #[test]
    fn two_projects_archived_in_the_same_second_do_not_land_on_each_other() {
        let scratch = Scratch::new("collision");
        let archive = scratch.archive();

        let first = scratch.project("vueconf");
        let first_entry = archive.put(&first, manifest(&first)).expect("archived");

        // The same name and the same second, which a script in a loop produces.
        let second = scratch.project("vueconf");
        let second_entry = archive.put(&second, manifest(&second)).expect("archived");

        assert_ne!(first_entry.path, second_entry.path);
        assert_eq!(archive.entries().len(), 2);
    }

    #[test]
    fn an_archived_project_with_uncommitted_work_says_so_in_its_manifest() {
        // The field that decides whether this copy is the only copy, months
        // later, for somebody deciding whether to keep it.
        let mut manifest = manifest(Path::new("/talks/vueconf"));

        manifest.git = Some(GitState { uncommitted: 3, ..GitState::default() });
        assert!(manifest.holds_uncommitted_work());

        manifest.git = Some(GitState::default());
        assert!(!manifest.holds_uncommitted_work());

        manifest.git = None;
        assert!(!manifest.holds_uncommitted_work());
    }

    #[test]
    fn an_entry_is_named_by_the_deck_rather_than_by_the_directory_where_it_can_be() {
        let mut manifest = manifest(Path::new("/talks/vueconf"));
        assert_eq!(manifest.label(), "A talk");

        manifest.title = None;
        assert_eq!(manifest.label(), "vueconf");

        manifest.title = Some("   ".into());
        assert_eq!(manifest.label(), "vueconf");
    }

    #[test]
    fn a_search_matches_the_directory_name_the_title_and_the_event() {
        let mut manifest = manifest(Path::new("/talks/vueconf"));
        manifest.event = Some("Vue Fes".into());

        let haystack = manifest.haystack();

        assert!(haystack.contains("vueconf"));
        assert!(haystack.contains("A talk"));
        assert!(haystack.contains("Vue Fes"));
    }

    #[test]
    fn a_timestamp_reads_as_a_date_somebody_can_recognise() {
        // The archive is browsed with `ls` as often as with slidx, and
        // `1785389775-vueconf` tells nobody anything.
        assert_eq!(stamp(0), "19700101-000000");
        assert_eq!(stamp(1_785_389_775), "20260730-053615");
        // A leap day, which is where a hand-rolled conversion goes wrong.
        assert_eq!(stamp(1_709_208_000), "20240229-120000");
    }

    #[test]
    fn the_time_in_a_name_sorts_the_archive_oldest_first() {
        let earlier = stamp(1_709_208_000);
        let later = stamp(1_785_389_775);

        assert!(earlier < later, "{earlier} should sort before {later}");
    }

    #[test]
    fn an_archive_that_has_never_been_written_to_is_empty_rather_than_an_error() {
        assert!(Archive::at("/nowhere/at/all/archive").entries().is_empty());
    }

    #[test]
    fn a_copy_that_fails_leaves_the_original_where_it_was() {
        // The order that matters: never remove the source until the copy is
        // complete. A half-copy plus a deleted original is the failure this
        // whole module exists to prevent.
        let scratch = Scratch::new("failed-copy");
        let project = scratch.project("vueconf");

        // A destination that cannot be created: a file already occupies the path.
        let blocked = scratch.0.join("blocked");
        fs::write(&blocked, "in the way").expect("write");

        assert!(copy_tree(&project, &blocked.join("inside")).is_err());
        assert!(project.join("slides/0001.md").is_file());
    }
}
