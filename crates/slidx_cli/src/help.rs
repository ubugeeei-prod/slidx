//! The help text, written out from the command table.
//!
//! Generated rather than typed, because a help text maintained by hand next to
//! a parser maintained by hand is two lists that drift — and the drift is
//! silent in the direction that matters, an undocumented flag being far more
//! likely than a documented one that does not exist.
//!
//! Fixed at 80 columns. A terminal narrower than that is rare; a paragraph
//! reflowed to a 200-column window is unreadable everywhere.
//!
//! ## Two ways in, one page
//!
//! `slidx lint --help` and `slidx help lint` produce the same bytes, because
//! both end up in [`command`] with the same route. Two help systems that agreed
//! most of the time would be worse than one, and a test asserts they agree for
//! every command and every subcommand in the table.

use crate::args::{Matches, Route};
use crate::command::{self, Command, Flag, ALL, GLOBAL, ROOT};
use crate::style::{width, Ink, Style};
use crate::Outcome;

const TAGLINE: &str = "the whole life of a conference talk, from proposal to publish";

/// `slidx help`, and `slidx help <command>`.
///
/// The same pages `--help` reaches, from the other spelling. Somebody who has
/// used git types one and somebody who has used a Rust tool types the other, and
/// a tool that answered only one of them would be answering half its users.
pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let asked: Vec<&str> = matches.positional().iter().map(String::as_str).collect();

    match route(&asked) {
        // Nothing named, or `slidx help help`, which has nothing else to say.
        None if asked.is_empty() => Outcome::out(root(style)),
        None => Outcome::misuse(no_such_page(&asked)),
        Some(route) => Outcome::out(command(&route, style)),
    }
}

/// The command a `slidx help …` line is asking about.
fn route(asked: &[&str]) -> Option<Route> {
    let first = command::find(asked.first()?)?;

    match asked.get(1) {
        Some(child) => first.subcommand(child).map(|found| Route::under(first, found)),
        None => Some(Route::to(first)),
    }
}

/// A page that does not exist, and the nearest one that does.
fn no_such_page(asked: &[&str]) -> String {
    let typed = asked.join(" ");
    let guess = command::nearest::to(asked[0], command::names().into_iter());

    let mut text = format!("slidx has no `{typed}` to describe.\n\n");

    if let Some(candidate) = guess {
        text.push_str(&format!("Did you mean `slidx help {candidate}`?\n\n"));
    }

    text.push_str(&format!("It has: {}\n\nTry: slidx --help\n", command::names().join(", ")));

    text
}

/// `slidx` with nothing, or `slidx --help`.
pub fn root(style: &Style) -> String {
    let mut text = String::new();

    text.push_str(&format!("{} — {TAGLINE}\n\n", style.paint(Ink::Strong, "slidx")));
    text.push_str(&format!("{}  slidx <command> [options]\n\n", heading("Usage", style)));

    text.push_str(&heading("Commands", style));
    let column = ALL.iter().map(|command| width::of(command.name)).max().unwrap_or(0);
    for command in ALL {
        text.push_str(&format!(
            "  {}  {}\n",
            style.pad(Ink::Strong, command.name, column),
            command.summary
        ));
    }

    text.push('\n');
    text.push_str(&options(ROOT, style));

    // Said here rather than only in the error for `slidx build`, so somebody
    // scanning the command list for it finds the answer instead of concluding
    // that slidx cannot build a deck.
    text.push_str(
        "\nBuilding is @slidxjs/vite-plugin's job. `vite build` emits the\n\
         deck, PDF and OG images. This binary checks things: the machine you are\n\
         about to speak from, and the deck you are about to show.\n\n\
         `slidx help <command>` describes one command, and so does\n\
         `slidx <command> --help`. They are the same page.\n",
    );

    text
}

/// `slidx <command> --help`, or `slidx <command> <subcommand> --help`.
pub fn command(route: &Route, style: &Style) -> String {
    let entry = route.command;
    let mut text = String::new();

    text.push_str(&format!(
        "{} {} — {}\n\n",
        style.paint(Ink::Strong, "slidx"),
        style.paint(Ink::Strong, route.typed()),
        entry.summary
    ));

    // A nested command's usage line is written the way it is typed — `version
    // install <version>` — so prefixing the parent again would print it twice.
    let usage = match route.parent {
        Some(parent) if !entry.usage.starts_with(parent.name) => {
            format!("{} {}", parent.name, entry.usage)
        }
        _ => entry.usage.to_string(),
    };
    text.push_str(&format!("{}  slidx {usage}\n\n", heading("Usage", style)));
    text.push_str(entry.about);
    text.push_str("\n\n");

    if entry.has_subcommands() {
        text.push_str(&subcommands(entry, style));
        text.push('\n');
    }

    text.push_str(&options(entry.flags, style));

    text
}

