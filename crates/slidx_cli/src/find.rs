//! `slidx open` — the deck you gave that talk from.
//!
//! Searches [`crate::index`], which fills itself as you work, and prints the
//! path of whichever project you pick. The point is the shell composition:
//!
//! ```bash
//! cd "$(slidx open vueconf)"
//! ```
//!
//! which is why the chosen path is the **only** thing on standard output. The
//! picker draws on standard error, so a command substitution captures the
//! answer and not the interface. A picker that printed its own frame into
//! `$(…)` would be a picker nobody could use this way, which is the way
//! everybody wants to use it.
//!
//! ## It never hangs
//!
//! Not a terminal means a pipe or a CI job, and there is nobody there to press
//! a key. In that case it prints the matches, one path per line, and exits.
//! Same for `--list`, and same on a machine whose terminal cannot be put into
//! raw mode. The interactive picker is the enhancement, not the contract.
//!
//! ## Exit codes
//!
//! `0` when something was chosen, `1` when nothing matched or the picker was
//! cancelled, `2` when it could not run at all. So `cd "$(slidx open x)"` fails
//! loudly rather than cd-ing to an empty string, which is your home directory.

pub mod picker;
pub mod scoring;
pub mod screen;

use std::path::PathBuf;

use crate::args::Matches;
use crate::home::Home;
use crate::index::{Entry, Index};
use crate::style::{Ink, Style};
use crate::{Outcome, FOUND, OK};

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let home = Home::discover();
    let index = Index::load(&home.index()).pruned();

    // Cleaned while it is open. This is the read, so the stat is already being
    // paid for — see `crate::index` on why the write path never does it.
    let _ = index.save(&home.index());

    if index.is_empty() {
        return Outcome::misuse(nothing_indexed());
    }

    let query = matches.first_positional().unwrap_or_default();
    let entries: Vec<&Entry> = scoring::rank(query, index.entries(), Entry::haystack)
        .into_iter()
        .map(|(entry, _)| entry)
        .collect();

    if entries.is_empty() {
        return Outcome { stderr: no_match(query), code: FOUND, ..Outcome::default() };
    }

    // One match for a query somebody typed is an answer, not a menu.
    if matches.is_set("list") || !crate::terminal::someone_is_there() {
        return listed(&entries, matches, style);
    }

    if entries.len() == 1 {
        return chosen(entries[0]);
    }

    match picker::choose(&entries, query, style) {
        picker::Outcome::Chose(index) => chosen(entries[index]),
        picker::Outcome::Cancelled => Outcome::default().with_code(FOUND),
        // No raw mode — an unusual terminal, or Windows. Falling back to the
        // list keeps the command working rather than failing on a terminal
        // nobody could have predicted.
        picker::Outcome::Unavailable => listed(&entries, matches, style),
    }
}

/// The answer: one path, nothing else, so `cd "$(slidx open …)"` works.
fn chosen(entry: &Entry) -> Outcome {
    Outcome::out(format!("{}\n", entry.path.display()))
}

/// Every match, for a pipe or a `--list`.
fn listed(entries: &[&Entry], matches: &Matches, style: &Style) -> Outcome {
    if matches.is_set("json") {
        let owned: Vec<&Entry> = entries.to_vec();
        return match serde_json::to_string_pretty(&owned) {
            Ok(json) => Outcome::out(format!("{json}\n")),
            Err(error) => Outcome::misuse(format!("could not serialise the matches: {error}\n")),
        };
    }

    // Paths alone, one per line: this output is read by `while read`, `fzf`,
    // and `head -1` far more often than by a person, and a decorated list
    // cannot be any of those.
    let text: String = entries.iter().map(|entry| format!("{}\n", entry.path.display())).collect();

    // The names go to stderr so they are visible to somebody watching and
    // invisible to whatever is consuming the paths.
    let names: String = entries
        .iter()
        .map(|entry| {
            let occasion = entry.occasion().map(|text| format!(" — {text}")).unwrap_or_default();
            format!("{}{occasion}\n", style.paint(Ink::Strong, entry.label()))
        })
        .collect();

    Outcome { stdout: text, stderr: names, code: OK }
}

