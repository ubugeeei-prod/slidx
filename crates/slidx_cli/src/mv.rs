//! `slidx mv` — a project's new name, followed everywhere it is written down.
//!
//! ## Why this is not `mv`
//!
//! Because `mv` moves a directory and leaves the index pointing at where it
//! used to be. The entry then fails its liveness check, drops out of the picker,
//! and the deck somebody gave last spring is unfindable until they run something
//! inside it again — which they cannot do, because they were looking for it.
//!
//! So a rename is two writes that have to happen together: the directory, and
//! the index. That is the whole reason the command exists.
//!
//! ## And why `--title` is here
//!
//! A project renamed from `vueconf` to `vue-fes-2026` whose title slide still
//! says the old thing is half a rename, and the half that is left is the one an
//! audience sees. The retitle goes through [`slidx_edit`] like every other write
//! to a deck, so the frontmatter keeps the author's key order and quoting.

use std::fs;
use std::path::{Path, PathBuf};

use slidx_core::DeckParseOptions;
use slidx_edit::{apply, EditOp};

use crate::args::Matches;
use crate::find::scoring;
use crate::home::Home;
use crate::index::{Entry, Index};
use crate::lint::source;
use crate::project;
use crate::style::{Ink, Style};
use crate::Outcome;

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let (Some(query), Some(name)) = (matches.positional().first(), matches.positional().get(1))
    else {
        return Outcome::misuse(needs_two_names());
    };

    let home = Home::discover();
    let mut index = Index::load(&home.index());

    let Some(from) = resolve(query, &index) else {
        return Outcome::misuse(no_match(query));
    };

    let to = destination(&from, name);

    if let Err(message) = movable(&from, &to) {
        return Outcome::misuse(message);
    }

    if let Err(error) = fs::rename(&from, &to) {
        return Outcome::misuse(unmovable(&from, &to, &error));
    }

    // The index is the other half of the rename. Without this the deck is
    // unfindable until something is run inside it again, which is exactly what
    // somebody looking for it cannot do.
    let entry = index.entries().iter().find(|entry| entry.path == from).cloned();
    index.forget(&from);
    index.record(carried(entry, &to));
    let _ = index.save(&home.index());

    let retitled = matches.value("title").map(|title| retitle(&to, title));

    Outcome::out(report(&from, &to, retitled.as_ref(), style))
}

/// The project a query names: a path if it is one, otherwise the closest match.
///
/// A path first, because `slidx mv . vue-fes-2026` is the obvious thing to type
/// while standing in the project, and a fuzzy search over the index would be a
/// strange way to answer it.
fn resolve(query: &str, index: &Index) -> Option<PathBuf> {
    let given = PathBuf::from(query);
    if given.is_dir() {
        return given.canonicalize().ok().or(Some(given));
    }

    scoring::rank(query, index.entries(), Entry::haystack)
        .first()
        .map(|(entry, _)| entry.path.clone())
        .filter(|path| path.is_dir())
}

/// Where the project is going.
///
/// A bare name is a rename in place — a sibling of where the project already is,
/// which is what `mv` means to everybody. Anything with a separator in it is a
/// path, so a project can also move house.
fn destination(from: &Path, name: &str) -> PathBuf {
    let given = PathBuf::from(name);

    if given.is_absolute() || given.components().count() > 1 {
        return given;
    }

    from.parent().unwrap_or(Path::new(".")).join(name)
}

fn movable(from: &Path, to: &Path) -> Result<(), String> {
    if !from.is_dir() {
        return Err(format!("{} is not a directory slidx can move.\n", from.display()));
    }

    if to.exists() {
        return Err(format!(
            "{} already exists.\n\n\
             slidx will not move a project onto something else — a rename that merged two\n\
             talks would be impossible to unpick. Pick another name.\n",
            to.display()
        ));
    }

    Ok(())
}

/// The old entry, pointing at the new place.
///
/// Everything the index knew is carried across: the title, the event, and when
/// the deck was last seen. A fresh entry would tell `slidx list` this project was
/// touched now, which is true of the directory and false of the talk.
fn carried(entry: Option<Entry>, to: &Path) -> Entry {
    let Some(old) = entry else {
        return Entry::new(to);
    };

    Entry { path: to.to_path_buf(), ..old }
}

