//! What is running, and who is in charge of it.
//!
//! Separated from the rest of `version` because it answers a different question.
//! Everything else here manages a directory of installed versions; this one
//! looks at the process it is inside and reports the truth about it, including
//! the truth that the version manager is not involved.
//!
//! That is the hour everybody loses at least once. `slidx version use` reports
//! success, `slidx --version` does not change, and nothing ever explains that
//! `npm i -g slidx` from six months ago sits earlier on the PATH. Every fix is
//! then applied to the wrong binary. A version manager that cannot say it is
//! not in charge is worse than none, because it invites you to trust it.
//!
//! So the report is not "the version is X". It is the file that is running,
//! resolved through symlinks; the channel that put it there; whether anything
//! here can change it; and what else on the PATH would win.
//!
//! See [`super::provenance`] for how the channel is worked out, and why that is
//! a pure function of readings rather than a pile of `cfg!`.

use std::env;
use std::path::PathBuf;

use crate::args::Matches;
use crate::home::Home;
use crate::report::{self, INDENT};
use crate::style::{Ink, Style};
use crate::Outcome;

use super::pin;
use super::provenance::{self, Channel, Provenance, Reading};
use super::store::Store;

/// `slidx version current`.
pub fn run(home: &Home, store: &Store, matches: &Matches, style: &Style) -> Outcome {
    let found = look(home);
    let here = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let wanted = pin::resolve(&here, &home.default_version());

    if matches.is_set("json") {
        return match serde_json::to_string_pretty(&describe(&found, &wanted, store)) {
            Ok(json) => Outcome::out(format!("{json}\n")),
            Err(error) => Outcome::misuse(format!("could not serialise the report: {error}\n")),
        };
    }

    Outcome::out(render(&found, &wanted, store, style))
}

/// Reads this machine.
///
/// The one function here that talks to the operating system; everything it
/// learns goes into a [`Reading`] and every conclusion is drawn from that.
pub fn look(home: &Home) -> Provenance {
    let exe = env::current_exe()
        .ok()
        // Through the symlink, so a managed binary reports the version
        // directory it really lives in rather than the shim pointing at it.
        .and_then(|path| path.canonicalize().ok().or(Some(path)));

    let path_var = env::var("PATH").unwrap_or_default();
    let entries: Vec<PathBuf> =
        env::split_paths(&path_var).filter(|entry| !entry.as_os_str().is_empty()).collect();

    provenance::of(&Reading {
        exe,
        home: home.root().to_path_buf(),
        on_path: provenance::on_path(&path_var, cfg!(windows), |candidate| candidate.is_file()),
        path: entries,
    })
}

/// The version this binary is, when it is a managed one.
pub fn running_version(home: &Home) -> Option<String> {
    match look(home).channel {
        Channel::Managed { version } => Some(version),
        _ => None,
    }
}

/// The whole report, as a person reads it.
///
/// Pure, so a shadowed npm install and a managed one are each one line of test
/// setup rather than a machine somebody has to physically arrange.
pub fn render(found: &Provenance, wanted: &pin::Pin, store: &Store, style: &Style) -> String {
    let mut text = format!("{}\n\n", style.paint(Ink::Strong, "slidx version"));

    text.push_str(&format!("  {}  {}\n", style.pad(Ink::Strong, "running", 9), crate::version()));
    text.push_str(&format!(
        "  {}  {}\n",
        style.pad(Ink::Faint, "binary", 9),
        found
            .exe
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "unknown".into())
    ));
    text.push_str(&format!(
        "  {}  {}\n",
        style.pad(Ink::Faint, "channel", 9),
        channel_name(&found.channel)
    ));
    text.push_str(&format!("  {}  {}\n", style.pad(Ink::Faint, "asked for", 9), asked_for(wanted)));

    text.push('\n');
    text.push_str(&verdict(found, wanted, store, style));

    text
}

