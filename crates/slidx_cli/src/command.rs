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
}

impl Command {
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

pub use table::{declined, find, names, ALL, DECLINED, GLOBAL, ROOT};

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

    #[test]
    fn every_command_says_what_it_is_for() {
        // `summary` is the root help column and `about` is the page someone
        // reads before deciding to run it. Neither can be blank.
        for command in ALL {
            assert!(!command.summary.is_empty(), "{} has no summary", command.name);
            assert!(!command.about.is_empty(), "{} has no about text", command.name);
            assert!(
                command.usage.starts_with(command.name),
                "{} misstates its usage",
                command.name
            );
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
    fn a_declined_command_names_the_tool_that_does_own_the_job() {
        // The whole value of the table: the reply has to be actionable, not a
        // refusal. Anyone typing `slidx build` needs the plugin's name.
        for (name, reason) in DECLINED {
            assert!(reason.contains("@slidx/vite-plugin"), "{name} refuses without redirecting");
        }
    }
}
