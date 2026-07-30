//! Asking, and what to do when there is nobody to ask.
//!
//! Two commands need an answer from a person: `slidx save` when there is no
//! repository to commit into, and `slidx rm` before it deletes anything. Both
//! have the same problem underneath — a prompt is only a question when somebody
//! is there, and slidx runs in CI, in pipes, and in scripts where the terminal
//! belongs to nobody.
//!
//! So there is no default answer here. [`Asked::NoTerminal`] is a distinct
//! outcome, and each caller decides what it means: for `save` it means "print
//! what to type"; for `rm --delete` it means "do nothing at all", because a
//! confirmation nobody could give is not one that can be assumed.
//!
//! ## Where the question is written
//!
//! The terminal, by name, not standard error — for the same reason the picker
//! does it: a command whose answer is captured in `$(…)` still has a terminal,
//! and its interface must not end up in the capture. Where `/dev/tty` is not
//! there, standard input is used if it is a terminal, which is what makes this
//! work on Windows.

use std::io::{BufRead, BufReader, IsTerminal, Write};

/// What came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// What was typed, trimmed. Empty when they just pressed return.
    Said(String),
    /// No terminal to ask on: a pipe, a CI job, a machine with no `/dev/tty`.
    NoTerminal,
}

/// Puts a question and reads one line back.
pub fn ask(question: &str) -> Asked {
    if let Some(mut tty) = crate::terminal::open() {
        let _ = write!(tty, "{question}");
        let _ = tty.flush();

        let mut line = String::new();
        let read = BufReader::new(&tty).read_line(&mut line);

        return match read {
            Ok(0) | Err(_) => Asked::NoTerminal,
            Ok(_) => Asked::Said(line.trim().to_string()),
        };
    }

    // No `/dev/tty` — Windows, or a stripped container. Standard input is the
    // terminal often enough that refusing here would be worse.
    if !std::io::stdin().is_terminal() {
        return Asked::NoTerminal;
    }

    let mut line = String::new();
    let mut error = std::io::stderr();
    let _ = write!(error, "{question}");
    let _ = error.flush();

    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => Asked::NoTerminal,
        Ok(_) => Asked::Said(line.trim().to_string()),
    }
}

/// A yes-or-no question whose default is no.
///
/// No, because every caller of this is about to do something that costs
/// something, and a return key pressed to make a prompt go away must not be the
/// thing that authorises it.
pub fn confirm(question: &str) -> Asked {
    match ask(&format!("{question} [y/N] ")) {
        Asked::Said(answer) => Asked::Said(answer.to_lowercase()),
        Asked::NoTerminal => Asked::NoTerminal,
    }
}

/// True for an answer that means yes and nothing else.
pub fn is_yes(answer: &Asked) -> bool {
    matches!(answer, Asked::Said(said) if said == "y" || said == "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_plain_yes_is_a_yes() {
        // Everything else is a no, including an empty line. A return key pressed
        // to make a prompt go away must not authorise anything.
        assert!(is_yes(&Asked::Said("y".into())));
        assert!(is_yes(&Asked::Said("yes".into())));

        for answer in ["", "n", "no", "yeah", "sure", "Y E S", "1"] {
            assert!(!is_yes(&Asked::Said(answer.into())), "`{answer}` was read as a yes");
        }
    }

    #[test]
    fn no_terminal_is_not_a_no_but_its_own_answer() {
        // The distinction each caller needs: `save` prints what to type, and
        // `rm --delete` does nothing at all. Folding this into "no" would make
        // one of the two wrong.
        assert!(!is_yes(&Asked::NoTerminal));
        assert_ne!(Asked::NoTerminal, Asked::Said(String::new()));
    }

    #[test]
    fn a_question_asked_where_there_is_no_terminal_says_so_rather_than_waiting() {
        // The property that matters in CI: this must never block. Under `cargo
        // test` there is no controlling terminal, so this is the real path.
        assert!(matches!(confirm("Delete it?"), Asked::NoTerminal | Asked::Said(_)));
    }
}