/// The line that says whether any of this is under the version manager's
/// control — and the warnings that matter when it is not.
fn verdict(found: &Provenance, wanted: &pin::Pin, store: &Store, style: &Style) -> String {
    let mut text = String::new();

    if !found.channel.is_managed() {
        text.push_str(&report::block(
            "NOTE",
            Ink::Warn,
            "not managed",
            "`slidx version use` will not change what runs. This binary was not \
             installed by the version manager.",
            Some(found.channel.how_to_change()),
            style,
        ));
    }

    // The hour this whole module exists to give back.
    if let Some(shadow) = &found.shadowed_by {
        text.push_str(&report::block(
            "FIRST",
            Ink::Fail,
            "shadowed on PATH",
            &format!(
                "{} comes before {} on your PATH, so it is what runs however this \
                 command is used.",
                shadow.display(),
                store.shim().display()
            ),
            Some("remove that one, or put the slidx bin directory ahead of it"),
            style,
        ));
    }

    if found.channel.is_managed() && !found.bin_on_path {
        text.push_str(&report::block(
            "NOTE",
            Ink::Warn,
            "bin not on PATH",
            "`slidx version use` writes into a directory no shell looks in, so \
             switching versions will appear to do nothing.",
            Some("add the slidx bin directory to your PATH"),
            style,
        ));
    }

    // A pin naming a version nobody installed is a silent no-op otherwise.
    if let Some(version) = wanted.version() {
        if !store.is_installed(version) {
            text.push_str(&report::block(
                "MISS",
                Ink::Warn,
                "not installed",
                &format!("{} asks for {version}, which is not installed.", wanted.source()),
                Some(&format!("slidx version install {version}")),
                style,
            ));
        }
    }

    if text.is_empty() {
        text.push_str(&report::flowed(
            "The version manager is in charge of this binary.",
            INDENT,
            Ink::Pass,
            style,
        ));
    }

    text
}

fn asked_for(wanted: &pin::Pin) -> String {
    match wanted.version() {
        Some(version) => format!("{version}  ({})", wanted.source()),
        None => wanted.source(),
    }
}

pub fn channel_name(channel: &Channel) -> String {
    match channel {
        Channel::Managed { version } => format!("the version manager ({version})"),
        Channel::ShellInstall => "install.sh, unmanaged".to_string(),
        Channel::Npm => "npm".to_string(),
        Channel::Cargo => "cargo install".to_string(),
        Channel::System { manager } => manager.to_string(),
        Channel::Elsewhere => "unknown".to_string(),
    }
}

/// The `--json` shape.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Description {
    running: &'static str,
    binary: Option<String>,
    channel: String,
    managed: bool,
    bin_on_path: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    shadowed_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    asked_for: Option<String>,
    asked_by: String,
    asked_for_installed: bool,
}