/// Writes the deck's own title, and says what happened either way.
fn retitle(project: &Path, title: &str) -> Result<PathBuf, String> {
    let deck = project::primary_deck(project)
        .ok_or_else(|| format!("no deck to retitle under {}", project.display()))?;

    let options = DeckParseOptions::default();
    let read = source::read(&deck, &options.separator)?;

    // A deck kept as one file per slide has its own frontmatter in the first
    // file, which is where the title is. That is the only file this can change.
    let target = read.files.first().cloned().unwrap_or_else(|| deck.clone());
    let source = fs::read_to_string(&target).map_err(|error| error.to_string())?;

    let edited = apply(
        &source,
        &options,
        &EditOp::SetField {
            slide: 0.into(),
            key: "title".to_string(),
            value: serde_json::json!(title.trim()),
        },
    )
    .map_err(|error| error.to_string())?;

    fs::write(&target, edited).map_err(|error| error.to_string())?;

    Ok(target)
}

fn report(
    from: &Path,
    to: &Path,
    retitled: Option<&Result<PathBuf, String>>,
    style: &Style,
) -> String {
    let mut text = format!(
        "  {}\n  {} {}\n",
        style.paint(Ink::Faint, from.display()),
        style.paint(Ink::Pass, "->"),
        style.paint(Ink::Strong, to.display())
    );

    text.push_str(&format!("  {}\n", style.paint(Ink::Faint, "the index followed it")));

    match retitled {
        Some(Ok(file)) => text.push_str(&format!(
            "  {}\n",
            style.paint(Ink::Faint, format!("retitled in {}", short(file)))
        )),
        // The directory moved and the title did not, which is worth a line
        // rather than a silent half-rename.
        Some(Err(reason)) => text.push_str(&format!(
            "  {}\n",
            style.paint(Ink::Warn, format!("moved, but not retitled: {reason}"))
        )),
        None => {}
    }

    text
}