/// The `Commands:` block under a command that has children.
fn subcommands(parent: &'static Command, style: &Style) -> String {
    let column = parent.subcommands.iter().map(|child| width::of(child.name)).max().unwrap_or(0);
    let mut text = heading("Commands", style);

    for child in parent.subcommands {
        // The default is marked, so `slidx version` on its own is not a
        // mystery somebody has to run to find out about.
        let note = if parent.default_subcommand == Some(child.name) { "  (default)" } else { "" };

        text.push_str(&format!(
            "  {}  {}{}\n",
            style.pad(Ink::Strong, child.name, column),
            child.summary,
            style.paint(Ink::Faint, note)
        ));
    }

    text
}

/// One `Options:` block, aligned on the widest entry in it.
///
/// The global flags are appended to every command's block rather than listed
/// separately, because a reader looking for `--help` wants it in the list they
/// are already reading.
fn options(flags: &'static [Flag], style: &Style) -> String {
    let listed: Vec<&Flag> = flags
        .iter()
        .chain(GLOBAL.iter().filter(|global| !flags.iter().any(|flag| flag.long == global.long)))
        .collect();

    let labels: Vec<String> = listed.iter().map(|flag| label(flag)).collect();
    let column = labels.iter().map(|label| width::of(label)).max().unwrap_or(0);

    let mut text = heading("Options", style);
    for (flag, label) in listed.iter().zip(&labels) {
        text.push_str(&format!("  {}  {}\n", style.pad(Ink::Strong, label, column), flag.summary));
    }

    text
}

/// `-h, --help` or `      --json <name>`, indented so long and short line up.
fn label(flag: &Flag) -> String {
    let short = match flag.short {
        Some(letter) => format!("-{letter}, "),
        None => "    ".to_string(),
    };

    match flag.value {
        Some(placeholder) => format!("{short}--{} {placeholder}", flag.long),
        None => format!("{short}--{}", flag.long),
    }
}

