//! `slidx rm` — removing a project without destroying it.
//!
//! ## The default is reversible
//!
//! A deck is often the only copy of work that took weeks. It is written at
//! night, it is not always in a repository, and the repository it is in has
//! usually never been pushed. So this command **moves** a project into
//! [`archive`] under the slidx home and records where it came from; it does not
//! unlink anything, and `--restore` puts it back exactly where it was.
//!
//! A destructive default would be indefensible here. The cost of being wrong is
//! not symmetric: an archive somebody meant to delete is a directory taking up
//! space, and a delete somebody meant to archive is a talk that no longer
//! exists.
//!
//! ## Deleting, when that is genuinely what is wanted
//!
//! `--delete` is the flag, and it is not enough on its own:
//!
//! 1. The project's name has to be typed back. Not `y` — the name, because `y`
//!    is what a hand presses to make a prompt go away.
//! 2. **A project with uncommitted changes gets a second prompt of its own**,
//!    naming how many files hold work that is in no commit. That is precisely
//!    the case where the copy being deleted is the only copy: everything else
//!    could be recovered from git, and this could not.
//! 3. Where there is no terminal, it refuses. A confirmation nobody could give
//!    is not one that can be assumed, and a delete is not a thing to do on the
//!    strength of a flag in a script.
//!
//! ## Why the index matters here
//!
//! An archived project leaves the index in the same breath, which is what keeps
//! `slidx grep` from searching it and `slidx list` from offering a directory
//! that has moved. The archive is deliberately outside everything that reads the
//! index: filing work away must not be undone by a search that surfaces it.

pub mod archive;

use std::fs;
use std::path::Path;

use slidx_core::{parse_deck, DeckParseOptions};

use crate::args::Matches;
use crate::find::{self, scoring};
use crate::home::Home;
use crate::index::{Entry, Index};
use crate::lint::source;
use crate::project;
use crate::prompt::{self, Asked};
use crate::report;
use crate::style::{Ink, Style};
use crate::Outcome;
use archive::{Archive, Manifest};

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let home = Home::discover();
    let archive = Archive::in_home(&home);

    if matches.is_set("list") {
        return Outcome::out(listed(&archive, style));
    }

    let Some(query) = matches.first_positional() else {
        return Outcome::misuse(needs_a_deck(matches));
    };

    if matches.is_set("restore") {
        return restore(&archive, &home, query, style);
    }

    if matches.is_set("delete") {
        return delete(&archive, &home, query, style);
    }

    archive_project(&archive, &home, query, style)
}

