//! `slidx save` — a commit about the talk, not about the lines.
//!
//! ## What this has that `git commit` does not
//!
//! A parser. That is the entire justification for the command: git compares
//! bytes and can tell you `+34 -6`, while slidx knows the file is a deck and can
//! say *two slides added, the demo retimed, notes written on the opening*. The
//! message writes itself because the information to write it exists here and
//! nowhere else. [`summary`] is where that happens.
//!
//! Anything less than that would make this a worse alias for `git commit`, and a
//! worse alias is a thing to delete rather than to ship.
//!
//! ## It commits the deck, and not your afternoon
//!
//! Only the deck's own paths go in. An author with something else half-staged
//! finds it still staged afterwards, because `git commit` with paths ignores the
//! index for everything else — see [`crate::git::Repo::commit`]. A command that
//! swept up whatever happened to be lying around would be one people stopped
//! typing without checking first, and then it would have no reason to exist.
//!
//! `--all` widens it to the whole project, which is a thing to ask for rather
//! than a thing to get.
//!
//! ## No repository is a state to help with, not to fail on
//!
//! A deck written this morning has no repository, and that is exactly when the
//! first commit matters most. So it offers to start one. Where there is nobody
//! to ask — a pipe, a CI job — it prints the two commands rather than guessing:
//! creating a repository somebody did not ask for is not a thing to do silently
//! in a directory you were pointed at.
//!
//! ## The message is the author's
//!
//! Nothing is appended to it. No trailer, no footer, no attribution, no mention
//! of slidx. `--message` overrules it entirely, and `--dry-run` prints what
//! would be written without writing anything.

pub mod message;
pub mod summary;

use std::path::{Path, PathBuf};

use slidx_core::{parse_deck, Deck, DeckParseOptions};

