//! The decks this machine knows about.
//!
//! A list of slidx projects, kept in `~/.slidx/index.json`, so a picker can
//! find the talk somebody gave last spring without them remembering where they
//! put it. Two properties decide whether that list is useful or landfill.
//!
//! ## It fills itself
//!
//! Every command that works on a deck records it. Nothing has to be registered,
//! added, or initialised — a tool nobody remembers to tell about a project
//! indexes nothing, and a picker over an empty index is worse than no picker
//! because somebody has to discover it is empty.
//!
//! Recording is best-effort in the strongest sense: a read-only home directory,
//! a full disk, a file somebody chmodded — none of it makes `slidx lint` fail.
//! The index is a convenience, and a convenience that can break the tool it is
//! attached to is not one.
//!
//! ## It prunes on read, not on write
//!
//! A directory that has been deleted or moved must be gone from the results the
//! next time they are read. A finder that offers paths which no longer exist
//! trains people to stop trusting it, and stopping trusting it is permanent.
//!
//! But statting every entry is the one expensive thing here, and the write path
//! runs on *every invocation*. So [`Index::record`] never touches the
//! filesystem, and [`Index::live`] — the read — filters as it goes. The file
//! itself is cleaned by [`Index::pruned`] whenever somebody actually browses,
//! which is the moment the cost is already being paid.
//!
//! ## What is worth storing
//!
//! Not just a path. `~/code/talks/vueconf/slides` and
//! `~/work/decks/internal/slides` are both "slides" in a picker, and a year
//! later neither name means anything. The deck's own title, its event and its
//! date come from frontmatter the author already wrote, and those are what make
//! an entry recognisable.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use slidx_core::Deck;

/// How many decks to remember.
///
/// A picker over five thousand entries is not a picker, and this file is read
/// on every invocation that records. Least recently seen is what falls off:
/// the deck somebody has not opened in two years is the one they will not miss,
/// and if they do open it, it comes straight back.
const CAPACITY: usize = 256;

/// One deck this machine has seen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// The project directory — where the deck lives, not the slides folder
    /// inside it. That is the path somebody wants to `cd` to.
    pub path: PathBuf,
    /// From the deck's frontmatter. What makes one `slides/` tellable from
    /// another in a list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Unix seconds. Orders the picker, and decides what falls off the end.
    pub last_seen: u64,
}

impl Entry {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), title: None, event: None, date: None, last_seen: now() }
    }

    /// Reads what a deck says about itself.
    pub fn describing(mut self, deck: &Deck) -> Self {
        self.title = deck.meta.title.clone();
        self.event = deck.meta.talk.event.clone();
        self.date = deck.meta.talk.date.clone();
        self
    }

    pub fn seen_at(mut self, seconds: u64) -> Self {
        self.last_seen = seconds;
        self
    }

    /// What to call this deck in a list.
    ///
    /// The title if it has one, the directory name otherwise — never an empty
    /// string and never a full path, both of which make a picker unreadable.
    pub fn label(&self) -> String {
        if let Some(title) = self.title.as_deref().filter(|title| !title.trim().is_empty()) {
            return title.to_string();
        }

        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }

    /// The event and date, for the second line of a picker row.
    pub fn occasion(&self) -> Option<String> {
        match (self.event.as_deref(), self.date.as_deref()) {
            (Some(event), Some(date)) => Some(format!("{event}, {date}")),
            (Some(event), None) => Some(event.to_string()),
            (None, Some(date)) => Some(date.to_string()),
            (None, None) => None,
        }
    }

    /// Everything a fuzzy search should match against.
    pub fn haystack(&self) -> String {
        let mut text = self.path.display().to_string();

        for extra in [self.title.as_deref(), self.event.as_deref()].into_iter().flatten() {
            text.push(' ');
            text.push_str(extra);
        }

        text
    }
}

/// Every deck, most recently seen first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Index {
    entries: Vec<Entry>,
}

impl Index {
    /// Reads the index, or an empty one.
    ///
    /// Never fails. A missing file is the state every machine starts in, and a
    /// truncated or hand-edited one is a cache that has gone bad — neither is a
    /// reason to stop somebody linting their deck. The bad file is replaced the
    /// next time anything is recorded.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Adds or refreshes one deck.
    ///
    /// Touches no filesystem beyond the eventual write: this runs on every
    /// invocation, and statting a few hundred directories to find out which
    /// still exist is the one thing here that would be felt.
    pub fn record(&mut self, entry: Entry) {
        self.entries.retain(|existing| existing.path != entry.path);
        self.entries.insert(0, entry);
        self.sort();
        self.entries.truncate(CAPACITY);
    }

