//! `slidx cd` — the directory of a deck, resolved from a query.
//!
//! ## Why this prints instead of doing it
//!
//! **A child process cannot change the working directory of the shell that
//! started it.** `chdir` acts on the process that calls it; slidx is a separate
//! process the shell forked, and when it exits the shell is standing exactly
//! where it was. No flag fixes that, no privilege changes it, and every tool
//! that appears to do it — `z`, `autojump`, `zoxide` — is a shell function
//! wrapping a binary that prints a path.
//!
//! So the split is deliberate: this command *resolves*, a shell function
//! *enters*. Somebody reading it later will want to collapse the two, and this
//! comment is here to say what they will find when they try.
//!
//! ## One path, or none
//!
//! Everything about the output serves `cd "$(slidx cd vueconf)"`:
//!
//! - The path is the **only** thing on standard output. The picker draws on
//!   standard error and reads the terminal by name, so it still appears inside
//!   a command substitution — where standard output is a pipe by definition —
//!   and a substitution captures the answer rather than the interface.
//! - Several matches and no terminal to choose on takes the **best ranked**
//!   one and says which on standard error. Printing them all would hand `cd` a
//!   list, and failing would make the command unusable from a script for the
//!   sake of a scruple: matching is a subsequence search, so an unrelated
//!   project sharing three letters with the query is ordinary, and it ranks
//!   nowhere near the deck somebody meant.
//! - Nothing is printed when nothing matched, so `cd "$(slidx cd nonsense)"`
//!   fails loudly instead of entering the empty string, which is somebody's
//!   home directory.
//!
//! The path is not quoted or escaped. Quoting is the caller's job and only the
//! caller knows which shell it is in — a path escaped for bash is wrong in
//! PowerShell, and one escaped twice is wrong everywhere.

use std::path::Path;

use crate::args::Matches;
use crate::find::{self, picker, scoring};
use crate::home::Home;
use crate::index::{Entry, Index};
use crate::style::{Ink, Style};
use crate::{Outcome, FOUND};

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let home = Home::discover();
    let index = Index::load(&home.index()).pruned();

    // Cleaned while it is open: this is a read, so the stat has already been
    // paid for. See `crate::index` on why the write path never does it.
    let _ = index.save(&home.index());

    if index.is_empty() {
        return Outcome::misuse(find::nothing_indexed());
    }

    let query = matches.first_positional().unwrap_or_default();
    // The match travels with the entry, because the picker draws a highlight
    // from it — see `crate::find::highlight`.
    let matched: Vec<find::Hit> = scoring::rank(query, index.entries(), Entry::haystack)
        .into_iter()
        .map(|(entry, found)| find::Hit { entry, found })
        .collect();

    match matched.as_slice() {
        [] => Outcome { stderr: no_match(query), code: FOUND, ..Outcome::default() },
        [only] => entered(&only.entry.path),
        several => match picker::choose(several, query, style) {
            picker::Outcome::Chose(index) => entered(&several[index].entry.path),
            // Cancelled on purpose. Nothing on standard output, so the shell
            // stays where it was rather than going somewhere arbitrary.
            picker::Outcome::Cancelled => Outcome::default().with_code(FOUND),
            // No terminal to pick on: a pipe, a script, a CI job. The ranking
            // is the answer, and saying which one was taken costs a line of
            // standard error that no substitution will capture.
            picker::Outcome::Unavailable => Outcome {
                stderr: guessed(query, several, style),
                ..entered(&several[0].entry.path)
            },
        },
    }
}

/// The answer: one path and a newline, and nothing else at all.
fn entered(path: &Path) -> Outcome {
    Outcome::out(format!("{}\n", path.display()))
}

fn no_match(query: &str) -> String {
    format!(
        "No deck matches `{query}`.\n\n\
         `slidx list` shows every deck slidx has seen.\n"
    )
}