use crate::args::Matches;
use crate::git::{self, Repo};
use crate::lint::project_root;
use crate::lint::source::{self, DeckSource};
use crate::prompt::{self, Asked};
use crate::style::{Ink, Style};
use crate::{Outcome, OK};
use summary::Summary;

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let separator =
        matches.value("separator").map(str::to_string).unwrap_or_else(|| "---".to_string());
    let path = matches
        .first_positional()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source::DEFAULT_DIR));

    let deck_source = match source::read(&path, &separator) {
        Ok(read) => read,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    if !git::available() {
        return Outcome::misuse(no_git());
    }

    let root = project_root(&path).unwrap_or_else(|| PathBuf::from("."));
    let repo = match repository(&root, matches) {
        Ok(repo) => repo,
        Err(outcome) => return outcome,
    };

    // What the commit will contain, decided before anything is staged so the
    // report and the commit cannot disagree.
    let committed =
        if matches.is_set("all") { root.clone() } else { deck_path(&deck_source, &path) };

    let changes = match repo.changes(&committed) {
        Ok(changes) => changes,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    if changes.is_empty() {
        return Outcome::out(nothing_to_save(&committed, style)).with_code(OK);
    }

    let options = DeckParseOptions { separator: separator.clone(), ..DeckParseOptions::default() };
    let after = parse_deck(&deck_source.source, &options);
    let summary = match committed_deck(&repo, &deck_source, &path, &options) {
        Some(before) => Summary::of(&before, &after),
        None => Summary::first(&after),
    };

    let message = matches
        .value("message")
        .map(|given| format!("{}\n", given.trim_end()))
        .unwrap_or_else(|| summary.message());

    if matches.is_set("dry-run") {
        return Outcome::out(planned(&message, &changes, style)).with_code(OK);
    }

    if let Err(message) = repo.stage(&committed) {
        return Outcome::misuse(format!("{message}\n"));
    }

    if let Err(failed) = repo.commit(&message, std::slice::from_ref(&committed)) {
        return Outcome::misuse(format!("{failed}\n"));
    }

    Outcome::out(saved(&message, &changes, style)).with_code(OK)
}

/// The repository to commit into, starting one where that is what was asked.
fn repository(root: &Path, matches: &Matches) -> Result<Repo, Outcome> {
    if let Some(repo) = Repo::discover(root) {
        return Ok(repo);
    }

    if matches.is_set("init") {
        return Repo::init(root).map_err(|message| Outcome::misuse(format!("{message}\n")));
    }

    let asked =
        prompt::confirm(&format!("{} is not in a git repository. Start one here?", root.display()));

    if prompt::is_yes(&asked) {
        return Repo::init(root).map_err(|message| Outcome::misuse(format!("{message}\n")));
    }

    // Either they said no, or there was nobody to ask. Both mean nothing is
    // created: a repository somebody did not ask for is not a thing to make
    // silently in a directory you were pointed at.
    Err(Outcome::misuse(match asked {
        Asked::NoTerminal => no_repository(root),
        Asked::Said(_) => "Nothing was saved.\n".to_string(),
    }))
}

/// The paths a save is about: the deck, however the deck is kept.
fn deck_path(deck: &DeckSource, given: &Path) -> PathBuf {
    if deck.files.is_empty() {
        return given.to_path_buf();
    }

    // A directory of slide files. The directory rather than the files, so a
    // slide added since the last save is included without being listed.
    given.to_path_buf()
}

/// The deck as HEAD has it, joined the same way the one on disk was.
///
/// `None` when there is no commit to compare against, which is the deck's first
/// save. Two decks assembled by different rules would diff against themselves,
/// so the joining is [`source::join`] in both directions.
fn committed_deck(
    repo: &Repo,
    deck: &DeckSource,
    given: &Path,
    options: &DeckParseOptions,
) -> Option<Deck> {
    if !repo.has_commits() {
        return None;
    }

    let source = if deck.files.is_empty() {
        repo.committed(given)?
    } else {
        let files = repo.committed_files(given);
        if files.is_empty() {
            return None;
        }

        let sources: Vec<String> = files.iter().filter_map(|file| repo.committed(file)).collect();
        source::join(&sources, &options.separator)
    };

    Some(parse_deck(&source, options))
}

fn planned(message: &str, changes: &[git::Change], style: &Style) -> String {
    format!(
        "{}\n  {}\n",
        quoted_message(message, style),
        style.paint(
            Ink::Faint,
            format!("{} would be committed. Nothing was written.", counted(changes.len()))
        )
    )
}

fn saved(message: &str, changes: &[git::Change], style: &Style) -> String {
    format!(
        "{}\n  {}\n",
        quoted_message(message, style),
        style.paint(Ink::Faint, format!("{} committed.", counted(changes.len())))
    )
}

/// The message, indented so it reads as a quotation of what was written.
fn quoted_message(message: &str, style: &Style) -> String {
    message
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let ink = if index == 0 { Ink::Strong } else { Ink::Faint };
            format!("  {}\n", style.paint(ink, line))
        })
        .collect()
}

fn counted(files: usize) -> String {
    if files == 1 {
        "1 file".to_string()
    } else {
        format!("{files} files")
    }
}

fn nothing_to_save(path: &Path, style: &Style) -> String {
    format!(
        "  {}\n",
        style.paint(Ink::Pass, format!("Nothing has changed under {}.", path.display()))
    )
}

fn no_git() -> String {
    "`slidx save` needs git, and there is none on this machine.\n\n\
     The deck is yours either way — nothing about slidx depends on a repository. But\n\
     a talk is worth keeping the history of, and git is how.\n"
        .to_string()
}

