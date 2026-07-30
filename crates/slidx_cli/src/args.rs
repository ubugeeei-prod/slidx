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
    Help(Option<Route>),
    Version,
    Run(Route, Matches),
    /// A command slidx deliberately does not have. Carries the redirect.
    Declined {
        name: String,
        reason: &'static str,
    },
    /// Nothing could be run. The string is addressed to the person who typed it.
    Misuse(String),
}

/// Which command was named, and — for a nested one — what it sits under.
///
/// The parent is kept rather than flattened because two commands under
/// different parents may share a name. `list` alone says nothing; `version
/// list` says everything, and it is what the dispatch and the help both key on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    pub command: &'static Command,
    pub parent: Option<&'static Command>,
}

impl Route {
    pub fn to(command: &'static Command) -> Self {
        Self { command, parent: None }
    }

    pub fn under(parent: &'static Command, command: &'static Command) -> Self {
        Self { command, parent: Some(parent) }
    }

    /// The pair the dispatch matches on, allocating nothing.
    pub fn key(&self) -> (Option<&'static str>, &'static str) {
        (self.parent.map(|parent| parent.name), self.command.name)
    }

    /// How somebody would have typed it: `lint`, or `version use`.
    pub fn typed(&self) -> String {
        match self.parent {
            Some(parent) => format!("{} {}", parent.name, self.command.name),
            None => self.command.name.to_string(),
        }
    }
}

impl std::ops::Deref for Route {
    type Target = Command;

    fn deref(&self) -> &Self::Target {
        self.command
    }
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

    let (route, remaining) = descend(found, rest.as_slice());
    let Some(route) = route else {
        return Invocation::Misuse(unknown_subcommand(found, &remaining[0]));
    };

    match collect(route.command, remaining) {
        Ok(matches) if matches.is_set("help") => Invocation::Help(Some(route)),
        Ok(matches) => Invocation::Run(route, matches),
        Err(message) => Invocation::Misuse(message),
    }
}

/// Resolves `version use` to its leaf, leaving the rest of the line alone.
///
/// A parent with no subcommand named falls back to its default — `slidx
/// version` means `slidx version current`, because the obvious reading of a
/// bare command is better than printing its help at somebody. A parent with no
/// default and no subcommand keeps itself as the route, so `--help` still
/// lands on the page listing what it has.
///
/// Returns `None` for a first argument that looks like a subcommand and is not
/// one; the caller turns that into a message naming the ones that exist.
fn descend<'a>(parent: &'static Command, argv: &'a [String]) -> (Option<Route>, &'a [String]) {
    if !parent.has_subcommands() {
        return (Some(Route::to(parent)), argv);
    }

    match argv.first() {
        // A flag, or nothing at all: no subcommand was named.
        None => (Some(default_route(parent)), argv),
        Some(first) if long_or_short(first).is_some() => {
            // `--help` belongs to the parent so it can list the children;
            // anything else falls through to the default subcommand, so
            // `slidx version --json` still means `current --json`.
            if is_help(first) {
                (Some(Route::to(parent)), argv)
            } else {
                (Some(default_route(parent)), argv)
            }
        }
        Some(first) => match parent.subcommand(first) {
            Some(child) => (Some(Route::under(parent, child)), &argv[1..]),
            None => (None, argv),
        },
    }
}

fn default_route(parent: &'static Command) -> Route {
    parent
        .default_subcommand
        .and_then(|name| parent.subcommand(name))
        .map(|child| Route::under(parent, child))
        .unwrap_or_else(|| Route::to(parent))
}

fn is_help(argument: &str) -> bool {
    matches!(long_or_short(argument), Some("help") | Some("h"))
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
    let typed = argument.trim_start_matches('-');
    let among = ROOT.iter().map(|flag| flag.long);

    format!(
        "`{argument}` is not a slidx option.\n\n{}{}",
        did_you_mean(command::nearest::to(typed, among).map(|long| format!("--{long}"))),
        suggest(&command::names())
    )
}

