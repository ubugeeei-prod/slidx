//! `slidx theme` — what slidx would do with a theme document.
//!
//! # Who this is for
//!
//! The author of a theme package, before they publish it. A deck author never
//! needs this: the build already reads the packages their project installed,
//! hardens each one, and reports whatever the linter says about the theme it
//! renders with. Nothing here is a second opinion on any of that — it runs
//! `slidx_theme::package` and `slidx_theme::audit`, the same two the build
//! runs, and reports what came back.
//!
//! What it adds is the *room*. A build judges a theme in the room the deck is
//! being built for; a theme is published once and shown in all of them, so this
//! one runs [`audit_every_room`](slidx_theme::audit::audit_every_room). A theme
//! that only holds up in a dark hall is a theme somebody finds out about on
//! stage.
//!
//! # Why it takes a path rather than a package name
//!
//! Resolving `@example/theme-x` to a directory means resolving `node_modules`,
//! and `@slidx/vite-plugin` is where that is done — once, in the place with
//! Node's own view of a project. A second resolver here would be a second
//! answer to which theme a name refers to, which is the one thing this feature
//! must not have two of.
//!
//! So this reads a file. A theme's author has the file open; a deck's author
//! who wants to look at one has a path their package manager already printed.

use std::fs;
use std::path::Path;

use slidx_core::Diagnostics;
use slidx_theme::package::{Catalogue, Published};
use slidx_theme::{audit, builtin, Theme};

use crate::args::Matches;
use crate::report;
use crate::style::{Ink, Style};
use crate::{Outcome, FOUND, OK};

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    match matches.first_positional() {
        Some(path) => document(Path::new(path), style),
        None => builtins(style),
    }
}

/// The status word for a theme slidx itself ships.
///
/// Seven characters, because [`report::STATUS_WIDTH`] is the column every slidx
/// report shares and a longer word pushes the rest of the row out by exactly
/// its overflow — visible as a wrapped line that no longer lines up under the
/// one above it.
const SHIPPED: &str = "shipped";

/// Every theme slidx ships, and what each is for.
///
/// The answer to "what can I write in `theme:`" on a machine with no project in
/// front of it. Package themes are deliberately absent: this command cannot see
/// them without resolving `node_modules`, and a list that silently omitted the
/// theme somebody had just installed would be worse than one that never claimed
/// to be complete.
fn builtins(style: &Style) -> Outcome {
    let mut text = format!("{}\n\n", style.paint(Ink::Strong, "slidx theme"));
    let width = builtin::all().iter().map(|theme| theme.id.len()).max().unwrap_or(0);

    for theme in builtin::all() {
        text.push_str(&report::line(
            SHIPPED,
            Ink::Pass,
            &theme.id,
            &theme.description,
            width,
            style,
        ));
    }

    text.push_str(&format!(
        "\n{}\n",
        style.paint(
            Ink::Faint,
            "  A theme package adds a name here for the project that installed it.\n  \
             Pass a path to a theme document to check one:  slidx theme ./theme.json",
        )
    ));

    Outcome::out(text)
}

/// One theme document, read the way a build would read it.
fn document(path: &Path, style: &Style) -> Outcome {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            return Outcome::misuse(format!("Could not read {}: {error}\n", path.display()))
        }
    };

    let label = path.display().to_string();
    let catalogue = Catalogue::read(&[Published::new(&label, text)]);

    let Some((_, theme)) = catalogue.installed().next() else {
        return Outcome::out(render(&label, None, catalogue.diagnostics(), style)).with_code(FOUND);
    };

    let theme = theme.clone();
    let mut findings = catalogue.diagnostics().clone();
    findings.extend(audit::audit_every_room(&theme));

    let code = if findings.is_empty() { OK } else { FOUND };

    Outcome::out(render(&label, Some(&theme), &findings, style)).with_code(code)
}