/// Which of several matches was taken, and that there were others.
///
/// On standard error, which is where it can be read by a person and not by a
/// command substitution. Silence here would be the one bad outcome: entering a
/// directory somebody did not choose *and* not saying so.
fn guessed(query: &str, entries: &[find::Hit<'_>], style: &Style) -> String {
    let chosen = entries[0].entry;
    let occasion = chosen.occasion().map(|text| format!(" — {text}")).unwrap_or_default();

    format!(
        "{}{occasion}, the closest of {} matches for `{query}`.\n",
        style.paint(Ink::Strong, chosen.label()),
        entries.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OK;
    use std::path::PathBuf;

    fn entry(path: &str, title: &str) -> Entry {
        let mut entry = Entry::new(path);
        entry.title = Some(title.to_string());
        entry
    }

    #[test]
    fn the_path_is_the_only_thing_on_standard_output() {
        // `cd "$(slidx cd vueconf)"` is the whole point of the command, and
        // anything else printed here ends up inside the substitution.
        let outcome = entered(Path::new("/home/somebody/talks/vueconf"));

        assert_eq!(outcome.stdout, "/home/somebody/talks/vueconf\n");
        assert!(outcome.stderr.is_empty());
        assert_eq!(outcome.code, OK);
    }

    #[test]
    fn a_directory_name_with_a_space_in_it_is_printed_whole_and_unescaped() {
        // The failure this command is most likely to have: a path split into
        // two arguments, or escaped for a shell that is not the one calling.
        // Quoting belongs to the caller, which is the only thing that knows
        // which shell it is.
        let outcome = entered(Path::new("/home/somebody/Vue Fes Japan 2026"));

        assert_eq!(outcome.stdout, "/home/somebody/Vue Fes Japan 2026\n");
        assert!(!outcome.stdout.contains('\\'));
        assert!(!outcome.stdout.contains('"'));
    }

    #[test]
    fn a_path_that_is_not_ascii_survives_byte_for_byte() {
        // A Japanese directory name is ordinary here. Anything that tried to
        // sanitise the path would break it, and a `cd` to a mangled path is a
        // failure somebody would blame on their shell.
        let outcome = entered(Path::new("/home/somebody/発表/Vue Fes 東京"));

        assert_eq!(outcome.stdout, "/home/somebody/発表/Vue Fes 東京\n");
    }

    #[test]
    fn the_output_is_one_line_so_a_shell_never_receives_two_paths() {
        assert_eq!(entered(Path::new("/talks/a")).stdout.lines().count(), 1);
    }

    #[test]
    fn nothing_matched_prints_nothing_at_all_on_standard_output() {
        // So `cd "$(slidx cd nonsense)"` fails rather than entering the empty
        // string, which is somebody's home directory.
        let outcome = Outcome { stderr: no_match("nonsense"), code: FOUND, ..Outcome::default() };

        assert!(outcome.stdout.is_empty());
        assert_eq!(outcome.code, FOUND);
        assert!(outcome.stderr.contains("nonsense"), "{}", outcome.stderr);
    }

    #[test]
    fn an_ambiguous_query_with_no_terminal_says_which_deck_it_took() {
        // Matching is a subsequence search, so an unrelated project sharing
        // three letters with the query is ordinary rather than exceptional.
        // Entering the best-ranked one is right; not saying so would not be.
        let entries = [
            entry("/talks/vueconf-tokyo", "Fast decks"),
            entry("/talks/vueconf-osaka", "Fast decks again"),
        ];
        let refs: Vec<find::Hit> = entries
            .iter()
            .map(|entry| find::Hit { entry, found: scoring::Match::default() })
            .collect();

        let stderr = guessed("vueconf", &refs, &Style::plain());

        assert!(stderr.contains("Fast decks"), "{stderr}");
        assert!(stderr.contains("2 matches"), "{stderr}");
        // On standard error, so it is never a path: one line there, and the
        // paths stay where a substitution reads them.
        assert_eq!(stderr.lines().count(), 1, "{stderr}");
        assert!(!stderr.contains("/talks/"), "{stderr}");
    }

    #[test]
    fn a_ranked_query_prefers_the_deck_whose_title_or_path_matches() {
        // The same ranking `slidx open` uses, so the two commands never
        // disagree about which deck a query means.
        let entries = vec![
            entry("/talks/vueconf", "Making decks fast"),
            entry("/work/review", "Architecture review"),
        ];
        let ranked = scoring::rank("vueconf", &entries, Entry::haystack);

        assert_eq!(ranked[0].0.path, PathBuf::from("/talks/vueconf"));
    }
}