fn unknown_command(name: &str) -> String {
    // A command slidx declines is answered elsewhere, with the tool that does
    // own the job. This is the one that is nothing at all.
    format!(
        "`{name}` is not a slidx command.\n\n{}{}",
        did_you_mean(command::nearest::to(name, command::names().into_iter()).map(str::to_string)),
        suggest(&command::names())
    )
}

fn unknown_subcommand(parent: &'static Command, name: &str) -> String {
    let known: Vec<&str> = parent.subcommands.iter().map(|child| child.name).collect();
    let guess = command::nearest::to(name, known.iter().copied())
        .map(|child| format!("{} {child}", parent.name));

    format!(
        "`{name}` is not something `slidx {}` does.\n\n{}It has: {}\n\nTry: slidx {} --help",
        parent.name,
        did_you_mean(guess),
        known.join(", "),
        parent.name
    )
}

/// An unknown flag, and what the command it was given to does take.
///
/// Names the command as well as the flag, because the same flag is right on one
/// command and wrong on another — `--use` belongs to `version install` and not to
/// `version use`, and "unknown option" alone would send somebody looking for a
/// spelling mistake that is not there.
fn unknown_flag(command: &'static Command, argument: &str) -> String {
    let typed = argument.trim_start_matches('-').split('=').next().unwrap_or(argument);
    let listed = command.all_flags();
    let guess = command::nearest::to(typed, listed.iter().map(|flag| flag.long));

    format!(
        "`{argument}` is not an option of `slidx {}`.\n\n\
         {}`slidx {}` — {}\n\n\
         It accepts: {}\n\n\
         Try: slidx {} --help",
        command.name,
        did_you_mean(guess.map(|long| format!("--{long}"))),
        command.name,
        command.summary,
        listed.iter().map(|flag| format!("--{}", flag.long)).collect::<Vec<_>>().join(", "),
        command.name,
    )
}

