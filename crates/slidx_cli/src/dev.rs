//! `slidx dev` — write the deck, with the editor open.
//!
//! ## What it is, and what it is not
//!
//! This command starts **the project's own dev server** and nothing else. The
//! server is Vite with `@slidxjs/vite-plugin`, which is what renders a slide,
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
pub mod share;

use std::io::Write;
use std::net::IpAddr;
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
use share::{address, Share};

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

    let shared = match sharing(matches) {
        Ok(shared) => shared,
        Err(message) => return Outcome::misuse(message),
    };

    let flags = match vite_flags(matches, shared.as_ref()) {
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
    let _ = write!(out, "{}", ready(&project, runner, shared.as_ref(), style));
    let _ = out.flush();

    let mut command = Command::new(&planned.program);
    command.args(&planned.args).current_dir(&project.root);

    // The environment rather than the command line: an argument list is readable
    // by every process on this machine, and this is a capability.
    if let Some(shared) = &shared {
        command.envs(shared.share.environment());
    }

    match command.status() {
        Ok(status) => finished(status.code()),
        Err(_) => Outcome::misuse(cannot_run(runner)),
    }
}

/// A share, once every part of it is settled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shared {
    pub share: Share,
    /// The address the other laptops in the room can reach this one at.
    pub address: IpAddr,
    /// Fixed, because a link cannot be printed for a port Vite has not chosen
    /// yet. See [`vite_flags`] on why `--strictPort` goes with it.
    pub port: u16,
}

/// What `--crdt` asked for, or nothing.
fn sharing(matches: &Matches) -> Result<Option<Shared>, String> {
    let allow_edit = matches.is_set("allow-edit");

    if !matches.is_set("crdt") {
        // Granting edit access to a deck nobody can reach is not a thing to do
        // quietly: somebody who typed this wanted to share.
        return if allow_edit { Err(edit_without_sharing()) } else { Ok(None) };
    }

    let port = match chosen_port(matches)? {
        Some(port) => port,
        None => free_port().ok_or_else(no_port)?,
    };

    let Some(address) = address::on_this_network() else {
        return Err(no_network());
    };

    Ok(Some(Shared {
        share: Share::mint(allow_edit).map_err(|error| error.message())?,
        address,
        port,
    }))
}

/// A port nothing is listening on, for a link that has to be printed up front.
fn free_port() -> Option<u16> {
    // Bound and released: there is a window in which something else could take
    // it, which is exactly why `--strictPort` is passed rather than letting Vite
    // move to the next number under a link that has already been printed.
    crate::preview::server::bind(0).ok()?.local_addr().ok().map(|address| address.port())
}

/// The port the command line asked for, if it asked.
fn chosen_port(matches: &Matches) -> Result<Option<u16>, String> {
    match matches.value("port") {
        None => Ok(None),
        Some(port) => port.parse().map(Some).map_err(|_| bad_port(port)),
    }
}

/// The Vite flags this command decided on.
///
/// Only flags Vite and `vp dev` both accept, which is what lets one vocabulary
/// serve every runner. `--open` takes the editor's route, so the browser lands
/// on the page the command exists for rather than on the first slide.
///
/// Sharing adds two, and the second is the interesting one. A share link names a
/// port, and it is printed before the server is listening — so if that port turns
/// out to be taken, Vite must refuse rather than quietly move to the next number
/// and leave a printed URL pointing at nothing. `--strictPort` is what makes the
/// link either right or absent.
fn vite_flags(matches: &Matches, shared: Option<&Shared>) -> Result<Vec<String>, String> {
    let mut flags = Vec::new();

    match (shared, chosen_port(matches)?) {
        (Some(shared), _) => {
            flags.push("--port".to_string());
            flags.push(shared.port.to_string());
            flags.push("--strictPort".to_string());
            // Bound wide only because sharing was asked for. Without --crdt
            // nothing here mentions --host and the deck stays on loopback.
            flags.push("--host".to_string());
            flags.push("0.0.0.0".to_string());
        }
        (None, Some(port)) => {
            flags.push("--port".to_string());
            flags.push(port.to_string());
        }
        (None, None) => {}
    }

    if !matches.is_set("no-open") {
        flags.push("--open".to_string());
        flags.push(EDITOR_ROUTE.to_string());
    }

    Ok(flags)
}

