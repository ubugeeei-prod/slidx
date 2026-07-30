//! `slidx dev` — write the deck, with the editor open.
//!
//! ## What it is, and what it is not
//!
//! This command starts **the project's own dev server** and nothing else. The
//! server is Vite with `@slidx/vite-plugin`, which is what renders a slide,
//! watches the slide directory, and serves the visual editor at
//! [`EDITOR_ROUTE`]. slidx contains no second copy of any of that, and adding
//! one would be the same mistake `slidx build` does not exist in order to
//! avoid: the artifact a speaker stands in front of has to come from one place.
//!
//! So what does the command earn? Three things a bare `vite dev` cannot do.
//! It is pointed at a **deck** rather than at a project — `./slides` by
//! default, the same path `slidx lint` and `slidx tui` take — and finds the
//! Vite config by walking up from it, so it works from inside a monorepo and
//! from inside the slide directory alike. It reaches Vite through the package
//! manager that actually installed it, which is what turns "vite: not found"
//! into a command that works. And it opens the editor, which is the page this
//! command exists for and the one Vite has no reason to know about.
//!
//! ## `dev` or `preview`
//!
//! Authoring versus looking at the result, and the line is sharp:
//!
//! | | `slidx dev` | `slidx preview` |
//! | --- | --- | --- |
//! | what it serves | the deck's *source*, live | the *build output* in `dist/` |
//! | the editor | yes, at `/__slidx/` | never — those routes write files |
//! | writes to your slides | yes, when you edit in the canvas | no |
//! | what it proves | the deck you are writing | the artifact a host will serve |
//!
//! A speaker checking the deck the night before a talk wants `preview`, because
//! the thing that goes on the projector is the build. An author writing a slide
//! wants this.
//!
//! ## Why the child keeps the terminal
//!
//! Vite's own output is inherited rather than captured. A dev server whose
//! output was reformatted by slidx would be a dev server whose errors do not
//! match anything an author can search for, and a wrapper that owned the
//! terminal would have to reimplement the URL block, the HMR log, and the
//! overlay's console half. Everything slidx has to say is said before the child
//! starts.

pub mod launch;
pub mod project;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use slidx_core::{parse_deck, DeckParseOptions};

use crate::args::Matches;
use crate::home::Home;
use crate::index::{self, Entry};
use crate::lint::{project_root, source};
use crate::report;
use crate::style::{Ink, Style};
use crate::{Outcome, OK};

use launch::Runner;
use project::Project;

/// Where the visual editor is served.
///
/// Written here as well as in `packages/vite-plugin/src/editor.ts`, because
/// nothing in Rust can read a TypeScript constant. Two spellings of one route is
/// tolerable only while both are pinned by a test, which is what the suite below
/// does on this side and `test/session.test.ts` does on the other.
pub const EDITOR_ROUTE: &str = "/__slidx/";

/// This command prints values rather than a column of findings, so it does not
/// wear the status column's indent — the same reasoning as `slidx preview`.
const VALUE_INDENT: usize = 2;

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let deck = matches
        .first_positional()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source::DEFAULT_DIR));

    // Checked before anything is started, because a dev server serving no deck
    // is a browser window somebody has to read an empty page in to find out.
    // An *empty* slide directory is fine — writing the first slide in the
    // editor is exactly what this command is for.
    if !deck.exists() {
        return Outcome::misuse(no_deck(&deck));
    }

    let Some(project) = Project::find(&deck) else {
        return Outcome::misuse(no_project(&deck));
    };

    let flags = match vite_flags(matches) {
        Ok(flags) => flags,
        Err(message) => return Outcome::misuse(message),
    };

    let runner = launch::runner_for(&project.root);
    let planned = launch::plan(runner, &flags);

    remember(&deck);

    // Written now rather than returned: the next call blocks until somebody
    // stops the dev server, so an Outcome printed afterwards would appear when
    // the server is already gone.
    let mut out = std::io::stdout();
    let _ = write!(out, "{}", ready(&project, runner, style));
    let _ = out.flush();

    let started =
        Command::new(&planned.program).args(&planned.args).current_dir(&project.root).status();

    match started {
        Ok(status) => finished(status.code()),
        Err(_) => Outcome::misuse(cannot_run(runner)),
    }
}

/// The Vite flags this command decided on.
///
/// Only flags Vite and `vp dev` both accept, which is what lets one vocabulary
/// serve every runner. `--open` takes the editor's route, so the browser lands
/// on the page the command exists for rather than on the first slide.
fn vite_flags(matches: &Matches) -> Result<Vec<String>, String> {
    let mut flags = Vec::new();

    if let Some(port) = matches.value("port") {
        if port.parse::<u16>().is_err() {
            return Err(bad_port(port));
        }
        flags.push("--port".to_string());
        flags.push(port.to_string());
    }

    if !matches.is_set("no-open") {
        flags.push("--open".to_string());
        flags.push(EDITOR_ROUTE.to_string());
    }

    Ok(flags)
}

