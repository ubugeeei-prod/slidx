//! `slidx version` — several slidx versions, side by side.
//!
//! A talk is rehearsed against one version and given against whatever is
//! installed a month later. That gap is the whole reason this exists: a deck
//! that lints clean in March and fails in April, twenty minutes before a talk,
//! is the failure mode, and a `.slidx-version` file next to the deck is the
//! fix.
//!
//! ## The four things it does
//!
//! `current` says what is running and who is in charge of it. `list` says what
//! is installed. `install` fetches a version and verifies it. `use` points the
//! shim at one.
//!
//! ## `current` is the important one
//!
//! Not because it does the most, but because it is the only defence against the
//! hour everybody loses at least once: `version use` reports success, `slidx
//! --version` does not change, and nothing ever explains that an npm install
//! from six months ago sits earlier on the PATH. So `current` reports the file
//! that is *actually running*, the channel that put it there, and — in as many
//! words — whether this command has any say in the matter at all.
//!
//! It lives in [`current`], apart from the rest, because it answers a different
//! question: everything else here manages a directory of installed versions,
//! and that one looks at the process it is inside.

pub mod current;
pub mod download;
pub mod pin;
pub mod provenance;
pub mod store;

use std::env;
use std::fs;
use std::path::Path;

use crate::args::Matches;
use crate::home::Home;
use crate::report::{self, INDENT};
use crate::style::{Ink, Style};
use crate::{Outcome, FOUND, MISUSE, OK};

use store::Store;

/// Dispatches one of the subcommands.
pub fn run(action: &str, matches: &Matches, style: &Style) -> Outcome {
    let home = Home::discover();
    let store = Store::new(home.versions(), home.bin());

    match action {
        "current" => current::run(&home, &store, matches, style),
        "list" => list(&home, &store, matches, style),
        "install" => install(&home, &store, matches, style),
        "use" => choose(&home, &store, matches, style),
        "remove" => remove(&home, &store, matches),
        other => Outcome::misuse(format!("`{other}` is declared but not wired up.\n")),
    }
}

/// Installed versions, marking the one in use and the one running.
fn list(home: &Home, store: &Store, matches: &Matches, style: &Style) -> Outcome {
    let installed = store.installed();
    let running = current::running_version(home);
    let default = pin::read(&home.default_version());

    if matches.is_set("json") {
        return match serde_json::to_string_pretty(&installed) {
            Ok(json) => Outcome::out(format!("{json}\n")),
            Err(error) => Outcome::misuse(format!("could not serialise the list: {error}\n")),
        };
    }

    if installed.is_empty() {
        return Outcome::out(format!(
            "{}\n\n  {}\n\n  {}\n",
            style.paint(Ink::Strong, "slidx version list"),
            style.paint(Ink::Faint, "No versions are installed under the version manager."),
            "slidx version install 0.1.0"
        ));
    }

    let mut text = format!("{}\n\n", style.paint(Ink::Strong, "slidx version list"));

    for version in &installed {
        // Two independent facts, and they can disagree — which is exactly the
        // situation somebody needs to see spelled out rather than inferred.
        let mut marks = Vec::new();
        if default.as_deref() == Some(version.as_str()) {
            marks.push("default");
        }
        if running.as_deref() == Some(version.as_str()) {
            marks.push("running");
        }

        let marker = if marks.is_empty() { " " } else { "*" };
        let note =
            if marks.is_empty() { String::new() } else { format!("  ({})", marks.join(", ")) };

        text.push_str(&format!(
            "  {} {}{}\n",
            style.paint(if marks.is_empty() { Ink::Faint } else { Ink::Pass }, marker),
            style.paint(Ink::Strong, version),
            style.paint(Ink::Faint, note)
        ));
    }

    Outcome::out(text)
}

/// Makes a version the one in use.
///
/// Both halves, always, in one place. The link is what the shell finds and the
/// record is what survives it being clobbered — and when `install --use` wrote
/// only the first, `version list` stopped marking the default and `version
/// remove` stopped refusing to delete it. Two ways to select a version is two
/// chances for them to disagree.
fn select(home: &Home, store: &Store, version: &str) -> std::io::Result<()> {
    store.select(version)?;
    pin::write(&home.default_version(), version)
}