fn heading(text: &str, style: &Style) -> String {
    format!("{}\n", style.paint(Ink::Strong, format!("{text}:")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Invocation;

    fn run_line(line: &str) -> crate::Outcome {
        let argv: Vec<String> = line.split_whitespace().map(String::from).collect();

        crate::run(&argv, &Style::plain())
    }

    /// Every page there is, by the route that reaches it.
    fn every_route() -> Vec<Route> {
        ALL.iter()
            .flat_map(|entry| {
                std::iter::once(Route::to(entry))
                    .chain(entry.subcommands.iter().map(|child| Route::under(entry, child)))
            })
            .collect()
    }

    #[test]
    fn asking_for_help_by_name_and_by_flag_prints_the_same_page() {
        // Two help systems that agreed most of the time would be worse than
        // one. They agree because both are this file reading one table, and this
        // is the test that keeps it that way.
        for route in every_route() {
            let typed = route.typed();

            assert_eq!(
                run_line(&format!("help {typed}")).stdout,
                run_line(&format!("{typed} --help")).stdout,
                "`slidx help {typed}` and `slidx {typed} --help` disagree"
            );
        }
    }

    #[test]
    fn help_with_nothing_named_is_the_page_the_bare_command_prints() {
        assert_eq!(run_line("help").stdout, run_line("--help").stdout);
    }

    #[test]
    fn help_for_something_that_is_not_a_command_suggests_the_nearest_one() {
        let outcome = run_line("help lnit");

        assert_eq!(outcome.code, crate::MISUSE);
        assert!(outcome.stderr.contains("slidx help lint"), "{}", outcome.stderr);
    }

    #[test]
    fn a_nested_commands_usage_line_names_it_once() {
        // Its usage is written the way it is typed, parent and all, so prefixing
        // the parent again printed `slidx version version install`.
        let text = run_line("help version install").stdout;

        assert!(text.contains("slidx version install <version>"), "{text}");
        assert!(!text.contains("version version"), "{text}");
    }

    #[test]
    fn every_usage_line_is_a_command_line_somebody_could_type() {
        // The line under `Usage:` is the one thing on the page a reader copies
        // verbatim, so it has to start with the binary and the route and nothing
        // repeated.
        for route in every_route() {
            let typed = route.typed();
            let page = command(&route, &Style::plain());
            let usage = page
                .lines()
                .find(|line| line.trim_start().starts_with("slidx "))
                .unwrap_or_else(|| panic!("{typed} has no usage line in:\n{page}"));

            assert_eq!(
                usage.trim().strip_prefix("slidx ").map(|rest| rest.starts_with(&typed)),
                Some(true),
                "`{}` does not begin `slidx {typed}`",
                usage.trim()
            );
        }
    }

    #[test]
    fn help_goes_to_standard_output_because_it_was_asked_for() {
        // Not stderr. `slidx help lint | less` is how a long page gets read.
        let outcome = run_line("help lint");

        assert!(outcome.stderr.is_empty());
        assert_eq!(outcome.code, crate::OK);
    }

    #[test]
    fn every_page_reachable_by_flag_is_reachable_by_name() {
        // A command whose help could only be got at one way would be the one
        // somebody could not find.
        for route in every_route() {
            let typed = route.typed();

            assert!(
                matches!(crate::args::parse(&["help".into(), typed.clone()]), Invocation::Run(..)),
                "`slidx help {typed}` does not resolve"
            );
        }
    }

    #[test]
    fn the_root_help_lists_every_command_with_what_it_is_for() {
        let text = root(&Style::plain());

        for entry in ALL {
            assert!(text.contains(entry.name), "{} is missing from the help", entry.name);
            assert!(text.contains(entry.summary), "{} has no summary in the help", entry.name);
        }
    }

    #[test]
    fn the_root_help_names_the_plugin_rather_than_leaving_building_unexplained() {
        // Somebody scanning the command list for `build` has to find the answer
        // here, or they conclude slidx cannot build a deck and stop looking.
        assert!(root(&Style::plain()).contains("@slidxjs/vite-plugin"));
    }

    #[test]
    fn a_command_help_documents_every_flag_the_parser_accepts() {
        // The drift this file exists to prevent, asserted in the direction it
        // actually happens: a flag added to the table and not to the docs.
        for entry in ALL {
            let text = command(&Route::to(entry), &Style::plain());

            for flag in entry.flags {
                assert!(
                    text.contains(&format!("--{}", flag.long)),
                    "{}: --{} undocumented",
                    entry.name,
                    flag.long
                );
                assert!(
                    text.contains(flag.summary),
                    "{}: --{} has no summary",
                    entry.name,
                    flag.long
                );
            }
        }
    }

    #[test]
    fn every_command_help_offers_help_itself() {
        for entry in ALL {
            assert!(command(&Route::to(entry), &Style::plain()).contains("--help"));
        }
    }

    #[test]
    fn a_flag_that_takes_a_value_shows_the_placeholder_next_to_it() {
        let text =
            command(&Route::to(command::find("lint").expect("lint exists")), &Style::plain());

        assert!(text.contains("--theme <name>"), "{text}");
    }

    #[test]
    fn the_lint_help_says_what_the_exit_code_means() {
        // The reason anyone puts this in CI. If it is not in the help, it is
        // discovered by accident or not at all.
        let text =
            command(&Route::to(command::find("lint").expect("lint exists")), &Style::plain());

        assert!(text.contains("non-zero"), "{text}");
    }

    #[test]
    fn plain_help_carries_no_escape_sequences_anywhere() {
        // Help is the text most likely to end up in a README, an issue, or a
        // `--help | head` in a pipe.
        let mut pages = vec![root(&Style::plain())];
        pages.extend(every_route().iter().map(|route| command(route, &Style::plain())));

        for page in pages {
            assert!(!page.contains('\u{1b}'), "{page}");
        }
    }

    #[test]
    fn no_help_line_runs_past_eighty_columns() {
        // Wrapped help reads as broken. The limit is checked rather than
        // trusted, because it is only ever violated by an edit to a summary.
        //
        // Cells rather than characters: a summary written in Japanese would pass
        // a character count and still wrap in the terminal.
        let mut pages = vec![root(&Style::plain())];
        pages.extend(every_route().iter().map(|route| command(route, &Style::plain())));

        for page in pages {
            for line in page.lines() {
                assert!(width::of(line) <= 80, "{} columns: {line}", width::of(line));
            }
        }
    }

    #[test]
    fn every_summary_in_the_command_list_starts_in_the_same_column() {
        // The reason the list reads as a table. Measured in cells, so a name or
        // a summary written in Japanese is held to the same claim rather than
        // passing a character count and shearing on screen.
        let page = root(&Style::plain());

        let starts: Vec<usize> = ALL
            .iter()
            .map(|entry| {
                let row = page
                    .lines()
                    .find(|line| line.trim_start().starts_with(entry.name))
                    .unwrap_or_else(|| panic!("{} has no row in:\n{page}", entry.name));
                let at = row.find(entry.summary).expect("a summary on the row");

                width::of(&row[..at])
            })
            .collect();

        assert!(starts.windows(2).all(|pair| pair[0] == pair[1]), "{starts:?}\n{page}");
    }

    #[test]
    fn the_short_and_long_forms_of_a_flag_line_up_in_the_column() {
        // `-h, --help` and `    --json` have to start their dashes in the same
        // place, or the block reads as two lists.
        assert_eq!(
            label(&Flag { long: "json", short: None, value: None, summary: "", repeatable: false })
                .find("--"),
            label(&Flag {
                long: "help",
                short: Some('h'),
                value: None,
                summary: "",
                repeatable: false
            })
            .find("--")
        );
    }
}
