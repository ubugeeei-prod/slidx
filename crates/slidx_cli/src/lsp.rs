//! `slidx lsp` — the language server, on stdin and stdout.
//!
//! ## Why the language server is a subcommand
//!
//! Because an editor has to find it, and finding a binary is the part of an
//! editor plugin that actually fails. slidx installs into `$SLIDX_HOME`, else
//! `$XDG_DATA_HOME/slidx`, else `~/.slidx` — `%LOCALAPPDATA%\slidx` on Windows
//! — or comes from npm, or from `cargo install`, with a version manager that
//! may be in charge of which one runs. Every editor plugin slidx ships has to
//! walk that, and one answer is hard enough to keep true.
//!
//! A second binary would double it. `slidx-lsp` would need its own release
//! asset, its own entry on the PATH, its own line in the npm wrapper, and it
//! would sit outside `.slidx-version` entirely — so a deck pinned to one
//! version would be edited against whichever server happened to be installed,
//! and the diagnostics an author saw would come from a different slidx than the
//! build a room sees.
//!
//! ## It writes nothing to standard output
//!
//! Standard output is the protocol. A banner, a tip, or one colour code on it
//! desynchronises the frame stream, and the editor's only symptom is a server
//! that never answers — so this command returns an empty [`Outcome`] and
//! everything it has to say goes to standard error.
//!
//! ## What it does not take
//!
//! Arguments. An editor starts a server and configures it over the protocol,
//! which is what `initialize` is for, and a flag here would be a setting no
//! editor's client has a place to put. The one thing that looks like a setting
//! — which files are decks — is decided by the server; see [`slidx_lsp::deck`].

use std::io::IsTerminal;

use crate::args::Matches;
use crate::style::{Ink, Style};
use crate::{Outcome, MISUSE, OK};

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let _ = matches;

    // Typed at a prompt, which is the one way somebody meets this command by
    // accident. A server would sit there reading a terminal that is never going
    // to send it a frame, looking exactly like a hang.
    if std::io::stdin().is_terminal() {
        return Outcome { stderr: by_hand(style), code: MISUSE, ..Outcome::default() };
    }

    // Blocks until the client goes away, so nothing may be written before it
    // and nothing written after it is read by anybody.
    finished(slidx_lsp::stdio::serve())
}

/// What the end of a session means.
///
/// Separated so both endings are reachable from a test without a process and a
/// pair of pipes — the same shape [`crate::dev::finished`] has.
pub fn finished(asked_to_exit: bool) -> Outcome {
    if asked_to_exit {
        return Outcome::default().with_code(OK);
    }

    // The editor was killed, or crashed, or never spoke the protocol at all.
    // Nothing was checked, so this is a 2 rather than a 1.
    Outcome {
        stderr: "slidx lsp: the editor closed the connection without shutting down.\n".to_string(),
        code: MISUSE,
        ..Outcome::default()
    }
}

/// What somebody who typed this at a prompt needs to read.
fn by_hand(style: &Style) -> String {
    format!(
        "{}\n\n\
         This is the language server. It speaks the language server protocol on stdin\n\
         and stdout and is started by an editor, not by hand — run it here and it would\n\
         wait for a frame your terminal is never going to send.\n\n\
         The editors that know how to start it are in docs/content/editors.md. What\n\
         it serves — diagnostics, completion, the deck outline, hover, and formatting\n\
         — is also what `slidx lint` and `slidx fmt` report from the command line:\n\n\
         \x20 slidx lint\n\
         \x20 slidx fmt\n",
        style.paint(Ink::Strong, "slidx lsp")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::source::DEFAULT_DIR;

    #[test]
    fn the_directory_the_server_calls_a_deck_is_the_one_lint_falls_back_to() {
        // Two crates, one fact. `slides` is the plugin's default `srcDir`, the
        // path `slidx lint` takes when given none, and the only directory whose
        // Markdown an editor puts slidx findings on. A server that decided
        // otherwise would light up files no other command reads.
        assert_eq!(slidx_lsp::deck::DECK_DIRECTORY, DEFAULT_DIR);
    }

    #[test]
    fn a_slide_file_in_a_conventional_project_is_what_the_server_will_serve() {
        assert!(slidx_lsp::is_deck("file:///talks/vueconf/slides/0001.md"));
        assert!(!slidx_lsp::is_deck("file:///talks/vueconf/README.md"));
    }

    #[test]
    fn a_session_the_client_ended_properly_exits_zero() {
        assert_eq!(finished(true).code, OK);
        assert!(finished(true).stderr.is_empty());
    }

    #[test]
    fn a_client_that_vanished_exits_two_rather_than_one() {
        // Exit 1 means "checked, and found something". Nothing was checked.
        let outcome = finished(false);

        assert_eq!(outcome.code, MISUSE);
        assert!(outcome.stdout.is_empty(), "standard output is the protocol");
    }

    #[test]
    fn typing_it_at_a_prompt_says_what_it_is_and_names_what_to_run_instead() {
        // The one way somebody meets this command by accident, and a server
        // waiting on a terminal is indistinguishable from a hang.
        let message = by_hand(&Style::plain());

        assert!(message.contains("started by an editor"), "{message}");
        assert!(message.contains("docs/content/editors.md"), "{message}");
        assert!(message.contains("slidx lint"), "{message}");
    }
}