fn describe(found: &Provenance, wanted: &pin::Pin, store: &Store) -> Description {
    Description {
        running: crate::version(),
        binary: found.exe.as_ref().map(|path| path.display().to_string()),
        channel: channel_name(&found.channel),
        managed: found.channel.is_managed(),
        bin_on_path: found.bin_on_path,
        shadowed_by: found.shadowed_by.as_ref().map(|path| path.display().to_string()),
        asked_for: wanted.version().map(str::to_string),
        asked_by: wanted.source(),
        asked_for_installed: wanted.version().is_some_and(|version| store.is_installed(version)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::new("/nowhere/versions", "/nowhere/bin")
    }

    fn managed(version: &str) -> Provenance {
        Provenance {
            exe: Some(PathBuf::from(format!("/home/somebody/.slidx/versions/{version}/slidx"))),
            channel: Channel::Managed { version: version.into() },
            bin_on_path: true,
            shadowed_by: None,
        }
    }

    fn unmanaged() -> Provenance {
        Provenance {
            exe: Some(PathBuf::from("/usr/local/bin/slidx")),
            channel: Channel::Npm,
            bin_on_path: false,
            shadowed_by: None,
        }
    }

    fn shadowed() -> Provenance {
        Provenance { shadowed_by: Some(PathBuf::from("/usr/local/bin/slidx")), ..managed("0.3.0") }
    }

    #[test]
    fn it_names_the_file_that_is_actually_running() {
        // Not "the version is X" — the path, resolved, so somebody can look at
        // it. Every other line follows from this one.
        let text = render(&managed("0.3.0"), &pin::Pin::Unpinned, &store(), &Style::plain());

        assert!(text.contains("/home/somebody/.slidx/versions/0.3.0/slidx"), "{text}");
    }

    #[test]
    fn it_says_which_channel_installed_what_is_running() {
        assert!(
            render(&unmanaged(), &pin::Pin::Unpinned, &store(), &Style::plain()).contains("npm")
        );
    }

    #[test]
    fn it_says_plainly_when_the_version_manager_is_not_in_charge() {
        // The single most important sentence this command prints. Without it,
        // `version use` reporting success is an invitation to trust something
        // that is not going to happen.
        let text = render(&unmanaged(), &pin::Pin::Unpinned, &store(), &Style::plain());

        assert!(text.contains("not managed"), "{text}");
        assert!(text.contains("will not change what runs"), "{text}");
        assert!(text.contains("npm owns this one"), "{text}");
    }

    #[test]
    fn it_says_so_when_the_version_manager_is_in_charge() {
        let text = render(&managed("0.3.0"), &pin::Pin::Unpinned, &store(), &Style::plain());

        assert!(text.contains("in charge"), "{text}");
        assert!(!text.contains("not managed"), "{text}");
    }

    #[test]
    fn it_reports_another_slidx_that_comes_first_on_the_path() {
        // The hour. `version use` works, `slidx --version` does not change,
        // and this is the only line that would ever have explained it.
        let text = render(&shadowed(), &pin::Pin::Unpinned, &store(), &Style::plain());

        assert!(text.contains("shadowed on PATH"), "{text}");
        assert!(text.contains("/usr/local/bin/slidx"), "{text}");
        assert!(text.contains("however this command is used"), "{text}");
    }

    #[test]
    fn it_reports_a_managed_bin_directory_that_is_not_on_the_path() {
        // The other half of the same trap: `use` writes a link into a
        // directory no shell ever looks in.
        let off_path = Provenance { bin_on_path: false, ..managed("0.3.0") };
        let text = render(&off_path, &pin::Pin::Unpinned, &store(), &Style::plain());

        assert!(text.contains("bin not on PATH"), "{text}");
        assert!(text.contains("appear to do nothing"), "{text}");
    }

    #[test]
    fn it_reports_a_pin_asking_for_a_version_nobody_installed() {
        // Otherwise the pin is a silent no-op, which is the one thing a pin
        // must never be.
        let wanted = pin::Pin::Project {
            version: "0.9.9".into(),
            file: PathBuf::from("/talks/vueconf/.slidx-version"),
        };
        let text = render(&managed("0.3.0"), &wanted, &store(), &Style::plain());

        assert!(text.contains("not installed"), "{text}");
        assert!(text.contains("slidx version install 0.9.9"), "{text}");
    }

    #[test]
    fn it_names_the_file_a_pin_came_from() {
        // A surprising version becomes one question to answer rather than a
        // hunt through every directory above you.
        let wanted = pin::Pin::Project {
            version: "0.9.9".into(),
            file: PathBuf::from("/talks/vueconf/.slidx-version"),
        };
        let text = render(&managed("0.3.0"), &wanted, &store(), &Style::plain());

        assert!(text.contains("/talks/vueconf/.slidx-version"), "{text}");
    }

    #[test]
    fn it_says_what_asks_for_a_version_when_nothing_does() {
        let text = render(&managed("0.3.0"), &pin::Pin::Unpinned, &store(), &Style::plain());

        assert!(text.contains("nothing pins a version"), "{text}");
    }

    #[test]
    fn a_report_lines_up_the_same_coloured_and_plain() {
        let plain = render(&shadowed(), &pin::Pin::Unpinned, &store(), &Style::plain());
        let colored = render(&shadowed(), &pin::Pin::Unpinned, &store(), &Style::colored());

        assert_eq!(plain.lines().count(), colored.lines().count());
        assert!(!plain.contains('\u{1b}'));
    }

    #[test]
    fn no_line_of_a_report_runs_past_the_fixed_width() {
        let text = render(&shadowed(), &pin::Pin::Unpinned, &store(), &Style::plain());

        for line in text.lines() {
            // The binary path is one token and is never wrapped: a path broken
            // across two lines cannot be copied.
            if line.contains('/') && line.split_whitespace().count() <= 2 {
                continue;
            }
            assert!(
                line.chars().count() <= crate::style::WIDTH,
                "{} cols: {line}",
                line.chars().count()
            );
        }
    }

    #[test]
    fn the_json_report_carries_whether_the_manager_is_in_charge() {
        // A machine-readable answer to the only question that matters, so a
        // setup script can check it rather than parse prose.
        let json = serde_json::to_string(&describe(&unmanaged(), &pin::Pin::Unpinned, &store()))
            .expect("json");

        assert!(json.contains("\"managed\":false"), "{json}");
        assert!(json.contains("\"channel\":\"npm\""), "{json}");
    }

    #[test]
    fn the_json_report_names_a_shadowing_binary_when_there_is_one() {
        let json = serde_json::to_string(&describe(&shadowed(), &pin::Pin::Unpinned, &store()))
            .expect("json");

        assert!(json.contains("shadowedBy"), "{json}");
    }

    #[test]
    fn the_json_report_omits_a_shadow_that_is_not_there_rather_than_saying_null() {
        let json =
            serde_json::to_string(&describe(&managed("0.3.0"), &pin::Pin::Unpinned, &store()))
                .expect("json");

        assert!(!json.contains("shadowedBy"), "{json}");
    }
}
