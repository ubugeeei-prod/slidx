//! # slidx
//!
//! The command line: the pre-flight a speaker runs in the room, and the linter
//! their CI runs.
//!
//! ## Why this is a Rust binary and not a Node CLI
//!
//! Everything else a user touches is JavaScript — the plugin, the runtime, the
//! wasm module — so a Node CLI would have been the obvious shape. Two things
//! rule it out.
//!
//! `doctor` reads the battery, the volume the deck sits on, the system clock
//! against a reference, the installed font families, and what is running that
//! could grab the screen. None of that is reachable from a WebAssembly sandbox,
//! and reaching it from Node would mean a native addon per platform — which is
//! precisely the platform matrix [`slidx_wasm`] exists to avoid for the parts
//! that do not need it.
//!
//! And `curl -fsSL … | sh` has to hand someone a working `slidx` on a machine
//! that has never had a Node toolchain installed. Somebody checking a room
//! twenty minutes before they speak should not be waiting on a package manager
//! to resolve a dependency tree over conference wifi.
//!
//! The pipeline stays where it is. Parsing, linting, theming and rendering are
//! one implementation in the workspace crates, reached from here and from wasm
//! alike, so the deck this binary lints is the deck the plugin builds.
//!
//! ## What it deliberately does not do
//!
//! There is no `slidx build`. Building a deck belongs to `@slidx/vite-plugin`,
//! and a second implementation of a pipeline is two answers to one question —
//! the worst thing that could happen to a tool whose whole promise is that the
//! artifact on the projector is the one that was checked. Typing it prints the
//! plugin's name rather than "unknown command"; see [`command::DECLINED`].
//!
//! ## Exit codes
//!
//! Three, and the distinction between the last two is the point:
//!
//! | code | meaning |
//! | ---- | ------- |
//! | 0 | nothing to act on |
//! | 1 | the check ran and found something |
//! | 2 | slidx could not run: bad arguments, or a deck it could not read |
//!
//! A mistyped flag must never look like a clean report. In CI the difference
//! between 1 and 2 is the difference between "your deck has a problem" and
//! "your deck was never checked".

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod args;
pub mod cd;
pub mod command;
pub mod completions;
pub mod dev;
pub mod doctor;
pub mod find;
pub mod fmt;
pub mod grep;
pub mod help;
pub mod home;
pub mod index;
pub mod lint;
pub mod list;
pub mod mcp;
pub mod preview;
pub mod project;
pub mod publish;
pub mod report;
pub mod sha256;
pub mod shell;
pub mod style;
pub mod terminal;
pub mod tui;
pub mod version;

use args::Invocation;
use style::Style;

/// What the process should print, and what it should exit with.
///
/// Returned rather than written, so every command is a function whose whole
/// output can be asserted on in a test. The alternative — printing as we go —
/// makes the interesting cases (a flat battery, an unreadable deck) reachable
/// only by running the binary on a machine that is in that state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub stdout: String,
    pub stderr: String,
    pub code: u8,
}

/// Nothing went wrong and there is nothing to act on.
pub const OK: u8 = 0;
/// The check ran and found something.
pub const FOUND: u8 = 1;
/// slidx could not run at all.
pub const MISUSE: u8 = 2;

impl Outcome {
    pub fn out(text: impl Into<String>) -> Self {
        Self { stdout: text.into(), ..Self::default() }
    }

    /// Something slidx could not do. Always exit 2 — see the crate docs.
    pub fn misuse(text: impl Into<String>) -> Self {
        Self { stderr: text.into(), code: MISUSE, ..Self::default() }
    }

    pub fn with_code(mut self, code: u8) -> Self {
        self.code = code;
        self
    }
}

/// Runs one command line.
///
/// Takes the arguments after the program name, and the styling the terminal
/// will accept. Both are injected so a test can state a command line and a
/// colour decision without owning the process.
pub fn run(argv: &[String], style: &Style) -> Outcome {
    match args::parse(argv) {
        Invocation::Help(None) => Outcome::out(help::root(style)),
        Invocation::Help(Some(route)) => Outcome::out(help::command(&route, style)),
        Invocation::Version => Outcome::out(format!("slidx {}\n", version())),
        // Keyed on the pair rather than the leaf name: two commands under
        // different parents may share one, and `list` alone says nothing.
        Invocation::Run(route, matches) => match route.key() {
            (None, "dev") => dev::run(&matches, style),
            (None, "doctor") => doctor::run(&matches, style),
            (None, "fmt") => fmt::run(&matches, style),
            (None, "lint") => lint::run(&matches, style),
            (None, "mcp") => mcp::run(&matches, style),
            (None, "open") => find::run(&matches, style),
            (None, "list") => list::run(&matches, style),
            (None, "cd") => cd::run(&matches, style),
            (None, "grep") => grep::run(&matches, style),
            (None, "preview") => preview::run(&matches, style),
            (None, "completions") => completions::run(&matches, style),
            (None, "shell") => shell::run(&matches, style),
            (None, "publish") => publish::run(&matches, style),
            (None, "tui") => tui::run(&matches, style),
            (Some("version"), action) => version::run(action, &matches, style),
            // Unreachable while the table and this match agree, which the
            // suite asserts. A panic here would be a crash in front of a room.
            _ => Outcome::misuse(format!("`{}` is declared but not wired up.\n", route.typed())),
        },
        Invocation::Declined { name, reason } => {
            Outcome::misuse(format!("slidx has no `{name}` command.\n\n{reason}\n"))
        }
        Invocation::Misuse(message) => Outcome::misuse(format!("{message}\n")),
    }
}

/// The version this binary was built from.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_line(line: &str) -> Outcome {
        let argv: Vec<String> = line.split_whitespace().map(String::from).collect();
        run(&argv, &Style::plain())
    }

    #[test]
    fn every_command_in_the_table_is_wired_to_something_that_runs_it() {
        // The one way the table and the dispatch can disagree. Reaching the
        // fallback arm in front of an audience is not an acceptable way to find
        // out that a command was declared and never connected.
        for command in command::ALL {
            let outcome = run_line(command.name);

            assert!(
                !outcome.stderr.contains("not wired up"),
                "{} is declared but not dispatched",
                command.name
            );
        }
    }

    #[test]
    fn the_version_is_printed_on_its_own_line_with_nothing_else() {
        // Scripts read this. A banner or a tip line would break every one of
        // them, quietly, on the release that added it.
        let outcome = run_line("--version");

        assert_eq!(outcome.stdout, format!("slidx {}\n", version()));
        assert_eq!(outcome.code, OK);
    }

    #[test]
    fn help_goes_to_stdout_and_exits_zero_because_it_was_asked_for() {
        let outcome = run_line("--help");

        assert!(outcome.stdout.contains("slidx"));
        assert!(outcome.stderr.is_empty());
        assert_eq!(outcome.code, OK);
    }

    #[test]
    fn a_misuse_goes_to_stderr_and_exits_two_rather_than_one() {
        // Exit 1 means "checked, and found something". A typo that exited 1
        // would be indistinguishable from a deck with a real problem.
        let outcome = run_line("lnit");

        assert!(outcome.stdout.is_empty());
        assert_eq!(outcome.code, MISUSE);
    }

    #[test]
    fn asking_for_a_build_prints_the_plugin_and_exits_two() {
        let outcome = run_line("build");

        assert!(outcome.stderr.contains("@slidx/vite-plugin"), "{}", outcome.stderr);
        assert_eq!(outcome.code, MISUSE);
    }
}
