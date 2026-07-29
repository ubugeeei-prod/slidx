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

/// One subcommand.
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
}

impl Command {
    /// Finds a flag by its long name or its short letter.
    pub fn flag(&self, token: &str) -> Option<&'static Flag> {
        self.flags.iter().chain(GLOBAL.iter()).find(|flag| {
            flag.long == token
                || flag.short.is_some_and(|short| token.len() == 1 && token.starts_with(short))
        })
    }
}

/// Accepted by every command, so they are not repeated in each table.
pub const GLOBAL: &[Flag] = &[Flag::switch("help", "Print this help").short('h')];

/// Accepted before a subcommand only.
pub const ROOT: &[Flag] = &[
    Flag::switch("help", "Print this help").short('h'),
    Flag::switch("version", "Print the version and exit").short('V'),
];

pub const ALL: &[Command] = &[
    Command {
        name: "doctor",
        summary: "check this machine before you speak",
        usage: "doctor [options]",
        about: "\
Reads power, disk, clock, fonts, running applications and the network, and
says what to do about each one. Everything it looks at is something that goes
wrong on stage and never at a desk, so it is worth the ten seconds in the room
even when it was clean this morning.

A reading that could not be taken is reported as unknown, never as a pass.",
        flags: &[
            Flag::taking("dir", "<path>", "Directory whose volume the disk check measures"),
            Flag::switch("offline", "Take no network readings, and say so in the report"),
            Flag::switch("explain", "Add what each check exists to catch"),
            Flag::switch("json", "Print the findings as JSON"),
        ],
    },
    Command {
        name: "lint",
        summary: "check a deck for what a room will do to it",
        usage: "lint [path] [options]",
        about: "\
Runs every slidx rule over a deck on disk: projector contrast, rendered font
size at the back row, offline assets, heading order, animation cost, and the
time budget against the declared slot.

Exits non-zero when something blocking is found, which is what makes it usable
in CI. `path` is a deck file or a directory of slide files, and defaults to
./slides — the same layout @slidx/vite-plugin builds.",
        flags: &[
            Flag::taking("theme", "<name>", "Theme to resolve colours against"),
            Flag::taking("separator", "<text>", "Slide separator in a single-file deck"),
            Flag::taking("allow", "<code>", "Suppress a rule or a whole group").repeatable(),
            Flag::switch("strict", "Also report advisory findings"),
            Flag::switch("json", "Print the diagnostics as JSON"),
        ],
    },
];

/// Commands slidx deliberately does not have, and where the work actually is.
///
/// Somebody typing one of these has a real need, and "unknown command" would
/// leave them hunting for a flag that is never coming. Naming the tool that
/// does own the job answers the question in one line.
///
/// The build pipeline is the Vite plugin. A second implementation of it here
/// would be two answers to one question — the artifact a speaker stands in
/// front of has to come from one place.
pub const DECLINED: &[(&str, &str)] = &[
    ("build", BUILD_LIVES_IN_THE_PLUGIN),
    ("dev", BUILD_LIVES_IN_THE_PLUGIN),
    ("serve", BUILD_LIVES_IN_THE_PLUGIN),
    ("export", BUILD_LIVES_IN_THE_PLUGIN),
    ("pdf", BUILD_LIVES_IN_THE_PLUGIN),
];

const BUILD_LIVES_IN_THE_PLUGIN: &str = "\
Building a deck belongs to @slidx/vite-plugin, and slidx will not grow a second
copy of it:

    npm i -D @slidx/vite-plugin

    // vite.config.ts
    import { slidx } from \"@slidx/vite-plugin\";
    export default { plugins: [slidx()] };

`vite dev` serves the deck and `vite build` emits the static deck, the PDF and
the OG images.";

pub fn find(name: &str) -> Option<&'static Command> {
    ALL.iter().find(|command| command.name == name)
}

/// The reason slidx does not have this command, if it is one of the declined.
pub fn declined(name: &str) -> Option<&'static str> {
    DECLINED.iter().find(|(candidate, _)| *candidate == name).map(|(_, reason)| *reason)
}

/// Every command name, for the help text and for shell completions.
pub fn names() -> Vec<&'static str> {
    ALL.iter().map(|command| command.name).collect()
}

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