/// The guess, on its own line, or nothing at all.
///
/// Nothing rather than a hedge: a suggestion slidx is not confident about costs
/// more than it saves, because somebody follows it and reads the wrong page.
fn did_you_mean(guess: Option<String>) -> String {
    match guess {
        Some(candidate) => format!("Did you mean `{candidate}`?\n\n"),
        None => String::new(),
    }
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
    fn a_mistyped_command_is_answered_with_the_one_that_was_meant() {
        // The list is the fallback. Naming the command ends the search instead
        // of handing it back to the person who made the typo.
        assert!(misuse_of("lnit").contains("Did you mean `lint`?"), "{}", misuse_of("lnit"));
        assert!(misuse_of("doctro").contains("Did you mean `doctor`?"));
    }

    #[test]
    fn something_that_is_not_a_misspelling_gets_the_list_and_no_guess() {
        // A wrong guess is worse than none: somebody follows it and reads a page
        // that cannot help them.
        let message = misuse_of("frobnicate");

        assert!(!message.contains("Did you mean"), "{message}");
        assert!(message.contains("slidx has:"), "{message}");
    }

    #[test]
    fn an_unknown_flag_lists_what_the_command_does_accept() {
        let message = misuse_of("lint --strcit");

        assert!(message.contains("`--strcit` is not an option"), "{message}");
        assert!(message.contains("--strict"), "{message}");
    }

    #[test]
    fn an_unknown_flag_names_the_command_and_says_what_that_command_is_for() {
        // The same flag is right on one command and wrong on another, so the
        // message has to say which command refused it — and what that command
        // does, because the answer is often "you wanted the other one".
        let message = misuse_of("lint --web");

        assert!(message.contains("`slidx lint`"), "{message}");
        assert!(message.contains("check a deck for what a room will do to it"), "{message}");
    }

    #[test]
    fn a_mistyped_flag_is_answered_with_the_one_that_was_meant() {
        assert!(misuse_of("lint --strcit").contains("Did you mean `--strict`?"));
        assert!(misuse_of("lint --thmee x").contains("Did you mean `--theme`?"));
    }

    #[test]
    fn a_mistyped_flag_given_a_value_inline_is_still_recognised() {
        // `--thmee=editorial` is the same typo and has to get the same answer;
        // the value is not part of the name.
        assert!(misuse_of("lint --thmee=editorial").contains("Did you mean `--theme`?"));
    }

    #[test]
    fn a_mistyped_subcommand_is_answered_with_the_one_that_was_meant() {
        let message = misuse_of("version instal");

        assert!(message.contains("Did you mean `version install`?"), "{message}");
    }

    #[test]
    fn a_mistyped_root_option_is_answered_with_the_one_that_was_meant() {
        assert!(misuse_of("--verson").contains("Did you mean `--version`?"));
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
        assert!(reason.contains("@ubugeeei/slidx-vite-plugin"), "{reason}");
    }

    #[test]
    fn a_nested_command_is_reached_through_its_parent() {
        let Invocation::Run(route, _) = parse_line("version use 0.3.0") else {
            panic!("version use should run");
        };

        assert_eq!(route.key(), (Some("version"), "use"));
        assert_eq!(route.typed(), "version use");
    }

    #[test]
    fn a_nested_commands_arguments_belong_to_it_and_not_to_its_parent() {
        assert_eq!(matches_of("version use 0.3.0").first_positional(), Some("0.3.0"));
        assert!(matches_of("version current --json").is_set("json"));
    }

    #[test]
    fn a_parent_with_no_subcommand_falls_back_to_its_obvious_reading() {
        // `slidx version` has to mean something. Printing the help at somebody
        // who asked a direct question is a worse answer than answering it.
        let Invocation::Run(route, _) = parse_line("version") else {
            panic!("bare version should run something");
        };

        assert_eq!(route.key(), (Some("version"), "current"));
    }

    #[test]
    fn a_flag_after_a_bare_parent_still_reaches_the_default_subcommand() {
        // `slidx version --json` means `slidx version current --json`, because
        // that is the only reading that is not a refusal.
        let Invocation::Run(route, matches) = parse_line("version --json") else {
            panic!("version --json should run");
        };

        assert_eq!(route.key(), (Some("version"), "current"));
        assert!(matches.is_set("json"));
    }

    #[test]
    fn help_asked_of_a_parent_describes_the_parent_rather_than_its_default() {
        // Somebody typing `slidx version --help` wants the list of what it
        // does, not the page for one of them.
        let Invocation::Help(Some(route)) = parse_line("version --help") else {
            panic!("version --help should describe version");
        };

        assert_eq!(route.key(), (None, "version"));
    }

    #[test]
    fn help_asked_of_a_nested_command_describes_that_one() {
        let Invocation::Help(Some(route)) = parse_line("version install --help") else {
            panic!("version install --help should describe it");
        };

        assert_eq!(route.key(), (Some("version"), "install"));
    }

    #[test]
    fn an_unknown_subcommand_lists_the_ones_that_exist() {
        let message = misuse_of("version instal");

        assert!(message.contains("`instal` is not something `slidx version` does"), "{message}");
        assert!(message.contains("install"), "{message}");
        assert!(message.contains("current"), "{message}");
    }

    #[test]
    fn a_flag_of_a_nested_command_is_not_accepted_by_its_sibling() {
        // `--use` belongs to `install`. Silently accepting it on `use` would
        // make a typo look like it worked.
        assert!(matches!(parse_line("version install --use"), Invocation::Run(..)));
        assert!(matches!(parse_line("version use --use"), Invocation::Misuse(_)));
    }

    #[test]
    fn every_nested_command_in_the_table_parses() {
        // The same guard as for top-level commands: a subcommand that is
        // declared and unreachable would only show up here.
        for parent in command::ALL.iter().filter(|entry| entry.has_subcommands()) {
            for child in parent.subcommands {
                let line = format!("{} {}", parent.name, child.name);
                let Invocation::Run(route, _) = parse_line(&line) else {
                    panic!("{line} is declared but does not parse");
                };

                assert_eq!(route.key(), (Some(parent.name), child.name));
            }
        }
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