/// Downloads a version and verifies it.
fn install(home: &Home, store: &Store, matches: &Matches, style: &Style) -> Outcome {
    let Some(version) = wanted_version(matches) else {
        return Outcome::misuse(needs_a_version("install"));
    };

    if download::target().is_empty() {
        return Outcome::misuse(no_prebuilt_binary());
    }

    if store.is_installed(&version) && !matches.is_set("force") {
        return Outcome::out(format!(
            "{version} is already installed.\n\n  slidx version use {version}\n"
        ));
    }

    let asset = download::asset_name(download::target());
    let base = download::release_url(&version);
    let scratch = store.root().join(format!(".downloading-{version}"));
    let _ = fs::remove_dir_all(&scratch);

    let archive = scratch.join(&asset);
    if let Err(problem) = download::fetch(&format!("{base}/{asset}"), &archive) {
        let _ = fs::remove_dir_all(&scratch);
        return Outcome::misuse(format!("{}\n", problem.message()));
    }

    let sums = scratch.join(download::CHECKSUM_FILE);
    if let Err(problem) = download::fetch(&format!("{base}/{}", download::CHECKSUM_FILE), &sums) {
        let _ = fs::remove_dir_all(&scratch);
        return Outcome::misuse(format!("{}\n", problem.message()));
    }

    // Read back rather than kept in memory from the fetch: the bytes that are
    // verified have to be the bytes that were written, or the check is of
    // something other than what will be installed.
    let bytes = fs::read(&archive).unwrap_or_default();
    let checksums = fs::read_to_string(&sums).unwrap_or_default();

    if let Err(problem) = download::verify(&bytes, &checksums, &asset) {
        let _ = fs::remove_dir_all(&scratch);
        return Outcome {
            stderr: format!("{}\n", problem.message()),
            code: FOUND,
            ..Outcome::default()
        };
    }

    // Only past verification does anything land in `versions/`. Unpacking
    // first would leave an unverified binary somewhere `use` could find it.
    let destination = store.directory(&version);
    let _ = fs::remove_dir_all(&destination);

    if let Err(problem) = download::unpack(&archive, &destination) {
        let _ = fs::remove_dir_all(&scratch);
        let _ = fs::remove_dir_all(&destination);
        return Outcome::misuse(format!("{}\n", problem.message()));
    }

    let _ = download::make_executable(&store.binary(&version));
    let _ = fs::remove_dir_all(&scratch);

    if !store.is_installed(&version) {
        let _ = fs::remove_dir_all(&destination);
        return Outcome::misuse(format!(
            "the archive for {version} did not contain a slidx binary\n"
        ));
    }

    let mut text = format!(
        "{}\n\n{}",
        style.paint(Ink::Strong, format!("slidx {version} installed")),
        report::flowed("checksum verified against the release", INDENT, Ink::Pass, style)
    );

    if matches.is_set("use") {
        return match select(home, store, &version) {
            Ok(()) => {
                text.push_str(&report::flowed("and now in use", INDENT, Ink::Pass, style));
                Outcome::out(text)
            }
            Err(error) => {
                Outcome::misuse(format!("installed, but could not switch to it: {error}\n"))
            }
        };
    }

    text.push_str(&report::flowed(
        &format!("slidx version use {version}"),
        INDENT,
        Ink::Faint,
        style,
    ));

    Outcome::out(text)
}

/// Points the shim at an installed version.
fn choose(home: &Home, store: &Store, matches: &Matches, style: &Style) -> Outcome {
    let Some(version) = wanted_version(matches) else {
        return Outcome::misuse(needs_a_version("use"));
    };

    if !store.is_installed(&version) {
        return Outcome::misuse(format!(
            "{version} is not installed.\n\n  slidx version install {version}\n\n\
             `slidx version list` shows what is.\n"
        ));
    }

    if let Err(error) = select(home, store, &version) {
        return Outcome::misuse(format!("could not switch to {version}: {error}\n"));
    }

    let found = current::look(home);
    let mut text =
        format!("{}\n\n", style.paint(Ink::Strong, format!("slidx {version} is now the default")));
    text.push_str(&report::flowed(
        &format!("{} -> {}", store.shim().display(), store.binary(&version).display()),
        INDENT,
        Ink::Faint,
        style,
    ));

    // Saying it worked without saying it will not take effect is the lie this
    // whole module is built to avoid.
    if let Some(shadow) = &found.shadowed_by {
        text.push('\n');
        text.push_str(&report::block(
            "FIRST",
            Ink::Fail,
            "shadowed on PATH",
            &format!(
                "{} still comes first, so `slidx` will keep running that one.",
                shadow.display()
            ),
            Some("remove it, or put the slidx bin directory ahead of it"),
            style,
        ));
    } else if !found.bin_on_path {
        text.push('\n');
        text.push_str(&report::block(
            "NOTE",
            Ink::Warn,
            "bin not on PATH",
            &format!(
                "{} is not on your PATH, so nothing will pick this up.",
                store.shim().display()
            ),
            Some(&format!("export PATH=\"{}:$PATH\"", home.bin().display())),
            style,
        ));
    }

    Outcome::out(text)
}