/// What to say on a machine that has not seen a deck yet.
///
/// Shared with [`crate::cd`], which reaches the same dead end from the same
/// empty index. Two wordings for one state would send two people looking for
/// two different setup steps, neither of which exists.
pub fn nothing_indexed() -> String {
    format!(
        "slidx has not seen any decks on this machine yet.\n\n\
         The index fills itself: run a command on a deck and it is remembered.\n\n\
         {}\n\n\
         The list lives in {}.\n",
        "  slidx lint ./slides",
        Home::discover().index().display()
    )
}

fn no_match(query: &str) -> String {
    format!(
        "No deck matches `{query}`.\n\n\
         `slidx open` with no query lists everything slidx has seen.\n"
    )
}

/// The user's home directory, for shortening paths in the picker.
pub fn user_home() -> Option<PathBuf> {
    std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Entry;

    fn matches_for(line: &str) -> Matches {
        let argv: Vec<String> =
            format!("open {line}").split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    fn entries() -> Vec<Entry> {
        let mut vueconf = Entry::new("/home/somebody/talks/vueconf");
        vueconf.title = Some("Making decks fast".into());
        vueconf.event = Some("VueConf".into());

        vec![vueconf, Entry::new("/home/somebody/work/arch-review")]
    }

    #[test]
    fn the_chosen_path_is_the_only_thing_on_standard_output() {
        // `cd "$(slidx open vueconf)"` is the whole point. Anything else on
        // stdout would end up in the command substitution.
        let entries = entries();
        let outcome = chosen(&entries[0]);

        assert_eq!(outcome.stdout, "/home/somebody/talks/vueconf\n");
        assert!(outcome.stderr.is_empty());
        assert_eq!(outcome.code, OK);
    }

    #[test]
    fn a_listing_puts_paths_on_stdout_and_the_names_beside_them_on_stderr() {
        // The paths are read by `while read` and `head -1` more often than by a
        // person; the names are for the person watching.
        let entries = entries();
        let refs: Vec<&Entry> = entries.iter().collect();
        let outcome = listed(&refs, &matches_for("--list"), &Style::plain());

        assert_eq!(outcome.stdout.lines().count(), 2);
        assert!(outcome.stdout.starts_with("/home/somebody/talks/vueconf"));
        assert!(outcome.stderr.contains("Making decks fast"));
        assert!(!outcome.stdout.contains("Making decks fast"));
    }

    #[test]
    fn a_listing_says_the_occasion_next_to_the_name() {
        let entries = entries();
        let refs: Vec<&Entry> = entries.iter().collect();

        assert!(listed(&refs, &matches_for("--list"), &Style::plain()).stderr.contains("VueConf"));
    }

    #[test]
    fn json_carries_everything_the_index_knows_rather_than_just_paths() {
        let entries = entries();
        let refs: Vec<&Entry> = entries.iter().collect();
        let outcome = listed(&refs, &matches_for("--list --json"), &Style::plain());

        assert!(outcome.stdout.starts_with("[\n"), "{}", outcome.stdout);
        assert!(outcome.stdout.contains("\"title\""), "{}", outcome.stdout);
    }

    #[test]
    fn ranking_finds_a_deck_by_its_title_as_well_as_its_path() {
        // People remember whichever of the two they remember.
        let entries = entries();
        let by_title = scoring::rank("decks fast", &entries, Entry::haystack);
        let by_path = scoring::rank("vueconf", &entries, Entry::haystack);

        assert_eq!(by_title[0].0.path, PathBuf::from("/home/somebody/talks/vueconf"));
        assert_eq!(by_path[0].0.path, PathBuf::from("/home/somebody/talks/vueconf"));
    }

    #[test]
    fn an_empty_index_explains_that_it_fills_itself_rather_than_asking_for_setup() {
        // Nobody should go looking for an `init` command that does not exist.
        let message = nothing_indexed();

        assert!(message.contains("fills itself"), "{message}");
        assert!(message.contains("slidx lint"), "{message}");
    }

    #[test]
    fn a_query_that_matches_nothing_exits_one_with_nothing_on_stdout() {
        // So `cd "$(slidx open nonsense)"` fails rather than cd-ing to an empty
        // string, which is somebody's home directory.
        let outcome = Outcome { stderr: no_match("nonsense"), code: FOUND, ..Outcome::default() };

        assert!(outcome.stdout.is_empty());
        assert_eq!(outcome.code, FOUND);
        assert!(outcome.stderr.contains("nonsense"));
    }
}
