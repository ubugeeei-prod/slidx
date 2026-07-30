//! Every `slidx` command, from the table the binary itself reads.
//!
//! [`slidx_cli::command::ALL`] is the only place a command is declared: the
//! argument parser, the help text and six completion scripts all read it, and
//! there is a test upstream that fails when the table and the dispatcher
//! disagree. Rendering the reference from it means the page cannot describe a
//! flag the parser does not accept, which is the usual way a CLI reference goes
//! wrong.
//!
//! The declined commands are here for the same reason they are in the binary.
//! `slidx build` is a reasonable thing to type, and answering it with "unknown
//! command" would leave a person believing the tool cannot do the thing it
//! exists to do — so the refusal, and why, is documentation rather than an
//! error message.

use slidx_cli::command::{Command, ALL, DECLINED, GLOBAL};

use super::{code, escape, prose, table};

/// Every command, as a section with its own flag table.
pub fn commands() -> String {
    ALL.iter().map(section).collect::<Vec<String>>().join("\n")
}

fn section(command: &Command) -> String {
    let mut html = format!(
        "<h3 id=\"slidx-{name}\"><code>slidx {name}</code></h3>\n<p><em>{summary}</em></p>\n{about}\n",
        name = escape(command.name),
        summary = escape(command.summary),
        about = prose(command.about),
    );

    html.push_str(&format!("<p><code>slidx {}</code></p>\n", escape(command.usage)));

    // `all_flags` folds in the flags every command accepts, so the page shows
    // what the parser would take rather than only what this entry listed.
    let flags: Vec<Vec<String>> = command
        .all_flags()
        .into_iter()
        .filter(|flag| !GLOBAL.iter().any(|global| global.long == flag.long))
        .map(|flag| {
            let spelling = match flag.value {
                Some(value) => format!("--{} {value}", flag.long),
                None => format!("--{}", flag.long),
            };
            vec![code(&spelling), prose(flag.summary)]
        })
        .collect();

    if !flags.is_empty() {
        html.push_str(&table(&["Flag", "What it does"], flags));
    }

    html
}

/// The commands slidx does not have, and what to type instead.
pub fn declined() -> String {
    let rows = DECLINED
        .iter()
        .map(|(name, reason)| vec![code(&format!("slidx {name}")), prose(reason)])
        .collect();

    table(&["Not a command", "Why, and what to do"], rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_the_binary_has_is_on_the_page() {
        let html = commands();

        for name in slidx_cli::command::names() {
            assert!(html.contains(&format!("slidx {name}</code>")), "{name} is undocumented");
        }
    }

    #[test]
    fn every_command_carries_the_paragraph_someone_reads_before_running_it() {
        let html = commands();

        for command in ALL {
            let first = command.about.lines().next().unwrap_or_default();
            assert!(!first.is_empty(), "{} has no about text", command.name);
        }
        assert!(html.contains("<h3 id=\"slidx-doctor\">"), "commands are not linkable");
    }

    #[test]
    fn a_flag_that_takes_a_value_shows_its_placeholder() {
        // `--allow` and `--allow <code>` are different things to type, and a
        // reference that spelled them the same would be worse than none.
        assert!(commands().contains("--theme &lt;name&gt;"));
    }

    #[test]
    fn help_is_not_repeated_under_every_command() {
        // `-h` is accepted everywhere. Printing it nineteen times is noise in a
        // table whose whole value is that the differences stand out.
        assert!(!commands().contains("<code>--help</code>"));
    }

    #[test]
    fn the_commands_slidx_declines_say_what_to_type_instead() {
        let html = declined();

        assert!(html.contains("slidx build"));
        assert!(html.contains("vite build"), "the refusal has to name the alternative: {html}");
    }
}