/// What is printed before the dev server takes the terminal.
pub fn ready(project: &Project, runner: Runner, shared: Option<&Shared>, style: &Style) -> String {
    let heading = if shared.is_some() { "slidx dev — shared" } else { "slidx dev" };
    let mut text = format!("{}\n\n", style.paint(Ink::Strong, heading));

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
                "{EDITOR_ROUTE} may not exist: nothing in this project names @slidxjs/vite-plugin"
            ),
            VALUE_INDENT,
            Ink::Warn,
            style,
        ));
    }

    if let Some(shared) = shared {
        text.push_str(&share::block(
            &shared.share,
            shared.address,
            shared.port,
            VALUE_INDENT,
            style,
        ));
        text.push('\n');
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
         needs @slidxjs/vite-plugin, which is what serves the slides and the editor:\n\n\
         \x20 npm i -D @slidxjs/vite-plugin\n\n\
         \x20 // vite.config.ts\n\
         \x20 import {{ slidx }} from \"@slidxjs/vite-plugin\";\n\
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

fn edit_without_sharing() -> String {
    "--allow-edit grants a second person the right to change your deck, and nothing is \
     shared without --crdt.\n\n\
     \x20 slidx dev --crdt --allow-edit\n\n\
     Without --crdt the dev server is on loopback and the editor is already yours.\n"
        .to_string()
}

fn no_network() -> String {
    "This machine has no address another machine could reach it at, so there is no share \
     link to print.\n\n\
     A share link is for the laptop next to you on the same Wi-Fi. With no network there \
     is nothing to share to, and slidx will not print a loopback URL as though there \
     were.\n\n\
     `slidx dev` without --crdt still works.\n"
        .to_string()
}

fn no_port() -> String {
    "No free port could be found to share on.\n\n  slidx dev --crdt --port 5173\n".to_string()
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

    fn shared_session(allow_edit: bool) -> Shared {
        Shared {
            share: Share {
                session: "0123456789abcdef".into(),
                read: "00112233445566778899aabbccddeeff".into(),
                edit: allow_edit.then(|| "ffeeddccbbaa99887766554433221100".to_string()),
            },
            address: IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 42)),
            port: 5173,
        }
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
        assert_eq!(vite_flags(&matches_of("dev"), None).unwrap(), ["--open", EDITOR_ROUTE]);
    }

    #[test]
    fn no_open_passes_no_flags_at_all_so_a_headless_machine_is_left_alone() {
        assert!(vite_flags(&matches_of("dev --no-open"), None).unwrap().is_empty());
    }

    #[test]
    fn a_port_is_handed_to_vite_in_vites_own_spelling() {
        assert_eq!(
            vite_flags(&matches_of("dev --port 4000 --no-open"), None).unwrap(),
            ["--port", "4000"]
        );
    }

    #[test]
    fn a_port_that_is_not_a_number_is_refused_here_rather_than_by_the_child() {
        // The child would report it too, in its own words, after slidx had
        // already printed a ready line promising an editor.
        let message = vite_flags(&matches_of("dev --port http"), None).expect_err("a refusal");

        assert!(message.contains("not a port number"), "{message}");
        assert!(message.contains("slidx dev --port"), "{message}");
    }

    #[test]
    fn the_ready_line_says_where_the_editor_is_and_what_is_being_run() {
        let text = ready(&project(), Runner::Pnpm, None, &Style::plain());

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
        let text = ready(&project(), Runner::Npm, None, &Style::plain());

        assert!(text.contains("may not exist"), "{text}");
    }

    #[test]
    fn the_ready_line_carries_no_escape_sequences_when_colour_is_off() {
        assert!(!ready(&project(), Runner::Yarn, None, &Style::plain()).contains('\u{1b}'));
    }

    #[test]
    fn nothing_is_bound_beyond_localhost_unless_sharing_was_asked_for() {
        // The default that matters. An unreleased talk on conference wifi is how
        // that happens without anybody deciding it.
        let flags = vite_flags(&matches_of("dev --port 4000"), None).unwrap();

        assert!(!flags.contains(&"--host".to_string()), "{flags:?}");
    }

    #[test]
    fn sharing_binds_wide_and_pins_the_port_the_link_names() {
        // The link is printed before the server is listening. Without
        // --strictPort, Vite would move to the next free number and leave that
        // printed URL pointing at nothing.
        let flags = vite_flags(&matches_of("dev --crdt --no-open"), Some(&shared_session(false)))
            .expect("flags");

        assert_eq!(flags, ["--port", "5173", "--strictPort", "--host", "0.0.0.0"]);
    }

    #[test]
    fn granting_edit_access_without_sharing_is_refused_rather_than_ignored() {
        // Somebody who typed --allow-edit wanted to share. Doing nothing would
        // leave them believing they had.
        let message = sharing(&matches_of("dev --allow-edit")).expect_err("a refusal");

        assert!(message.contains("--crdt"), "{message}");
    }

    #[test]
    fn asking_for_nothing_shares_nothing() {
        assert_eq!(sharing(&matches_of("dev")).expect("no sharing"), None);
    }

    #[test]
    fn the_ready_line_of_a_shared_deck_prints_the_link_and_says_it_is_read_only() {
        let text = ready(&project(), Runner::Pnpm, Some(&shared_session(false)), &Style::plain());

        assert!(text.contains("slidx dev — shared"), "{text}");
        assert!(text.contains("http://192.168.1.42:5173/__slidx/#s="), "{text}");
        assert!(text.contains("read only"), "{text}");
        assert!(!text.contains("can edit"), "{text}");
    }

    #[test]
    fn the_ready_line_of_an_editable_share_names_both_links() {
        let text = ready(&project(), Runner::Pnpm, Some(&shared_session(true)), &Style::plain());

        assert!(text.contains("read only"), "{text}");
        assert!(text.contains("can edit"), "{text}");
        assert!(text.contains("ctrl-c"), "{text}");
    }

    #[test]
    fn a_shared_ready_line_never_puts_the_secret_before_the_hash() {
        // A URL whose secret is in the path or the query is a URL that reaches an
        // access log. This is the assertion that would catch a link built wrong.
        let shared = shared_session(true);
        let text = ready(&project(), Runner::Pnpm, Some(&shared), &Style::plain());

        for line in text.lines().filter(|line| line.contains("http://")) {
            let url = line.trim();
            let before = url.split('#').next().unwrap_or("");
            assert!(!before.contains(&shared.share.read), "{url}");
            assert!(!before.contains(shared.share.edit.as_deref().unwrap()), "{url}");
        }
    }

    #[test]
    fn a_machine_with_no_network_is_told_rather_than_handed_a_loopback_link() {
        // Printing 127.0.0.1 as though it were shareable would be a link the
        // person next to you cannot open, with no sign of why.
        let message = no_network();

        assert!(message.contains("no address another machine could reach"), "{message}");
        assert!(message.contains("without --crdt still works"), "{message}");
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

        assert!(message.contains("@slidxjs/vite-plugin"), "{message}");
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