/// The report, as a person reads it.
fn render(label: &str, theme: Option<&Theme>, findings: &Diagnostics, style: &Style) -> String {
    let mut text = format!(
        "{} {}\n\n",
        style.paint(Ink::Strong, "slidx theme"),
        style.paint(Ink::Faint, label)
    );

    match theme {
        Some(theme) => text.push_str(&report::line(
            "theme",
            Ink::Pass,
            &theme.id,
            &format!("{} — {}", theme.name, theme.description),
            theme.id.len(),
            style,
        )),
        // Nothing about the document survived, so there is nothing to describe.
        // The findings below say why, and they are the whole report. Not a
        // column line: the subject there is a short locator, and this one is
        // whatever path somebody typed.
        None => text.push_str(&format!(
            "  {}  {}\n",
            style.pad(Ink::Fail, "unread", report::STATUS_WIDTH),
            style.paint(Ink::Strong, "not a theme document"),
        )),
    }

    text.push('\n');

    for finding in findings.iter() {
        let (status, ink) =
            if finding.is_blocking() { ("fail", Ink::Fail) } else { ("warn", Ink::Warn) };

        text.push_str(&report::block(
            status,
            ink,
            &finding.code,
            &finding.message,
            finding.help.as_deref(),
            style,
        ));
        text.push('\n');
    }

    if findings.is_empty() {
        text.push_str(&format!(
            "  {}\n",
            style.paint(Ink::Pass, "Legible in every room slidx models."),
        ));
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the command the way a person types it.
    fn run_on(path: &str) -> Outcome {
        let argv: Vec<String> =
            format!("theme {path}").split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => run(&matches, &Style::plain()),
            other => panic!("expected a run, got {other:?}"),
        }
    }

    fn check(name: &str, document: &str) -> Outcome {
        let path =
            std::env::temp_dir().join(format!("slidx-theme-{name}-{}.json", std::process::id()));
        fs::write(&path, document).expect("write");

        let outcome = run_on(&path.display().to_string());
        let _ = fs::remove_file(&path);

        outcome
    }

    #[test]
    fn the_theme_this_repository_publishes_passes_in_every_room() {
        // The command a theme author runs before publishing, run against the
        // one theme this repository publishes.
        let outcome = check("published", &slidx_theme::published::document());

        assert_eq!(outcome.code, OK, "{}", outcome.stdout);
        assert!(outcome.stdout.contains("workshop"), "{}", outcome.stdout);
    }

    #[test]
    fn a_theme_that_is_illegible_in_a_bright_room_exits_non_zero() {
        // The whole reason a theme author would run this: `audit` alone asks
        // about the default room, and a theme is published once and shown in
        // all of them.
        let mut theme = slidx_theme::published::workshop();
        theme.light.muted = theme.light.surface;
        theme.dark.muted = theme.dark.surface;

        let outcome = check("illegible", &serde_json::to_string(&theme).unwrap());

        assert_eq!(outcome.code, FOUND);
        assert!(outcome.stdout.contains("contrast/"), "{}", outcome.stdout);
    }

    #[test]
    fn a_document_that_is_not_a_theme_says_so_rather_than_reporting_on_nothing() {
        let outcome = check("stylesheet", ":root { --slidx-color-text: red }");

        assert_eq!(outcome.code, FOUND);
        assert!(outcome.stdout.contains("theme/unreadable-package"), "{}", outcome.stdout);
    }

    #[test]
    fn a_path_that_is_not_there_is_a_misuse_rather_than_a_finding() {
        // A job that mistyped a path has to fail differently from one whose
        // theme needs work, the same split `slidx fmt --check` makes.
        assert_eq!(run_on("./nowhere/theme.json").code, crate::MISUSE);
    }

    #[test]
    fn with_no_path_it_names_every_theme_slidx_ships() {
        let outcome = run_on("");

        for theme in builtin::all() {
            assert!(outcome.stdout.contains(&theme.id), "{} is not listed", theme.id);
        }
        assert_eq!(outcome.code, OK);
    }

    #[test]
    fn every_row_of_the_list_lines_up_under_the_one_above_it() {
        // A status word wider than the shared column pushes its row out by
        // exactly its overflow, and the first wrapped description is where it
        // shows. Cheap to state, and invisible in a diff otherwise.
        assert!(SHIPPED.len() <= report::STATUS_WIDTH);

        let outcome = run_on("");
        let indented: Vec<&str> =
            outcome.stdout.lines().filter(|line| line.trim_start().starts_with(SHIPPED)).collect();

        assert_eq!(indented.len(), builtin::all().len());
    }

    #[test]
    fn the_list_says_a_package_can_add_to_it() {
        // A list that read as complete would teach somebody that the theme they
        // just installed does not exist.
        let outcome = run_on("");

        assert!(outcome.stdout.contains("theme package"), "{}", outcome.stdout);
    }
}