fn remove(home: &Home, store: &Store, matches: &Matches) -> Outcome {
    let Some(version) = wanted_version(matches) else {
        return Outcome::misuse(needs_a_version("remove"));
    };

    // Removing what the shim points at would leave a `slidx` on the PATH that
    // reports "command not found" from a directory that exists.
    if pin::read(&home.default_version()).as_deref() == Some(version.as_str()) {
        return Outcome::misuse(format!(
            "{version} is the version in use, so removing it would leave the shim \
             pointing at nothing.\n\nSwitch to another one first:\n\n  slidx version list\n"
        ));
    }

    match store.remove(&version) {
        Ok(()) => Outcome::out(format!("removed slidx {version}\n")),
        Err(error) => Outcome::misuse(format!("could not remove {version}: {error}\n")),
    }
}

/// The version named on the command line, with a leading `v` forgiven.
fn wanted_version(matches: &Matches) -> Option<String> {
    matches
        .first_positional()
        .map(|version| version.trim_start_matches('v').to_string())
        .filter(|version| !version.is_empty())
}

fn needs_a_version(action: &str) -> String {
    format!(
        "`slidx version {action}` needs a version.\n\n  slidx version {action} 0.1.0\n\n\
         `slidx version list` shows what is installed.\n"
    )
}

fn no_prebuilt_binary() -> String {
    format!(
        "slidx does not publish a prebuilt binary for {}-{}.\n\n\
         Build it from source instead:\n\n  cargo install slidx_cli\n",
        env::consts::OS,
        env::consts::ARCH
    )
}

/// Where a pin would be read from, for a caller that wants to say so.
pub fn pin_for(directory: &Path, home: &Home) -> pin::Pin {
    pin::resolve(directory, &home.default_version())
}

/// Exit code for a `version` subcommand that could not do what it was asked.
pub const COULD_NOT: u8 = MISUSE;
/// Exit code for a verification that failed — the download ran and was refused.
pub const REFUSED: u8 = FOUND;
/// Everything worked.
pub const DONE: u8 = OK;
#[cfg(test)]
mod tests {
    use super::*;

    fn matches_for(line: &str) -> Matches {
        let argv: Vec<String> = line.split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    #[test]
    fn a_version_argument_forgives_the_leading_v_that_tags_are_written_with() {
        assert_eq!(
            wanted_version(&matches_for("version install v0.3.0")).as_deref(),
            Some("0.3.0")
        );
        assert_eq!(wanted_version(&matches_for("version install 0.3.0")).as_deref(), Some("0.3.0"));
    }

    #[test]
    fn install_and_use_without_a_version_say_what_they_wanted() {
        assert!(wanted_version(&matches_for("version install")).is_none());

        for action in ["install", "use", "remove"] {
            let message = needs_a_version(action);
            assert!(message.contains(&format!("slidx version {action} 0.1.0")), "{message}");
        }
    }

    #[test]
    fn a_platform_with_no_prebuilt_binary_is_pointed_at_cargo() {
        assert!(no_prebuilt_binary().contains("cargo install slidx_cli"));
    }

    #[test]
    fn every_subcommand_the_table_declares_is_dispatched_to_something() {
        // The one way this match and the table can drift. Reaching the
        // fallback arm would be a command that parses and then does nothing.
        let parent = crate::command::find("version").expect("version is a command");

        for child in parent.subcommands {
            assert!(!matches!(child.name, ""), "{} has no name", child.name);
            assert!(
                ["current", "list", "install", "use", "remove"].contains(&child.name),
                "`version {}` is declared but this module does not dispatch it",
                child.name
            );
        }
    }
}