/// The default: move it, record it, and say how to undo it.
fn archive_project(archive: &Archive, home: &Home, query: &str, style: &Style) -> Outcome {
    let mut index = Index::load(&home.index());

    let Some(project) = find::project(query, &index) else {
        return Outcome::misuse(no_match(query));
    };

    let (title, event) = describes(&project);
    let manifest = Manifest::of(&project, title, event);

    let entry = match archive.put(&project, manifest) {
        Ok(entry) => entry,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    // Out of the index in the same breath, so nothing searches a directory that
    // has moved and `slidx list` stops offering it.
    index.forget(&project);
    let _ = index.save(&home.index());

    Outcome::out(archived(&entry, style))
}

/// The inverse, which is the thing that makes the archive an archive.
fn restore(archive: &Archive, home: &Home, query: &str, style: &Style) -> Outcome {
    let entries = archive.entries();
    let ranked = scoring::rank(query, &entries, |entry| entry.manifest.haystack());

    let Some((entry, _)) = ranked.first() else {
        return Outcome::misuse(nothing_archived_like(query, &entries));
    };

    let manifest = entry.manifest.clone();
    let back = match archive.restore(entry) {
        Ok(back) => back,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    // Back in the index, keeping the time it was archived rather than now: the
    // talk was not touched today, and a restored deck should not push a deck
    // somebody is working on down the list.
    let (title, event) = describes(&back);
    let mut index = Index::load(&home.index());
    let mut restored = Entry::new(&back).seen_at(manifest.archived);
    restored.title = title.or(manifest.title);
    restored.event = event.or(manifest.event);
    index.record(restored);
    let _ = index.save(&home.index());

    Outcome::out(put_back(&back, style))
}

/// Real deletion, behind a flag, a name, and — where it matters — a second
/// question.
fn delete(archive: &Archive, home: &Home, query: &str, style: &Style) -> Outcome {
    let mut index = Index::load(&home.index());

    // A live project first, then the archive: `slidx rm --delete vueconf` after
    // archiving it is somebody emptying the archive, and refusing there would
    // leave them with `rm -rf` and no help at all.
    let (target, name, uncommitted) = match find::project(query, &index) {
        Some(project) => {
            let uncommitted = crate::git::Repo::discover(&project)
                .and_then(|repo| repo.changes(&project).ok())
                .map(|changes| changes.len())
                .unwrap_or(0);
            let name = file_name(&project);

            (project, name, uncommitted)
        }
        None => {
            let entries = archive.entries();
            let ranked = scoring::rank(query, &entries, |entry| entry.manifest.haystack());

            let Some((entry, _)) = ranked.first() else {
                return Outcome::misuse(no_match(query));
            };

            let uncommitted = entry.manifest.git.as_ref().map(|git| git.uncommitted).unwrap_or(0);

            (entry.path.clone(), entry.manifest.name.clone(), uncommitted)
        }
    };

    if uncommitted > 0 {
        // The case where the copy being deleted is the only copy. Everything
        // else could be recovered from git; this could not.
        let asked = prompt::confirm(&uncommitted_warning(&name, uncommitted));

        match asked {
            Asked::NoTerminal => return Outcome::misuse(no_terminal(&name)),
            _ if !prompt::is_yes(&asked) => return Outcome::misuse(kept(&name)),
            _ => {}
        }
    }

    match prompt::ask(&format!("Type `{name}` to delete it permanently: ")) {
        Asked::NoTerminal => return Outcome::misuse(no_terminal(&name)),
        Asked::Said(said) if said == name => {}
        // Anything else is a no. A name typed wrong is somebody who is not sure,
        // and being not sure is a reason to stop.
        Asked::Said(_) => return Outcome::misuse(kept(&name)),
    }

    if let Err(error) = fs::remove_dir_all(&target) {
        return Outcome::misuse(format!("Could not delete {}: {error}\n", target.display()));
    }

    index.forget(&target);
    let _ = index.save(&home.index());

    Outcome::out(deleted(&target, style))
}

/// What the deck says about itself, for the manifest and for the index.
fn describes(project: &Path) -> (Option<String>, Option<String>) {
    let Some(deck) = project::primary_deck(project) else {
        return (None, None);
    };

    let options = DeckParseOptions::default();
    let Ok(read) = source::read(&deck, &options.separator) else {
        return (None, None);
    };

    let deck = parse_deck(&read.source, &options);

    (deck.meta.title.clone(), deck.meta.talk.event.clone())
}

fn archived(entry: &archive::Entry, style: &Style) -> String {
    let mut text = format!(
        "  {}  {}\n",
        style.pad(Ink::Pass, "archived", 8),
        style.paint(Ink::Strong, entry.manifest.label())
    );

    text.push_str(&format!(
        "  {}  {}\n  {}  {}\n",
        style.pad(Ink::Faint, "from", 8),
        style.paint(Ink::Faint, entry.manifest.origin.display()),
        style.pad(Ink::Faint, "to", 8),
        style.paint(Ink::Faint, entry.path.display())
    ));

    // The undo, spelled out. An archive nobody knows how to reverse is a slower
    // delete.
    text.push_str(&format!(
        "\n  {}\n",
        style.paint(
            Ink::Strong,
            format!("slidx rm --restore {}", report::shell_arg(&entry.manifest.name))
        )
    ));

    text
}

fn put_back(project: &Path, style: &Style) -> String {
    format!(
        "  {}  {}\n",
        style.pad(Ink::Pass, "restored", 8),
        style.paint(Ink::Strong, project.display())
    )
}

fn deleted(path: &Path, style: &Style) -> String {
    format!(
        "  {}  {}\n",
        style.pad(Ink::Fail, "deleted", 8),
        style.paint(Ink::Strong, path.display())
    )
}

fn listed(archive: &Archive, style: &Style) -> String {
    let entries = archive.entries();

    if entries.is_empty() {
        return format!(
            "  {}\n",
            style.paint(
                Ink::Faint,
                format!(
                    "Nothing is archived. `slidx rm <deck>` puts one here, in {}.",
                    archive.root().display()
                )
            )
        );
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);

    let width =
        entries.iter().map(|entry| entry.manifest.label().chars().count()).max().unwrap_or(0);

    let mut text = String::new();
    for entry in &entries {
        let manifest = &entry.manifest;
        let held = if manifest.holds_uncommitted_work() {
            format!(
                "  {}",
                style.paint(
                    Ink::Warn,
                    format!(
                        "{} uncommitted",
                        manifest.git.as_ref().map(|git| git.uncommitted).unwrap_or(0)
                    )
                )
            )
        } else {
            String::new()
        };

        text.push_str(&format!(
            "  {}  {}  {}{held}\n",
            style.pad(Ink::Strong, &manifest.label(), width),
            style.pad(Ink::Faint, &project::ago(manifest.archived, now), 9),
            style.paint(Ink::Faint, manifest.origin.display())
        ));
    }

    text.push_str(&format!(
        "\n  {}\n",
        style.paint(Ink::Faint, "`slidx rm --restore <name>` puts one back where it was.")
    ));

    text
}

fn needs_a_deck(matches: &Matches) -> String {
    let verb = if matches.is_set("restore") { "restore" } else { "remove" };

    format!(
        "`slidx rm` needs a deck to {verb}.\n\n\
         \x20 slidx rm vueconf              archive it, reversibly\n\
         \x20 slidx rm --restore vueconf    put it back where it was\n\
         \x20 slidx rm --list               what is archived\n"
    )
}

fn no_match(query: &str) -> String {
    format!(
        "No deck matches `{query}`, and it is not a directory.\n\n\
         `slidx list` shows every deck slidx has seen.\n"
    )
}

fn nothing_archived_like(query: &str, entries: &[archive::Entry]) -> String {
    if entries.is_empty() {
        return "Nothing is archived.\n\n`slidx rm <deck>` is what puts a project there.\n"
            .to_string();
    }

    format!(
        "Nothing in the archive matches `{query}`.\n\n\
         `slidx rm --list` shows what is there.\n"
    )
}

fn uncommitted_warning(name: &str, files: usize) -> String {
    format!(
        "{name} has {files} {} that {} in no commit — deleting it is the only copy.\n\
         Archive it instead with `slidx rm {name}`, which is reversible.\n\n\
         Delete anyway?",
        if files == 1 { "file" } else { "files" },
        if files == 1 { "is" } else { "are" }
    )
}

fn no_terminal(name: &str) -> String {
    format!(
        "Deleting {name} needs a confirmation, and there is no terminal here to ask on.\n\n\
         Nothing was deleted. `slidx rm {name}` archives it instead, which needs no\n\
         confirmation because it can be undone.\n"
    )
}

fn kept(name: &str) -> String {
    format!("{name} was not deleted.\n")
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Entry as IndexEntry;
    use archive::GitState;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("slidx-rm-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        fn project(&self, name: &str, deck: &str) -> PathBuf {
            let root = self.0.join(name);
            fs::create_dir_all(root.join("slides")).expect("slides");
            fs::write(root.join("slides/0001.md"), deck).expect("write");

            root
        }

        fn home(&self) -> Home {
            Home::at(self.0.join("home"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const DECK: &str =
        "---\ntitle: Making decks fast\nevent: VueConf\n---\n\n# Making decks fast\n";

    fn index_over(home: &Home, project: &Path) -> Index {
        let mut index = Index::default();
        index.record(IndexEntry::new(project));
        let _ = index.save(&home.index());

        index
    }

    #[test]
    fn removing_a_project_archives_it_and_leaves_nothing_unlinked() {
        // The whole design. A deck is often the only copy of work that took
        // weeks, and a destructive default would eventually cost somebody a
        // talk.
        let scratch = Scratch::new("archive");
        let home = scratch.home();
        let project = scratch.project("vueconf", DECK);
        index_over(&home, &project);

        let archive = Archive::in_home(&home);
        let outcome = archive_project(&archive, &home, "vueconf", &Style::plain());

        assert_eq!(outcome.code, crate::OK, "{}", outcome.stderr);
        assert!(!project.exists());

        let entries = archive.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].manifest.origin, project);
        assert!(entries[0].project().join("slides/0001.md").is_file());
    }

    #[test]
    fn the_deck_is_described_in_the_manifest_so_a_year_later_it_is_recognisable() {
        let scratch = Scratch::new("described");
        let home = scratch.home();
        let project = scratch.project("vueconf", DECK);
        index_over(&home, &project);

        archive_project(&Archive::in_home(&home), &home, "vueconf", &Style::plain());
        let entries = Archive::in_home(&home).entries();

        assert_eq!(entries[0].manifest.title.as_deref(), Some("Making decks fast"));
        assert_eq!(entries[0].manifest.event.as_deref(), Some("VueConf"));
    }

    #[test]
    fn an_archived_project_leaves_the_index_in_the_same_breath() {
        // Otherwise `slidx list` offers a directory that has moved, and
        // `slidx grep` searches a path that is not there.
        let scratch = Scratch::new("index");
        let home = scratch.home();
        let project = scratch.project("vueconf", DECK);
        index_over(&home, &project);

        archive_project(&Archive::in_home(&home), &home, "vueconf", &Style::plain());

        assert!(Index::load(&home.index()).is_empty());
    }

    #[test]
    fn the_report_says_the_command_that_undoes_it() {
        // An archive nobody knows how to reverse is a slower delete.
        let scratch = Scratch::new("undo");
        let home = scratch.home();
        let project = scratch.project("vueconf", DECK);
        index_over(&home, &project);

        let outcome = archive_project(&Archive::in_home(&home), &home, "vueconf", &Style::plain());

        assert!(outcome.stdout.contains("slidx rm --restore vueconf"), "{}", outcome.stdout);
    }

    #[test]
    fn restoring_puts_the_project_back_where_it_came_from_and_into_the_index() {
        // The reversal has to be a real thing a person can do, or the archive is
        // a filing cabinet with no handle.
        let scratch = Scratch::new("restore");
        let home = scratch.home();
        let project = scratch.project("vueconf", DECK);
        index_over(&home, &project);
        let archive = Archive::in_home(&home);

        archive_project(&archive, &home, "vueconf", &Style::plain());
        let outcome = restore(&archive, &home, "vueconf", &Style::plain());

        assert_eq!(outcome.code, crate::OK, "{}", outcome.stderr);
        assert!(project.join("slides/0001.md").is_file());
        assert!(archive.entries().is_empty());

        let index = Index::load(&home.index());
        assert_eq!(index.entries().len(), 1);
        assert_eq!(index.entries()[0].path, project);
        assert_eq!(index.entries()[0].title.as_deref(), Some("Making decks fast"));
    }

    #[test]
    fn restoring_something_the_archive_does_not_have_says_what_it_does_have() {
        let scratch = Scratch::new("no-restore");
        let home = scratch.home();
        let archive = Archive::in_home(&home);

        let outcome = restore(&archive, &home, "nothing-like-it", &Style::plain());

        assert_eq!(outcome.code, crate::MISUSE);
        assert!(outcome.stderr.contains("Nothing is archived"), "{}", outcome.stderr);
    }

    #[test]
    fn a_delete_with_no_terminal_to_confirm_on_deletes_nothing() {
        // A confirmation nobody could give is not one that can be assumed, and
        // this is the path a script takes.
        let scratch = Scratch::new("no-terminal");
        let home = scratch.home();
        let project = scratch.project("vueconf", DECK);
        index_over(&home, &project);

        let outcome = delete(&Archive::in_home(&home), &home, "vueconf", &Style::plain());

        assert_eq!(outcome.code, crate::MISUSE);
        assert!(project.join("slides/0001.md").is_file(), "the project was deleted anyway");
        assert!(outcome.stderr.contains("no terminal"), "{}", outcome.stderr);
        assert!(outcome.stderr.contains("archives it instead"), "{}", outcome.stderr);
    }

    #[test]
    fn the_warning_about_uncommitted_work_says_how_much_and_offers_the_archive() {
        // The one case where the copy being deleted is the only copy.
        let message = uncommitted_warning("vueconf", 3);

        assert!(message.contains("3 files"), "{message}");
        assert!(message.contains("only copy"), "{message}");
        assert!(message.contains("slidx rm vueconf"), "{message}");

        assert!(
            uncommitted_warning("vueconf", 1).contains("1 file that is"),
            "{}",
            uncommitted_warning("vueconf", 1)
        );
    }

    #[test]
    fn a_listing_shows_where_each_project_came_from_and_when() {
        let scratch = Scratch::new("list");
        let home = scratch.home();
        let project = scratch.project("vueconf", DECK);
        index_over(&home, &project);
        let archive = Archive::in_home(&home);

        archive_project(&archive, &home, "vueconf", &Style::plain());
        let text = listed(&archive, &Style::plain());

        assert!(text.contains("Making decks fast"), "{text}");
        assert!(text.contains("vueconf"), "{text}");
        assert!(text.contains("--restore"), "{text}");
    }

    #[test]
    fn a_listing_marks_an_archived_project_that_holds_work_in_no_commit() {
        // The line that tells somebody browsing the archive which entries they
        // cannot rebuild from git.
        let scratch = Scratch::new("list-dirty");
        let archive = Archive::at(scratch.0.join("archive"));
        let project = scratch.project("vueconf", DECK);

        let mut manifest = Manifest::of(&project, Some("A talk".into()), None);
        manifest.git = Some(GitState { uncommitted: 4, ..GitState::default() });
        archive.put(&project, manifest).expect("archived");

        assert!(listed(&archive, &Style::plain()).contains("4 uncommitted"));
    }

    #[test]
    fn an_empty_archive_says_what_puts_something_in_it() {
        let scratch = Scratch::new("empty");
        let text = listed(&Archive::at(scratch.0.join("archive")), &Style::plain());

        assert!(text.contains("Nothing is archived"), "{text}");
        assert!(text.contains("slidx rm"), "{text}");
    }

    #[test]
    fn a_query_that_is_a_directory_is_taken_as_the_project_rather_than_searched_for() {
        let scratch = Scratch::new("path");
        let project = scratch.project("vueconf", DECK);

        let resolved = find::project(&project.display().to_string(), &Index::default());

        assert_eq!(resolved.map(|path| path.canonicalize().unwrap()), project.canonicalize().ok());
    }

    #[test]
    fn nothing_matching_is_a_misuse_rather_than_a_removal_of_the_nearest_thing() {
        let scratch = Scratch::new("no-match");
        let home = scratch.home();

        let outcome =
            archive_project(&Archive::in_home(&home), &home, "nothing-like-it", &Style::plain());

        assert_eq!(outcome.code, crate::MISUSE);
        assert!(outcome.stderr.contains("No deck matches"), "{}", outcome.stderr);
    }

    #[test]
    fn a_bare_command_says_all_three_things_it_can_do() {
        let message = needs_a_deck(&Matches::default());

        assert!(message.contains("archive it, reversibly"), "{message}");
        assert!(message.contains("--restore"), "{message}");
        assert!(message.contains("--list"), "{message}");
    }
}