fn short(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

fn needs_two_names() -> String {
    "`slidx mv` needs a deck and a new name.\n\n\
     \x20 slidx mv vueconf vue-fes-2026\n\
     \x20 slidx mv . ../talks/vue-fes-2026\n\n\
     The first is a query over the decks slidx has seen, or a path. A bare second\n\
     name renames the project where it is; a path moves it.\n"
        .to_string()
}

fn no_match(query: &str) -> String {
    format!(
        "No deck matches `{query}`, and it is not a directory.\n\n\
         `slidx list` shows every deck slidx has seen.\n"
    )
}

fn unmovable(from: &Path, to: &Path, error: &std::io::Error) -> String {
    format!(
        "Could not move {} to {}: {error}\n\n\
         Nothing was changed. A move between two disks is the usual cause, and `cp -a`\n\
         followed by `slidx rm` is the way to do that one.\n",
        from.display(),
        to.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::parse_deck;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("slidx-mv-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(path)
        }

        /// A project with a deck in it, laid out the conventional way.
        fn project(&self, name: &str, deck: &str) -> PathBuf {
            let root = self.0.join(name);
            fs::create_dir_all(root.join("slides")).expect("slides");
            fs::write(root.join("slides/0001.md"), deck).expect("write");

            root
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const DECK: &str =
        "---\ntitle: Making decks fast\nevent: VueConf\n---\n\n# Making decks fast\n";

    #[test]
    fn a_bare_name_renames_the_project_where_it_already_is() {
        // What `mv` means to everybody, and what somebody standing in
        // `~/talks/` means when they type one word.
        let from = Path::new("/home/somebody/talks/vueconf");

        assert_eq!(
            destination(from, "vue-fes-2026"),
            PathBuf::from("/home/somebody/talks/vue-fes-2026")
        );
    }

    #[test]
    fn a_name_with_a_path_in_it_moves_the_project_house() {
        let from = Path::new("/home/somebody/talks/vueconf");

        assert_eq!(destination(from, "../archive/vueconf"), PathBuf::from("../archive/vueconf"));
        assert_eq!(destination(from, "/elsewhere/vueconf"), PathBuf::from("/elsewhere/vueconf"));
    }

    #[test]
    fn a_destination_that_exists_is_refused_rather_than_merged() {
        // Two talks in one directory would be impossible to unpick, and the
        // author would find out from a deck with twice the slides.
        let scratch = Scratch::new("occupied");
        let from = scratch.project("vueconf", DECK);
        let to = scratch.project("vue-fes-2026", DECK);

        let message = movable(&from, &to).expect_err("refused");
        assert!(message.contains("already exists"), "{message}");
    }

    #[test]
    fn the_index_follows_the_project_rather_than_being_left_behind() {
        // The whole reason this is not `mv`. An entry pointing at where a
        // project used to be drops out of the picker, and the deck is
        // unfindable by the one search somebody would run for it.
        let mut index = Index::default();
        index.record(Entry::new("/talks/vueconf").seen_at(500));

        let entry = index.entries().first().cloned();
        index.forget(Path::new("/talks/vueconf"));
        index.record(carried(entry, Path::new("/talks/vue-fes-2026")));

        assert_eq!(index.len(), 1);
        assert_eq!(index.entries()[0].path, PathBuf::from("/talks/vue-fes-2026"));
    }

    #[test]
    fn a_renamed_project_keeps_when_the_talk_was_last_seen() {
        // The directory changed now; the talk did not. A fresh entry would send
        // a year-old deck to the top of the list.
        let mut entry = Entry::new("/talks/vueconf").seen_at(500);
        entry.title = Some("Making decks fast".into());

        let moved = carried(Some(entry), Path::new("/talks/vue-fes-2026"));

        assert_eq!(moved.last_seen, 500);
        assert_eq!(moved.title.as_deref(), Some("Making decks fast"));
    }

    #[test]
    fn a_project_slidx_has_never_seen_is_still_recorded_under_its_new_name() {
        let recorded = carried(None, Path::new("/talks/vue-fes-2026"));

        assert_eq!(recorded.path, PathBuf::from("/talks/vue-fes-2026"));
    }

    #[test]
    fn a_query_that_is_a_directory_is_taken_as_the_project_rather_than_searched_for() {
        // `slidx mv . vue-fes-2026` is the obvious thing to type while standing
        // in the project.
        let scratch = Scratch::new("path-query");
        let project = scratch.project("vueconf", DECK);

        let resolved = resolve(&project.display().to_string(), &Index::default());

        assert_eq!(resolved.map(|path| path.canonicalize().unwrap()), project.canonicalize().ok());
    }

    #[test]
    fn a_query_is_matched_against_the_index_the_way_every_other_command_matches_one() {
        let scratch = Scratch::new("query");
        let project = scratch.project("vueconf", DECK);
        let mut index = Index::default();
        index.record(Entry::new(&project));

        assert_eq!(resolve("vueconf", &index), Some(project));
        assert_eq!(resolve("nothing-like-it", &index), None);
    }

    #[test]
    fn a_retitle_writes_the_frontmatter_through_the_edit_crate() {
        // Which is what keeps the author's other keys, their order and their
        // quoting exactly as they were.
        let scratch = Scratch::new("retitle");
        let project = scratch.project("vueconf", DECK);

        let file = retitle(&project, "Vue Fes Japan 2026").expect("retitled");
        let source = fs::read_to_string(&file).expect("read");
        let deck = parse_deck(&source, &DeckParseOptions::default());

        assert_eq!(deck.meta.title.as_deref(), Some("Vue Fes Japan 2026"));
        assert!(source.contains("event: VueConf"), "{source}");
    }

    #[test]
    fn a_retitle_that_could_not_happen_is_said_out_loud_rather_than_swallowed() {
        // The directory moved and the title did not. Silence there is a
        // half-rename nobody knows about.
        let scratch = Scratch::new("no-deck");
        let empty = scratch.0.join("no-deck-here");
        fs::create_dir_all(&empty).expect("directory");

        let failed = retitle(&empty, "Anything");
        assert!(failed.is_err());

        let text = report(&empty, &empty, Some(&failed), &Style::plain());
        assert!(text.contains("not retitled"), "{text}");
    }

    #[test]
    fn the_report_says_both_halves_of_the_rename_happened() {
        let text = report(
            Path::new("/talks/vueconf"),
            Path::new("/talks/vue-fes-2026"),
            None,
            &Style::plain(),
        );

        assert!(text.contains("/talks/vueconf"), "{text}");
        assert!(text.contains("/talks/vue-fes-2026"), "{text}");
        assert!(text.contains("the index followed it"), "{text}");
    }

    #[test]
    fn one_name_is_not_enough_to_rename_with() {
        assert!(needs_two_names().contains("slidx mv vueconf vue-fes-2026"));
    }
}