/// What is printed before the dev server takes the terminal.
pub fn ready(project: &Project, runner: Runner, style: &Style) -> String {
    let mut text = format!("{}\n\n", style.paint(Ink::Strong, "slidx dev"));

    text.push_str(&report::flowed(
        &format!("the editor is at {EDITOR_ROUTE} on the address printed below"),
        VALUE_INDENT,
        Ink::Strong,
        style,
    ));
    text.push_str(&report::flowed(
        &format!("running `{}` in {}", runner.typed(), project.root.display()),
        VALUE_INDENT,
        Ink::Faint,
        style,
    ));

    if !project.mentions_slidx() {
        // Not a refusal — the plugin can be registered from a file the config
        // imports. But promising an editor that is not there would be worse
        // than saying which half of this is a guess.
        text.push_str(&report::flowed(
            &format!(
                "nothing in this project names @slidx/vite-plugin, so {EDITOR_ROUTE} may not exist"
            ),
            VALUE_INDENT,
            Ink::Warn,
            style,
        ));
    }

    text.push_str(&report::flowed("ctrl-c to stop", VALUE_INDENT, Ink::Faint, style));

    text
}

/// What the dev server exiting means.
///
/// A ctrl-c is how this command is *meant* to end, so it exits zero. Everything
/// else is a dev server that stopped for a reason it has already printed, and
/// exit 2 rather than 1: nothing was checked, so nothing was found.
pub fn finished(code: Option<i32>) -> Outcome {
    match code {
        // Killed by a signal, which is ctrl-c on every Unix.
        None => Outcome::default().with_code(OK),
        Some(code) if code == 0 || INTERRUPTED.contains(&code) => Outcome::default().with_code(OK),
        Some(code) => Outcome::misuse(format!(
            "the dev server stopped with exit code {code}; its own output above says why\n"
        )),
    }
}

/// Exit codes that mean somebody pressed ctrl-c.
///
/// 130 is a shell reporting SIGINT. The other is Windows'
/// `STATUS_CONTROL_C_EXIT`, which arrives as an exit code rather than a signal
/// and would otherwise be reported as a dev server that crashed every time an
/// author stopped one.
const INTERRUPTED: [i32; 2] = [130, -1073741510];

/// Records the deck so `slidx open` can find it later.
///
/// Best-effort in the strongest sense — see `index::remember`. A project with
/// no readable deck yet is still worth remembering, because somebody who ran
/// `slidx dev` in it is about to write one.
fn remember(deck: &Path) {
    let Some(project) = project_root(deck) else { return };

    let options = DeckParseOptions::default();
    let mut entry = Entry::new(project);

    if let Ok(read) = source::read(deck, &options.separator) {
        entry = entry.describing(&parse_deck(&read.source, &options));
    }

    index::remember(&Home::discover().index(), entry);
}

fn no_deck(deck: &Path) -> String {
    format!(
        "There is nothing at {}.\n\n\
         `slidx dev` takes the deck to serve and looks in ./{} when given neither, the\n\
         same as `slidx lint`. An empty slide directory is fine — the editor is how you\n\
         write the first slide — but it has to be there:\n\n\
         \x20 mkdir {}\n\n\
         Or point this at the deck you have:\n\n\
         \x20 slidx dev path/to/slides\n",
        deck.display(),
        source::DEFAULT_DIR,
        source::DEFAULT_DIR
    )
}

fn no_project(deck: &Path) -> String {
    format!(
        "There is no Vite project at {} or above it.\n\n\
         `slidx dev` starts the project's own dev server; it is not one itself. A deck\n\
         needs @slidx/vite-plugin, which is what serves the slides and the editor:\n\n\
         \x20 npm i -D @slidx/vite-plugin\n\n\
         \x20 // vite.config.ts\n\
         \x20 import {{ slidx }} from \"@slidx/vite-plugin\";\n\
         \x20 export default {{ plugins: [slidx()] }};\n\n\
         To look at a deck that is already built instead:\n\n\
         \x20 slidx preview --web\n",
        deck.display()
    )
}

fn cannot_run(runner: Runner) -> String {
    let program = runner.typed().split(' ').next().unwrap_or("npm");

    format!(
        "`{program}` is not on PATH, so the project's dev server cannot be started.\n\n\
         slidx runs the dev server this project already has rather than shipping one, so\n\
         it needs the package manager that installed it. Install {program}, or start the\n\
         server yourself:\n\n\
         \x20 {}\n",
        runner.typed()
    )
}

