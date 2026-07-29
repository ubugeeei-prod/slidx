//! Turning a command line into something typed.
//!
//! Hand-rolled rather than delegated to an argument parser, for the same reason
//! the binary exists at all: `curl | sh` should hand someone one file that
//! starts instantly, and every dependency here is bytes on that download and
//! another crate in the supply chain of a tool people are asked to pipe into a
//! shell. The grammar is small — subcommand, flags, one optional path — and a
//! small grammar is cheaper to write than to justify a dependency for.
//!
//! Nothing here knows which flags exist. The shapes come from
//! [`crate::command`], so a flag that parses is a flag that is documented.
//!
//! ## What a misuse costs
//!
//! A mistyped flag exits 2, never 0 and never 1. In a CI job the difference is
//! everything: `slidx lint --strcit` that exited 0 would report a clean deck
//! that was never linted.

use crate::command::{self, Command, Flag, ROOT};

/// What the command line asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// `slidx` with nothing, `--help`, or `slidx <command> --help`.
    Help(Option<&'static Command>),
    Version,
    Run(&'static Command, Matches),
    /// A command slidx deliberately does not have. Carries the redirect.
    Declined {
        name: String,
        reason: &'static str,
    },
    /// Nothing could be run. The string is addressed to the person who typed it.
    Misuse(String),
}

/// The flags and positionals one command was given.
///
/// Values stay as strings: the table knows a flag takes *a* value, and what
/// that value means is the command's business. Pushing types in here would put
/// path handling and theme names in the parser.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Matches {
    flags: Vec<(&'static str, Option<String>)>,
    positional: Vec<String>,
}

impl Matches {
    pub fn is_set(&self, long: &str) -> bool {
        self.flags.iter().any(|(name, _)| *name == long)
    }

    /// The last value given for a flag. Last rather than first, so a wrapper
    /// script's default can be overridden by appending to its command line.
    pub fn value(&self, long: &str) -> Option<&str> {
        self.flags
            .iter()
            .rev()
            .find(|(name, _)| *name == long)
            .and_then(|(_, value)| value.as_deref())
    }

    /// Every value given for a repeatable flag, in the order they were written.
    pub fn values<'a>(&'a self, long: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.flags
            .iter()
            .filter(move |(name, _)| *name == long)
            .filter_map(|(_, value)| value.as_deref())
    }

    pub fn positional(&self) -> &[String] {
        &self.positional
    }

    pub fn first_positional(&self) -> Option<&str> {
        self.positional.first().map(String::as_str)
    }
}

/// Parses the arguments after the program name.
pub fn parse(argv: &[String]) -> Invocation {
    let mut rest = argv.iter();

    let Some(first) = rest.next() else {
        return Invocation::Help(None);
    };

    // Root flags are answered before a subcommand is looked for, so `slidx -V`
    // does not have to be spelled `slidx version`.
    if let Some(token) = long_or_short(first) {
        return match root_flag(token) {
            Some(flag) if flag.long == "version" => Invocation::Version,
            Some(_) => Invocation::Help(None),
            None => Invocation::Misuse(unknown_root_flag(first)),
        };
    }

    let Some(found) = command::find(first) else {
        return match command::declined(first) {
            Some(reason) => Invocation::Declined { name: first.clone(), reason },
            None => Invocation::Misuse(unknown_command(first)),
        };
    };

    match collect(found, rest.as_slice()) {
        Ok(matches) if matches.is_set("help") => Invocation::Help(Some(found)),
        Ok(matches) => Invocation::Run(found, matches),
        Err(message) => Invocation::Misuse(message),
    }
}

/// Walks one command's arguments against its own flag table.
fn collect(command: &'static Command, argv: &[String]) -> Result<Matches, String> {
    let mut matches = Matches::default();
    let mut index = 0;

    while index < argv.len() {
        let argument = &argv[index];
        index += 1;

        // Everything after a bare `--` is a positional, so a deck directory
        // whose name starts with a dash is still reachable.
        if argument == "--" {
            matches.positional.extend(argv[index..].iter().cloned());
            break;
        }

        let Some(token) = long_or_short(argument) else {
            matches.positional.push(argument.clone());
            continue;
        };

        // `--flag=value` is split here rather than in the lookup, so the table
        // never has to know about the two spellings.
        let (name, inline) = match token.split_once('=') {
            Some((name, value)) => (name, Some(value.to_string())),
            None => (token, None),
        };

        let flag = command.flag(name).ok_or_else(|| unknown_flag(command, argument))?;

        if !flag.takes_value() {
            if inline.is_some() {
                return Err(format!(
                    "--{} is a switch and takes no value.\n\nTry: slidx {} --{}",
                    flag.long, command.name, flag.long
                ));
            }
            matches.flags.push((flag.long, None));
            continue;
        }

        let value = match inline {
            Some(value) => value,
            None => {
                let next = argv.get(index).ok_or_else(|| missing_value(command, flag))?;
                index += 1;
                next.clone()
            }
        };

        if !flag.repeatable && matches.is_set(flag.long) {
            // Not an error — the last one wins, so a wrapper script can set a
            // default and a caller can override it by appending.
            matches.flags.retain(|(name, _)| *name != flag.long);
        }

        matches.flags.push((flag.long, Some(value)));
    }

    Ok(matches)
}