    /// The decks that are still there, most recent first.
    ///
    /// The read path, and the only one that stats. An entry whose directory has
    /// been deleted or moved simply is not in the results.
    pub fn live(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|entry| entry.path.is_dir())
    }

    /// This index with everything that has gone away removed.
    ///
    /// For the moment somebody browses: the stat is already being paid for, so
    /// the file may as well be cleaned while it is open.
    pub fn pruned(self) -> Self {
        Self { entries: self.entries.into_iter().filter(|entry| entry.path.is_dir()).collect() }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Writes the index, creating the directory if it is missing.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, serde_json::to_string_pretty(self)?)
    }

    fn sort(&mut self) {
        self.entries.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
    }
}

/// Records one deck, and does not care whether it worked.
///
/// The whole point of the index is that nobody has to maintain it, which means
/// nobody should ever see it fail either. A read-only home directory is a
/// perfectly good reason for this to do nothing at all.
pub fn remember(index_path: &Path, entry: Entry) {
    let mut index = Index::load(index_path);
    index.record(entry);
    let _ = index.save(index_path);
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-index-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        fn dir(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(&path).expect("a directory");
            path
        }

        fn file(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn deck(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    #[test]
    fn a_deck_is_recorded_with_what_its_frontmatter_says_about_it() {
        // A path alone is unrecognisable a year later. Two projects both called
        // `slides` have to be tellable apart in a list.
        let deck =
            deck("---\ntitle: Making decks fast\nevent: VueConf\ndate: 2026-03-14\n---\n\n# One\n");
        let entry = Entry::new("/talks/vueconf").describing(&deck);

        assert_eq!(entry.title.as_deref(), Some("Making decks fast"));
        assert_eq!(entry.event.as_deref(), Some("VueConf"));
        assert_eq!(entry.date.as_deref(), Some("2026-03-14"));
    }

    #[test]
    fn a_deck_with_no_title_is_labelled_by_its_directory_rather_than_left_blank() {
        // A blank row in a picker is a row nobody can choose.
        assert_eq!(Entry::new("/talks/vueconf").label(), "vueconf");
    }

    #[test]
    fn a_title_that_is_only_whitespace_is_treated_as_no_title() {
        let mut entry = Entry::new("/talks/vueconf");
        entry.title = Some("   ".into());

        assert_eq!(entry.label(), "vueconf");
    }

    #[test]
    fn the_occasion_reads_as_a_sentence_whichever_half_is_present() {
        let mut entry = Entry::new("/talks/a");
        assert_eq!(entry.occasion(), None);

        entry.event = Some("VueConf".into());
        assert_eq!(entry.occasion().as_deref(), Some("VueConf"));

        entry.date = Some("2026-03-14".into());
        assert_eq!(entry.occasion().as_deref(), Some("VueConf, 2026-03-14"));
    }

    #[test]
    fn a_search_matches_the_path_the_title_and_the_event() {
        // People look for a talk by any of the three, and remember whichever
        // one they remember.
        let deck = deck("---\ntitle: Making decks fast\nevent: VueConf\n---\n\n# One\n");
        let haystack = Entry::new("/talks/vueconf").describing(&deck).haystack();

        assert!(haystack.contains("/talks/vueconf"));
        assert!(haystack.contains("Making decks fast"));
        assert!(haystack.contains("VueConf"));
    }

    #[test]
    fn recording_the_same_deck_twice_refreshes_it_rather_than_duplicating_it() {
        let mut index = Index::default();
        index.record(Entry::new("/talks/a").seen_at(100));
        index.record(Entry::new("/talks/a").seen_at(200));

        assert_eq!(index.len(), 1);
        assert_eq!(index.entries()[0].last_seen, 200);
    }

    #[test]
    fn the_most_recently_seen_deck_is_first() {
        // What somebody is looking for is almost always what they looked at
        // last, so the picker opens on it.
        let mut index = Index::default();
        index.record(Entry::new("/talks/old").seen_at(100));
        index.record(Entry::new("/talks/new").seen_at(300));
        index.record(Entry::new("/talks/middle").seen_at(200));

        let order: Vec<&Path> = index.entries().iter().map(|entry| entry.path.as_path()).collect();
        assert_eq!(
            order,
            [Path::new("/talks/new"), Path::new("/talks/middle"), Path::new("/talks/old")]
        );
    }

    #[test]
    fn the_least_recently_seen_deck_falls_off_the_end() {
        // A deck nobody has opened in two years is the one they will not miss,
        // and opening it brings it straight back.
        let mut index = Index::default();
        for seconds in 0..(CAPACITY as u64 + 10) {
            index.record(Entry::new(format!("/talks/{seconds}")).seen_at(seconds));
        }

        assert_eq!(index.len(), CAPACITY);
        assert!(index.entries().iter().all(|entry| entry.last_seen >= 10));
    }

    #[test]
    fn a_deck_whose_directory_has_gone_is_not_in_the_results() {
        // The property that decides whether anybody keeps using the picker.
        let scratch = Scratch::new("gone");
        let here = scratch.dir("here");

        let mut index = Index::default();
        index.record(Entry::new(&here));
        index.record(Entry::new(scratch.0.join("moved-away")));

        let live: Vec<&Path> = index.live().map(|entry| entry.path.as_path()).collect();
        assert_eq!(live, [here.as_path()]);
    }

    #[test]
    fn a_file_where_a_project_used_to_be_does_not_count_as_a_project() {
        let scratch = Scratch::new("file");
        let path = scratch.file("was-a-directory");
        fs::write(&path, "not a deck").expect("write");

        let mut index = Index::default();
        index.record(Entry::new(&path));

        assert_eq!(index.live().count(), 0);
    }

    #[test]
    fn recording_stats_nothing_so_the_write_path_stays_cheap_on_every_invocation() {
        // A path that does not exist is still recorded. The check happens on
        // read, where it is paid for once by somebody who is browsing.
        let mut index = Index::default();
        index.record(Entry::new("/nowhere/at/all"));

        assert_eq!(index.len(), 1);
        assert_eq!(index.live().count(), 0);
    }

    #[test]
    fn pruning_drops_the_dead_entries_from_the_file_itself() {
        let scratch = Scratch::new("prune");
        let here = scratch.dir("here");

        let mut index = Index::default();
        index.record(Entry::new(&here));
        index.record(Entry::new("/nowhere/at/all"));

        assert_eq!(index.pruned().len(), 1);
    }

    #[test]
    fn an_index_round_trips_through_the_file() {
        let scratch = Scratch::new("roundtrip");
        let path = scratch.file("index.json");

        let mut index = Index::default();
        index.record(Entry::new("/talks/a").seen_at(100));
        index.save(&path).expect("save");

        assert_eq!(Index::load(&path), index);
    }

    #[test]
    fn saving_creates_the_directory_rather_than_failing_on_a_fresh_machine() {
        // Nothing has made ~/.slidx yet on the first run.
        let scratch = Scratch::new("mkdir");
        let path = scratch.0.join("nested/deeper/index.json");

        Index::default().save(&path).expect("save");
        assert!(path.exists());
    }

    #[test]
    fn a_missing_index_reads_as_an_empty_one() {
        assert!(Index::load(Path::new("/nowhere/index.json")).is_empty());
    }

    #[test]
    fn a_corrupt_index_reads_as_an_empty_one_rather_than_failing_a_command() {
        // A cache that has gone bad is not a reason to stop somebody linting
        // their deck. It is replaced the next time anything is recorded.
        let scratch = Scratch::new("corrupt");
        let path = scratch.file("index.json");
        fs::write(&path, "{ this is not json").expect("write");

        assert!(Index::load(&path).is_empty());
    }

    #[test]
    fn remembering_a_deck_never_fails_however_unwritable_the_index_is() {
        // The index is a convenience, and a convenience that can break the tool
        // it is attached to is not one.
        remember(Path::new("/proc/nowhere/index.json"), Entry::new("/talks/a"));
    }

    #[test]
    fn remembering_a_deck_puts_it_in_the_file() {
        let scratch = Scratch::new("remember");
        let path = scratch.file("index.json");

        remember(&path, Entry::new("/talks/a"));
        remember(&path, Entry::new("/talks/b"));

        assert_eq!(Index::load(&path).len(), 2);
    }
}