fn bad_port(given: &str) -> String {
    format!("`{given}` is not a port number.\n\n  slidx dev --port 5173\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args;

    fn matches_of(line: &str) -> Matches {
        let argv: Vec<String> = line.split_whitespace().map(String::from).collect();
        match args::parse(&argv) {
            args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    fn project() -> Project {
        Project { root: PathBuf::from("/talks/vueconf"), config: PathBuf::from("/nowhere") }
    }

    #[test]
    fn the_editor_route_is_the_one_the_plugin_serves() {
        // Pinned on both sides of the wasm boundary. Nothing in Rust can read a
        // TypeScript constant, so the only defence against the two drifting is
        // a test in each language naming the same string.
        assert_eq!(EDITOR_ROUTE, "/__slidx/");
    }

    #[test]
    fn a_browser_is_opened_on_the_editor_rather_than_on_the_first_slide() {
        // The page this command exists for. Vite has no reason to know it is
        // there, so slidx is the thing that has to say so.
        assert_eq!(vite_flags(&matches_of("dev")).unwrap(), ["--open", EDITOR_ROUTE]);
    }

    #[test]
    fn no_open_passes_no_flags_at_all_so_a_headless_machine_is_left_alone() {
        assert!(vite_flags(&matches_of("dev --no-open")).unwrap().is_empty());
    }

    #[test]
    fn a_port_is_handed_to_vite_in_vites_own_spelling() {
        assert_eq!(
            vite_flags(&matches_of("dev --port 4000 --no-open")).unwrap(),
            ["--port", "4000"]
        );
    }

    #[test]
    fn a_port_that_is_not_a_number_is_refused_here_rather_than_by_the_child() {
        // The child would report it too, in its own words, after slidx had
        // already printed a ready line promising an editor.
        let message = vite_flags(&matches_of("dev --port http")).expect_err("a refusal");

        assert!(message.contains("not a port number"), "{message}");
        assert!(message.contains("slidx dev --port"), "{message}");
    }

    #[test]
    fn the_ready_line_says_where_the_editor_is_and_what_is_being_run() {
        let text = ready(&project(), Runner::Pnpm, &Style::plain());

        assert!(text.contains(EDITOR_ROUTE), "{text}");
        assert!(text.contains("pnpm exec vite"), "{text}");
        assert!(text.contains("/talks/vueconf"), "{text}");
        assert!(text.contains("ctrl-c"), "{text}");
    }

    #[test]
    fn the_ready_line_admits_it_when_nothing_in_the_project_names_the_plugin() {
        // A project whose config is unreadable from here is exactly that case,
        // and promising an editor that is not there would be worse than saying
        // which half is a guess.
        let text = ready(&project(), Runner::Npm, &Style::plain());

        assert!(text.contains("may not exist"), "{text}");
    }

    #[test]
    fn the_ready_line_carries_no_escape_sequences_when_colour_is_off() {
        assert!(!ready(&project(), Runner::Yarn, &Style::plain()).contains('\u{1b}'));
    }

    #[test]
    fn a_dev_server_stopped_with_ctrl_c_exits_zero_because_that_is_how_it_ends() {
        assert_eq!(finished(None).code, OK);
        assert_eq!(finished(Some(0)).code, OK);
        assert_eq!(finished(Some(130)).code, OK);
        // Windows reports the same interruption as an exit code.
        assert_eq!(finished(Some(-1073741510)).code, OK);
    }

    #[test]
    fn a_dev_server_that_failed_to_start_exits_two_rather_than_one() {
        // Exit 1 means "checked, and found something". Nothing was checked.
        let outcome = finished(Some(1));

        assert_eq!(outcome.code, crate::MISUSE);
        assert!(outcome.stderr.contains("exit code 1"), "{}", outcome.stderr);
    }

    #[test]
    fn a_deck_that_is_not_there_stops_the_command_before_a_server_is_started() {
        // A dev server serving no deck is a browser window somebody has to
        // read an empty page in to find out about.
        let outcome = run(&matches_of("dev definitely/not/here"), &Style::plain());

        assert_eq!(outcome.code, crate::MISUSE);
        assert!(outcome.stderr.contains("definitely/not/here"), "{}", outcome.stderr);
        assert!(outcome.stdout.is_empty(), "{}", outcome.stdout);
    }

    #[test]
    fn a_missing_deck_says_that_an_empty_slide_directory_would_have_been_enough() {
        // The editor is how the first slide gets written, so the requirement is
        // that the directory exists — not that anything is in it.
        let message = no_deck(Path::new("slides"));

        assert!(message.contains("An empty slide directory is fine"), "{message}");
        assert!(message.contains("mkdir slides"), "{message}");
    }

    #[test]
    fn a_project_that_is_not_there_is_answered_with_the_plugin_and_with_preview() {
        // Somebody in the wrong directory needs the plugin's name; somebody who
        // wanted to look at a build needs the other command.
        let message = no_project(Path::new("."));

        assert!(message.contains("@slidx/vite-plugin"), "{message}");
        assert!(message.contains("vite.config.ts"), "{message}");
        assert!(message.contains("slidx preview"), "{message}");
    }

    #[test]
    fn a_missing_package_manager_names_the_command_to_run_by_hand() {
        // slidx runs the project's dev server rather than shipping one, so this
        // is the one failure where the answer is a command, not a flag.
        let message = cannot_run(Runner::Pnpm);

        assert!(message.contains("`pnpm` is not on PATH"), "{message}");
        assert!(message.contains("pnpm exec vite"), "{message}");
    }
}