fn no_repository(root: &Path) -> String {
    format!(
        "{} is not in a git repository, and there is no terminal here to ask on.\n\n\
         \x20 slidx save --init\n\n\
         or, the same thing by hand:\n\n\
         \x20 git init {}\n",
        root.display(),
        root.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    /// A project in a real repository.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-save-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("slides")).expect("scratch");

            let scratch = Self(path);
            for arguments in [
                vec!["init"],
                vec!["config", "user.email", "tests@slidx.invalid"],
                vec!["config", "user.name", "slidx tests"],
                vec!["config", "commit.gpgsign", "false"],
            ] {
                let _ = Command::new("git").current_dir(&scratch.0).args(&arguments).output();
            }

            scratch
        }

        fn slide(&self, name: &str, body: &str) {
            fs::write(self.0.join("slides").join(name), body).expect("write");
        }

        fn slides(&self) -> PathBuf {
            self.0.join("slides")
        }

        fn repo(&self) -> Repo {
            Repo::discover(&self.0).expect("a repository")
        }

        fn commit(&self, message: &str) {
            let repo = self.repo();
            repo.stage(&self.slides()).expect("stage");
            repo.commit(message, &[self.slides()]).expect("commit");
        }

        fn last_message(&self) -> String {
            let output = Command::new("git")
                .current_dir(&self.0)
                .args(["log", "-1", "--pretty=%B"])
                .output()
                .expect("git log");

            String::from_utf8_lossy(&output.stdout).trim_end().to_string()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn matches_for(line: &str) -> Matches {
        let argv: Vec<String> = line.split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    fn options() -> DeckParseOptions {
        DeckParseOptions::default()
    }

    /// Runs `slidx save` from inside a scratch project.
    ///
    /// The command takes the deck path as its positional, so nothing here has to
    /// change the process's working directory — which two tests running at once
    /// would fight over.
    fn save(scratch: &Scratch, line: &str) -> Outcome {
        let full = format!("save {} {line}", scratch.slides().display());
        let argv: Vec<String> = full.split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => run(&matches, &Style::plain()),
            other => panic!("expected a run, got {other:?}"),
        }
    }

    #[test]
    fn a_first_save_commits_the_deck_and_says_what_it_is() {
        if !git::available() {
            return;
        }

        let scratch = Scratch::new("first");
        scratch.slide("0001.md", "---\ntitle: Making decks fast\n---\n\n# Making decks fast\n");

        let outcome = save(&scratch, "");

        assert_eq!(outcome.code, OK, "{}", outcome.stderr);
        assert_eq!(scratch.last_message(), "Add the deck, 1 slide");
    }

    #[test]
    fn the_message_describes_the_deck_rather_than_the_diff() {
        if !git::available() {
            return;
        }

        // The reason the command exists. git would have said `+3 -0`.
        let scratch = Scratch::new("message");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n# A talk\n");
        scratch.commit("first");

        scratch.slide("0002.md", "# What goes wrong\n");
        save(&scratch, "");

        assert_eq!(scratch.last_message(), "Add \"What goes wrong\"");
    }

    #[test]
    fn nothing_is_appended_to_the_message_that_reaches_the_repository() {
        if !git::available() {
            return;
        }

        // Asserted against real git output rather than against the generator,
        // because a trailer added by a hook or by this command would look the
        // same to the author and only one of them is ours to prevent.
        let scratch = Scratch::new("trailers");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n# A talk\n");
        save(&scratch, "");

        let message = scratch.last_message();
        for forbidden in ["Co-authored-by", "Signed-off-by", "slidx", "Generated"] {
            assert!(!message.contains(forbidden), "{forbidden} appeared in:\n{message}");
        }
    }

    #[test]
    fn an_authors_own_message_is_used_exactly_as_given() {
        if !git::available() {
            return;
        }

        let scratch = Scratch::new("override");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n# A talk\n");

        save(&scratch, "--message the-slides-for-tuesday");

        assert_eq!(scratch.last_message(), "the-slides-for-tuesday");
    }

    #[test]
    fn only_the_deck_is_committed_and_a_half_staged_afternoon_is_left_alone() {
        if !git::available() {
            return;
        }

        // The property that makes this safe to type without checking first.
        let scratch = Scratch::new("only-deck");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n# A talk\n");
        fs::write(scratch.0.join("notes.txt"), "half-finished\n").expect("write");
        scratch.repo().stage(&scratch.0.join("notes.txt")).expect("stage");

        save(&scratch, "");

        let output = Command::new("git")
            .current_dir(&scratch.0)
            .args(["show", "--name-only", "--pretty=", "HEAD"])
            .output()
            .expect("git show");
        let files = String::from_utf8_lossy(&output.stdout);

        assert!(files.contains("slides/0001.md"), "{files}");
        assert!(!files.contains("notes.txt"), "{files}");
    }

    #[test]
    fn everything_in_the_project_goes_in_when_that_is_what_was_asked_for() {
        if !git::available() {
            return;
        }

        let scratch = Scratch::new("all");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n# A talk\n");
        fs::write(scratch.0.join("vite.config.ts"), "export default {};\n").expect("write");

        save(&scratch, "--all");

        let output = Command::new("git")
            .current_dir(&scratch.0)
            .args(["show", "--name-only", "--pretty=", "HEAD"])
            .output()
            .expect("git show");
        let files = String::from_utf8_lossy(&output.stdout);

        assert!(files.contains("vite.config.ts"), "{files}");
    }

    #[test]
    fn a_dry_run_prints_the_message_and_writes_nothing() {
        if !git::available() {
            return;
        }

        let scratch = Scratch::new("dry");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n# A talk\n");

        let outcome = save(&scratch, "--dry-run");

        assert!(outcome.stdout.contains("Add the deck"), "{}", outcome.stdout);
        assert!(outcome.stdout.contains("Nothing was written"), "{}", outcome.stdout);
        assert!(!scratch.repo().has_commits(), "a dry run made a commit");
    }

    #[test]
    fn a_deck_that_has_not_changed_makes_no_commit_and_says_so() {
        if !git::available() {
            return;
        }

        let scratch = Scratch::new("unchanged");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n# A talk\n");
        save(&scratch, "");
        let first = scratch.last_message();

        let outcome = save(&scratch, "");

        assert_eq!(outcome.code, OK);
        assert!(outcome.stdout.contains("Nothing has changed"), "{}", outcome.stdout);
        assert_eq!(scratch.last_message(), first, "a second commit was made");
    }

    #[test]
    fn a_deck_read_out_of_a_commit_is_joined_the_same_way_the_one_on_disk_is() {
        if !git::available() {
            return;
        }

        // Two decks assembled by different rules would diff against themselves,
        // and every save would report the whole deck as rewritten.
        let scratch = Scratch::new("joined");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n# One\n");
        scratch.slide("0002.md", "# Two\n");
        scratch.commit("first");

        let deck = source::read(&scratch.slides(), "---").expect("a deck");
        let before = committed_deck(&scratch.repo(), &deck, &scratch.slides(), &options())
            .expect("a committed deck");
        let after = parse_deck(&deck.source, &options());

        assert_eq!(before.slides.len(), after.slides.len());
        assert!(Summary::of(&before, &after).is_empty());
    }

    #[test]
    fn a_repository_with_no_commits_yet_is_the_decks_first_save() {
        if !git::available() {
            return;
        }

        let scratch = Scratch::new("no-head");
        scratch.slide("0001.md", "---\ntitle: A talk\n---\n\n# One\n");

        let deck = source::read(&scratch.slides(), "---").expect("a deck");

        assert!(committed_deck(&scratch.repo(), &deck, &scratch.slides(), &options()).is_none());
    }

    #[test]
    fn no_repository_and_nobody_to_ask_prints_the_two_commands_rather_than_making_one() {
        // Creating a repository somebody did not ask for, in a directory they
        // pointed at, is not a thing to do silently.
        let message = no_repository(Path::new("/talks/vueconf"));

        assert!(message.contains("slidx save --init"), "{message}");
        assert!(message.contains("git init /talks/vueconf"), "{message}");
    }

    #[test]
    fn a_machine_with_no_git_says_so_without_pretending_the_deck_is_at_risk() {
        let message = no_git();

        assert!(message.contains("needs git"), "{message}");
        assert!(message.contains("The deck is yours either way"), "{message}");
    }

    #[test]
    fn the_report_quotes_the_message_that_was_written() {
        let changes =
            vec![git::Change { status: "M ".into(), path: PathBuf::from("slides/0001.md") }];
        let text = saved("Add \"The fix\"\n\n- added \"The fix\"\n", &changes, &Style::plain());

        assert!(text.contains("Add \"The fix\""), "{text}");
        assert!(text.contains("1 file committed."), "{text}");
    }

    #[test]
    fn the_deck_a_save_is_about_is_the_path_it_was_given() {
        let single =
            DeckSource { label: "talk.md".into(), files: Vec::new(), source: String::new() };

        assert_eq!(deck_path(&single, Path::new("talk.md")), PathBuf::from("talk.md"));
    }

    #[test]
    fn a_message_given_on_the_command_line_keeps_its_own_line_ending() {
        let matches = matches_for("save --message hello");

        assert_eq!(matches.value("message"), Some("hello"));
    }
}
