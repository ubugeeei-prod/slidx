//! What commands exist, spelled out once.
//!
//! The parser, the help text, and the shell completions all have to agree about
//! which commands and flags exist. Three hand-maintained lists is three lists
//! that drift, and the failure is quiet: a flag that parses but is undocumented,
//! or a completion that offers something the parser rejects. So this table is
//! the only place a command is declared, and everything else reads it.
//!
//! Adding a command is adding one entry here and one arm in [`crate::run`].

/// A flag, as the parser matches it and as the help text prints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag {
    pub long: &'static str,
    pub short: Option<char>,
    /// The placeholder for a flag that takes a value — `"<path>"`. `None` marks
    /// a boolean flag, which is also how the parser knows not to eat the next
    /// argument.
    pub value: Option<&'static str>,
    pub summary: &'static str,
    /// True when giving the flag twice adds rather than replaces.
    pub repeatable: bool,
}

impl Flag {
    const fn switch(long: &'static str, summary: &'static str) -> Self {
        Self { long, short: None, value: None, summary, repeatable: false }
    }

    const fn taking(long: &'static str, value: &'static str, summary: &'static str) -> Self {
        Self { long, short: None, value: Some(value), summary, repeatable: false }
    }

    const fn repeatable(self) -> Self {
        Self { repeatable: true, ..self }
    }

    const fn short(self, short: char) -> Self {
        Self { short: Some(short), ..self }
    }

    /// True when this flag consumes the argument after it.
    pub fn takes_value(&self) -> bool {
        self.value.is_some()
    }
}

/// One command, and — where it has them — the commands under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub name: &'static str,
    /// One line, for the root help. Lowercase and verbless where it reads
    /// better in a column.
    pub summary: &'static str,
    /// The line under `Usage:`, without the leading `slidx`.
    pub usage: &'static str,
    /// What the command is for, and — where it matters — what it is not. Read
    /// by someone deciding whether this is the thing they want.
    pub about: &'static str,
    pub flags: &'static [Flag],
    /// Commands under this one, as `version list` sits under `version`.
    ///
    /// Empty for a leaf. A command with children is not itself runnable except
    /// through [`Command::default_subcommand`] — `slidx version` on its own has
    /// to mean something, and "print the help" is a worse answer than the
    /// obvious one when there is an obvious one.
    pub subcommands: &'static [Command],
    /// What a parent command does when given no subcommand.
    pub default_subcommand: Option<&'static str>,
    /// True when everything this command prints is a directory the person who
    /// ran it wants to be standing in.
    ///
    /// A process cannot change its parent's directory — so a command like this
    /// can only ever print where to go, and the shell that was typed into is
    /// the only thing that can go there. [`crate::shell::integration`] reads
    /// this to decide which commands its wrapper function captures, in every
    /// shell at once.
    ///
    /// It is deliberately narrow. A command whose output is a report must not
    /// be captured: capturing turns its standard output into a pipe, which
    /// costs it colour and makes it arrive all at once at the end.
    pub takes_the_caller_with_it: bool,
}

impl Command {
    /// Marks a command whose output the caller's shell has to act on.
    ///
    /// Written as a builder so adding a command stays one call to
    /// [`table::leaf`], and so the property reads as the exception it is.
    pub const fn taking_the_caller_with_it(self) -> Self {
        Self { takes_the_caller_with_it: true, ..self }
    }

    /// Finds a flag by its long name or its short letter.
    pub fn flag(&self, token: &str) -> Option<&'static Flag> {
        self.flags.iter().chain(GLOBAL.iter()).find(|flag| {
            flag.long == token
                || flag.short.is_some_and(|short| token.len() == 1 && token.starts_with(short))
        })
    }

    /// One of this command's children, by name.
    pub fn subcommand(&self, name: &str) -> Option<&'static Command> {
        self.subcommands.iter().find(|candidate| candidate.name == name)
    }

    pub fn has_subcommands(&self) -> bool {
        !self.subcommands.is_empty()
    }

    /// Every flag this command accepts, its own and the global ones.
    ///
    /// The same chain [`Command::flag`] looks through, so what completes is
    /// exactly what parses — which is the whole reason this table exists.
    pub fn all_flags(&self) -> Vec<&'static Flag> {
        self.flags
            .iter()
            .chain(
                GLOBAL
                    .iter()
                    .filter(|global| !self.flags.iter().any(|flag| flag.long == global.long)),
            )
            .collect()
    }
}

