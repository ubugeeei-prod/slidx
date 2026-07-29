//! The help text, written out from the command table.
//!
//! Generated rather than typed, because a help text maintained by hand next to
//! a parser maintained by hand is two lists that drift — and the drift is
//! silent in the direction that matters, an undocumented flag being far more
//! likely than a documented one that does not exist.
//!
//! Fixed at 80 columns. A terminal narrower than that is rare; a paragraph
//! reflowed to a 200-column window is unreadable everywhere.

use crate::command::{Command, Flag, ALL, GLOBAL, ROOT};
use crate::style::{Ink, Style};

const TAGLINE: &str = "the whole life of a conference talk, from proposal to publish";

/// `slidx` with nothing, or `slidx --help`.
pub fn root(style: &Style) -> String {
    let mut text = String::new();

    text.push_str(&format!("{} — {TAGLINE}\n\n", style.paint(Ink::Strong, "slidx")));
    text.push_str(&format!("{}  slidx <command> [options]\n\n", heading("Usage", style)));

    text.push_str(&heading("Commands", style));
    let width = ALL.iter().map(|command| command.name.len()).max().unwrap_or(0);
    for command in ALL {
        text.push_str(&format!(
            "  {}  {}\n",
            style.pad(Ink::Strong, command.name, width),
            command.summary
        ));
    }

    text.push('\n');
    text.push_str(&options(ROOT, style));

    // Said here rather than only in the error for `slidx build`, so somebody
    // scanning the command list for it finds the answer instead of concluding
    // that slidx cannot build a deck.
    text.push_str(
        "\nBuilding a deck is @slidx/vite-plugin's job — `vite build` emits the deck,\n\
         the PDF and the OG images. This binary checks things: the machine you are\n\
         about to speak from, and the deck you are about to show.\n\n\
         `slidx <command> --help` describes one command.\n",
    );

    text
}

/// `slidx <command> --help`.
pub fn command(command: &'static Command, style: &Style) -> String {
    let mut text = String::new();

    text.push_str(&format!(
        "{} {} — {}\n\n",
        style.paint(Ink::Strong, "slidx"),
        style.paint(Ink::Strong, command.name),
        command.summary
    ));
    text.push_str(&format!("{}  slidx {}\n\n", heading("Usage", style), command.usage));
    text.push_str(command.about);
    text.push_str("\n\n");
    text.push_str(&options(command.flags, style));

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
    let width = labels.iter().map(|label| label.chars().count()).max().unwrap_or(0);

    let mut text = heading("Options", style);
    for (flag, label) in listed.iter().zip(&labels) {
        text.push_str(&format!("  {}  {}\n", style.pad(Ink::Strong, label, width), flag.summary));
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
    use crate::command;

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
        assert!(root(&Style::plain()).contains("@slidx/vite-plugin"));
    }

    #[test]
    fn a_command_help_documents_every_flag_the_parser_accepts() {
        // The drift this file exists to prevent, asserted in the direction it
        // actually happens: a flag added to the table and not to the docs.
        for entry in ALL {
            let text = command(entry, &Style::plain());

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
            assert!(command(entry, &Style::plain()).contains("--help"));
        }
    }

    #[test]
    fn a_flag_that_takes_a_value_shows_the_placeholder_next_to_it() {
        let text = command(command::find("lint").expect("lint exists"), &Style::plain());

        assert!(text.contains("--theme <name>"), "{text}");
    }

    #[test]
    fn the_lint_help_says_what_the_exit_code_means() {
        // The reason anyone puts this in CI. If it is not in the help, it is
        // discovered by accident or not at all.
        let text = command(command::find("lint").expect("lint exists"), &Style::plain());

        assert!(text.contains("non-zero"), "{text}");
    }

    #[test]
    fn plain_help_carries_no_escape_sequences_anywhere() {
        // Help is the text most likely to end up in a README, an issue, or a
        // `--help | head` in a pipe.
        let mut pages = vec![root(&Style::plain())];
        pages.extend(ALL.iter().map(|entry| command(entry, &Style::plain())));

        for page in pages {
            assert!(!page.contains('\u{1b}'), "{page}");
        }
    }

    #[test]
    fn no_help_line_runs_past_eighty_columns() {
        // Wrapped help reads as broken. The limit is checked rather than
        // trusted, because it is only ever violated by an edit to a summary.
        let mut pages = vec![root(&Style::plain())];
        pages.extend(ALL.iter().map(|entry| command(entry, &Style::plain())));

        for page in pages {
            for line in page.lines() {
                assert!(line.chars().count() <= 80, "{} columns: {line}", line.chars().count());
            }
        }
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