/// Strips the leading dashes, if the argument is a flag at all.
///
/// A bare `-` is a positional: it is the conventional spelling of standard
/// input, and refusing it here would make that spelling impossible to add.
fn long_or_short(argument: &str) -> Option<&str> {
    if argument == "-" || argument == "--" {
        return None;
    }

    argument.strip_prefix("--").or_else(|| argument.strip_prefix('-'))
}

fn root_flag(token: &str) -> Option<&'static Flag> {
    ROOT.iter().find(|flag| {
        flag.long == token || flag.short.is_some_and(|short| token == short.to_string())
    })
}

fn unknown_root_flag(argument: &str) -> String {
    format!("`{argument}` is not a slidx option.\n\n{}", suggest(&command::names()))
}

fn unknown_command(name: &str) -> String {
    format!("`{name}` is not a slidx command.\n\n{}", suggest(&command::names()))
}

fn unknown_flag(command: &'static Command, argument: &str) -> String {
    let known: Vec<&str> = command.flags.iter().map(|flag| flag.long).chain(["help"]).collect();

    format!(
        "`{argument}` is not an option of `slidx {}`.\n\nIt accepts: {}\n\nTry: slidx {} --help",
        command.name,
        known.iter().map(|long| format!("--{long}")).collect::<Vec<_>>().join(", "),
        command.name,
    )
}

fn missing_value(command: &'static Command, flag: &'static Flag) -> String {
    format!(
        "--{} needs a value.\n\nTry: slidx {} --{} {}",
        flag.long,
        command.name,
        flag.long,
        flag.value.unwrap_or("<value>")
    )
}