/// A leaf command with no children, which is most of them.
mod table;

pub mod nearest;

pub use table::{declined, find, names, taking_the_caller_with_them, ALL, DECLINED, GLOBAL, ROOT};

#[cfg(test)]
mod tests {
    use super::*;

    fn every_flag() -> impl Iterator<Item = (&'static str, &'static Flag)> {
        ALL.iter().flat_map(|command| command.flags.iter().map(move |flag| (command.name, flag)))
    }

    #[test]
    fn every_command_is_declared_exactly_once() {
        let mut names = names();
        let total = names.len();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), total, "a command name is declared twice");
    }

    #[test]
    fn no_command_is_both_offered_and_declined() {
        // The two tables answer opposite questions. A name in both would make
        // the parser's behaviour depend on which is consulted first.
        for name in names() {
            assert!(declined(name).is_none(), "{name} is both a command and declined");
        }
    }

    /// Every command and every subcommand, because a nested one is somebody's
    /// whole answer too and has been the one left undocumented before.
    fn every_command() -> impl Iterator<Item = &'static Command> {
        ALL.iter().flat_map(|command| std::iter::once(command).chain(command.subcommands.iter()))
    }

    #[test]
    fn every_command_says_what_it_is_for() {
        // `summary` is the root help column and `about` is the page someone
        // reads before deciding to run it. Neither can be blank.
        for command in every_command() {
            assert!(!command.summary.is_empty(), "{} has no summary", command.name);
            assert!(!command.about.is_empty(), "{} has no about text", command.name);
            assert!(
                command.usage.starts_with(command.name)
                    || command.usage.contains(&format!(" {}", command.name)),
                "{} misstates its usage",
                command.name
            );
        }
    }

    #[test]
    fn no_command_is_documented_with_a_label_instead_of_an_explanation() {
        // The failure this catches is a command added in a hurry with `about`
        // set to a copy of its summary. That reads as documented and answers
        // nothing, and it fails CI here rather than shipping.
        for command in every_command() {
            assert_ne!(command.about, command.summary, "{}'s page is its summary", command.name);
            assert!(
                command.about.chars().count() > 80,
                "{}'s page is {} characters — too short to say why",
                command.name,
                command.about.chars().count()
            );
        }
    }

    #[test]
    fn every_command_shows_at_least_one_worked_example() {
        // A flag list says what exists. An example says what a whole line looks
        // like, which is what somebody is actually trying to write.
        for command in every_command() {
            let has_one = command
                .about
                .lines()
                .any(|line| line.starts_with("    ") && line.contains("slidx"));

            assert!(has_one, "{} documents no example anybody could copy", command.name);
        }
    }

    #[test]
    fn an_example_fits_the_page_it_is_printed_on() {
        // `about` is printed as written — the help text wraps nothing, because
        // reflowing it would reflow the examples in it too. So the 80 columns
        // the page is fixed at are this string's to keep, and a long example is
        // the one thing that overhangs them.
        for command in every_command() {
            for line in command.about.lines() {
                assert!(
                    line.chars().count() <= 80,
                    "{}: {} characters is past the page: {line}",
                    command.name,
                    line.chars().count()
                );
            }
        }
    }

    #[test]
    fn no_command_declares_the_same_flag_twice() {
        for command in ALL {
            let mut longs: Vec<&str> = command.flags.iter().map(|flag| flag.long).collect();
            let total = longs.len();
            longs.sort_unstable();
            longs.dedup();

            assert_eq!(longs.len(), total, "{} declares a flag twice", command.name);
        }
    }

    #[test]
    fn no_command_redeclares_a_global_flag() {
        // A local `--help` would shadow the global one and could disagree with
        // it. Lookup chains the two tables, so the local entry would win
        // silently.
        for (command, flag) in every_flag() {
            assert!(
                !GLOBAL.iter().any(|global| global.long == flag.long),
                "{command} redeclares the global --{}",
                flag.long
            );
        }
    }

    #[test]
    fn flag_names_are_spelled_without_their_dashes() {
        // The parser strips the dashes before it looks a flag up. A table entry
        // written as "--json" would then never match anything.
        for (command, flag) in every_flag() {
            assert!(!flag.long.starts_with('-'), "{command}: --{} carries its dashes", flag.long);
            assert!(!flag.summary.is_empty(), "{command}: --{} has no summary", flag.long);
        }
    }

    #[test]
    fn a_flag_that_takes_a_value_names_its_placeholder() {
        // The placeholder is what the help text prints. Without one the flag
        // documents itself as a switch and reads as taking nothing.
        for (_, flag) in every_flag().filter(|(_, flag)| flag.takes_value()) {
            let placeholder = flag.value.unwrap_or_default();
            assert!(placeholder.starts_with('<'), "--{} has a bare placeholder", flag.long);
        }
    }

    #[test]
    fn a_flag_is_found_by_its_long_name_or_its_short_letter() {
        let doctor = find("doctor").expect("doctor is a command");

        assert_eq!(doctor.flag("json").map(|flag| flag.long), Some("json"));
        assert_eq!(doctor.flag("h").map(|flag| flag.long), Some("help"));
        assert!(doctor.flag("nonsense").is_none());
    }

    #[test]
    fn every_command_accepts_the_global_flags_without_declaring_them() {
        for command in ALL {
            assert!(command.flag("help").is_some(), "{} cannot be asked for help", command.name);
        }
    }

    #[test]
    fn at_least_one_command_needs_the_callers_shell_or_the_integration_is_a_stub() {
        // `slidx shell` writes a wrapper function whose whole job is to follow
        // these. With none of them declared it would write a function that
        // shadows the binary and does nothing, which is worse than writing
        // nothing at all.
        assert!(!taking_the_caller_with_them().is_empty());
    }

    #[test]
    fn only_a_top_level_command_asks_for_the_callers_shell() {
        // The wrapper matches on the first word after `slidx`, in four shells.
        // A nested command marked here would be silently ignored by all of
        // them, which is the kind of dead declaration this table exists to
        // make impossible.
        for parent in ALL.iter().filter(|entry| entry.has_subcommands()) {
            for child in parent.subcommands {
                assert!(
                    !child.takes_the_caller_with_it,
                    "`{} {}` asks for the caller's shell, which only a top-level command can",
                    parent.name, child.name
                );
            }
        }
    }

    #[test]
    fn a_command_that_takes_the_caller_with_it_names_the_thing_that_carries_it() {
        // These commands are half a feature on their own: they print where to go
        // and cannot go there. `slidx shell` is the other half, and somebody
        // reading this page is exactly the person who needs to know it exists —
        // otherwise they write the command substitution and never learn that
        // they did not have to.
        for command in ALL.iter().filter(|entry| entry.takes_the_caller_with_it) {
            assert!(
                command.about.contains("slidx shell"),
                "{} is followed by the shell integration and never mentions it",
                command.name
            );
        }
    }

    #[test]
    fn a_declined_command_names_the_tool_that_does_own_the_job() {
        // The whole value of the table: the reply has to be actionable, not a
        // refusal. Anyone typing `slidx build` needs the plugin's name.
        for (name, reason) in DECLINED {
            assert!(reason.contains("@slidx/vite-plugin"), "{name} refuses without redirecting");
        }
    }
}