fn suggest(names: &[&str]) -> String {
    format!("slidx has: {}\n\nTry: slidx --help", names.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_line(line: &str) -> Invocation {
        let argv: Vec<String> = line.split_whitespace().map(String::from).collect();
        parse(&argv)
    }

    fn matches_of(line: &str) -> Matches {
        match parse_line(line) {
            Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    fn misuse_of(line: &str) -> String {
        match parse_line(line) {
            Invocation::Misuse(message) => message,
            other => panic!("expected a misuse, got {other:?}"),
        }
    }

    #[test]
    fn slidx_on_its_own_prints_the_help_rather_than_doing_something() {
        // A tool that guesses what an empty command line meant is a tool that
        // occasionally guesses wrong on a machine somebody cares about.
        assert_eq!(parse_line(""), Invocation::Help(None));
    }

    #[test]
    fn the_version_is_asked_for_before_a_subcommand_is_looked_for() {
        assert_eq!(parse_line("--version"), Invocation::Version);
        assert_eq!(parse_line("-V"), Invocation::Version);
    }

    #[test]
    fn help_can_be_asked_for_globally_or_of_one_command() {
        assert_eq!(parse_line("--help"), Invocation::Help(None));
        assert_eq!(parse_line("-h"), Invocation::Help(None));

        let Invocation::Help(Some(command)) = parse_line("doctor --help") else {
            panic!("doctor --help should describe doctor");
        };
        assert_eq!(command.name, "doctor");
    }

    #[test]
    fn help_wins_over_everything_else_on_the_line() {
        // Someone who has typed `--help` wants to read, not to run. Probing a
        // machine or linting a deck because the rest of the line also parsed
        // would be the wrong reading of that.
        assert!(matches!(parse_line("lint slides --json --help"), Invocation::Help(Some(_))));
    }

    #[test]
    fn a_switch_is_recorded_without_a_value() {
        let matches = matches_of("doctor --json --offline");

        assert!(matches.is_set("json"));
        assert!(matches.is_set("offline"));
        assert!(!matches.is_set("explain"));
    }

    #[test]
    fn a_flag_takes_the_argument_after_it_or_the_one_glued_on_with_an_equals() {
        assert_eq!(matches_of("lint --theme terminal").value("theme"), Some("terminal"));
        assert_eq!(matches_of("lint --theme=terminal").value("theme"), Some("terminal"));
    }

    #[test]
    fn a_value_containing_an_equals_survives_the_split() {
        // `--separator=---` and, worse, a separator that is itself an `=`. The
        // split has to be on the first one only.
        assert_eq!(matches_of("lint --separator=a=b").value("separator"), Some("a=b"));
    }

    #[test]
    fn a_repeatable_flag_keeps_every_value_in_order() {
        let matches = matches_of("lint --allow contrast --allow structure/missing-alt");

        assert_eq!(
            matches.values("allow").collect::<Vec<_>>(),
            ["contrast", "structure/missing-alt"]
        );
    }

    #[test]
    fn a_flag_that_is_not_repeatable_keeps_the_last_value_given() {
        // So a wrapper script can set a default and a caller can override it by
        // appending to the command line rather than editing the script.
        assert_eq!(
            matches_of("lint --theme minimal --theme contrast").value("theme"),
            Some("contrast")
        );
    }

    #[test]
    fn the_deck_path_is_read_as_a_positional() {
        assert_eq!(matches_of("lint ./talk/slides").first_positional(), Some("./talk/slides"));
        assert_eq!(matches_of("lint").first_positional(), None);
    }

    #[test]
    fn a_path_after_a_bare_double_dash_is_a_path_even_when_it_looks_like_a_flag() {
        let matches = matches_of("lint -- --strange-directory");

        assert_eq!(matches.first_positional(), Some("--strange-directory"));
        assert!(!matches.is_set("strict"));
    }

    #[test]
    fn a_single_dash_is_a_positional_rather_than_an_unknown_flag() {
        // The conventional spelling of standard input. Rejecting it here would
        // make it impossible to mean later.
        assert_eq!(matches_of("lint -").first_positional(), Some("-"));
    }

    #[test]
    fn an_unknown_command_names_the_commands_that_do_exist() {
        let message = misuse_of("lnit");

        assert!(message.contains("`lnit` is not a slidx command"), "{message}");
        assert!(message.contains("doctor"), "{message}");
        assert!(message.contains("lint"), "{message}");
    }

    #[test]
    fn an_unknown_flag_lists_what_the_command_does_accept() {
        let message = misuse_of("lint --strcit");

        assert!(message.contains("`--strcit` is not an option"), "{message}");
        assert!(message.contains("--strict"), "{message}");
    }

    #[test]
    fn a_flag_left_without_its_value_says_so_rather_than_swallowing_the_path() {
        let message = misuse_of("lint --theme");

        assert!(message.contains("--theme needs a value"), "{message}");
        assert!(message.contains("<name>"), "{message}");
    }

    #[test]
    fn a_switch_given_a_value_is_a_misuse_rather_than_a_silent_ignore() {
        // `--json=false` reads like it turns JSON off. Accepting it and
        // printing JSON anyway is the worst of the three options.
        let message = misuse_of("doctor --json=false");

        assert!(message.contains("takes no value"), "{message}");
    }

    #[test]
    fn build_is_answered_with_the_plugin_rather_than_with_unknown_command() {
        // The one command people will type first. "Unknown command" would send
        // them looking for a flag that is never coming.
        let Invocation::Declined { name, reason } = parse_line("build") else {
            panic!("build should be declined, not unknown");
        };

        assert_eq!(name, "build");
        assert!(reason.contains("@slidx/vite-plugin"), "{reason}");
    }

    #[test]
    fn every_command_in_the_table_parses_with_no_arguments() {
        // The table and the parser are the same list read twice. A command that
        // is declared but unreachable would only show up here.
        for command in command::ALL {
            assert!(
                matches!(parse_line(command.name), Invocation::Run(..)),
                "{} is declared but does not parse",
                command.name
            );
        }
    }

    #[test]
    fn every_flag_in_the_table_is_accepted_by_the_command_that_declares_it() {
        for command in command::ALL {
            for flag in command.flags {
                let line = match flag.value {
                    Some(_) => format!("{} --{} placeholder", command.name, flag.long),
                    None => format!("{} --{}", command.name, flag.long),
                };

                assert!(
                    matches!(parse_line(&line), Invocation::Run(..)),
                    "{}: --{} is documented but rejected",
                    command.name,
                    flag.long
                );
            }
        }
    }
}
