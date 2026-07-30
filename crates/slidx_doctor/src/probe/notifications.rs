//! Whether the machine has been told to stay quiet.
//!
//! The reading the roadmap said could not be taken, and the reason it said so
//! was about the browser: no web API reports Do Not Disturb, and none should,
//! because a page that could read your Focus state could fingerprint you with
//! it. None of that binds a native binary the speaker installed and ran.
//!
//! What does bind it is that each platform answers differently and one barely
//! answers at all:
//!
//! - **macOS** keeps the active Focus in a file under the user's own library.
//!   It is read strictly — the envelope has to be the shape slidx knows, or the
//!   reading is unavailable. Apple has moved this once, and a build that no
//!   longer recognises what it is reading must say so rather than conclude
//!   nothing is on.
//! - **Linux** has no one answer, so the two desktops that do are asked in
//!   turn: GNOME through `gsettings`, Plasma through `kreadconfig`.
//! - **Windows** exposes whether banners are switched off outright, and does
//!   not expose Focus assist. So the "quiet" half is readable and the noisy
//!   half is not, and slidx reports the first and refuses to invent the second.
//!
//! That last one is the shape of this whole line: half an answer, reported as
//! half an answer.

use std::fs;
use std::path::Path;

use crate::environment::{Notifications, Platform, Reading};
use crate::probe::tools::{self, Tools};

/// Where macOS keeps the Focus assertions, under the user's own library.
const MACOS_ASSERTIONS: &str = "Library/DoNotDisturb/DB/Assertions.json";

/// Reads whether banners are switched off entirely. Focus assist is not here
/// and is not reachable from any documented interface, which is why a `1` is
/// not read as "nothing is silencing this machine".
const WINDOWS_TOASTS: &str = "(Get-ItemProperty -Path \
     'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings' \
     -Name NOC_GLOBAL_SETTING_TOASTS_ENABLED -ErrorAction SilentlyContinue)\
     .NOC_GLOBAL_SETTING_TOASTS_ENABLED";

pub fn read(platform: Platform, home: Option<&Path>, tools: &Tools) -> Reading<Notifications> {
    match platform {
        Platform::MacOs => read_macos(home),
        Platform::Linux => read_linux(tools),
        Platform::Windows => tools::parsed(
            tools.output(
                "powershell",
                &["-NoProfile", "-NonInteractive", "-Command", WINDOWS_TOASTS],
            ),
            parse_windows_toasts,
            "Windows does not report whether Focus assist is on, and banners are not switched off",
        ),
        Platform::Unknown => {
            Reading::unavailable("slidx has no way to read the notification state on this platform")
        }
    }
}

/// Reads the file macOS writes the active Focus into.
///
/// A file rather than a command, so it takes the home directory as an argument
/// the way the sysfs battery walk does — a fixture directory is then a whole
/// test, on every platform, rather than a Mac somebody has to put into Do Not
/// Disturb by hand.
fn read_macos(home: Option<&Path>) -> Reading<Notifications> {
    let Some(home) = home else {
        return Reading::unavailable("slidx could not find your home directory");
    };

    match fs::read_to_string(home.join(MACOS_ASSERTIONS)) {
        Ok(text) => match parse_assertions(&text) {
            Some(state) => Reading::known(state),
            None => Reading::unavailable(
                "this macOS records its Focus state in a shape slidx does not recognise",
            ),
        },
        Err(error) => Reading::unavailable(why_unreadable(error.kind())),
    }
}

/// Why macOS would not hand the file over.
///
/// The two answers are different sentences to a speaker and only one of them is
/// about slidx. Recent macOS protects this directory, so a terminal without
/// Full Disk Access is refused rather than told the file is absent — and
/// reporting "there is no Focus state here" to somebody who has one turned on
/// is the guess this whole line exists to refuse.
fn why_unreadable(kind: std::io::ErrorKind) -> &'static str {
    match kind {
        std::io::ErrorKind::PermissionDenied => {
            "macOS will not let slidx read the Focus state — recent versions protect that folder, \
             and a terminal without Full Disk Access is refused"
        }
        std::io::ErrorKind::NotFound => "this macOS keeps no Focus state where slidx looks",
        _ => "this macOS would not hand over the Focus state",
    }
}

/// Asks the two Linux desktops that answer, in turn.
///
/// GNOME first because it is the larger installed base, and one failed lookup
/// costs a process that exits immediately. Neither answering is the honest
/// outcome on a desktop that keeps this somewhere slidx has never heard of.
fn read_linux(tools: &Tools) -> Reading<Notifications> {
    let gnome =
        tools.output("gsettings", &["get", "org.gnome.desktop.notifications", "show-banners"]);

    if let Some(state) = gnome.ok().as_deref().and_then(parse_show_banners) {
        return Reading::known(state);
    }

    for reader in ["kreadconfig6", "kreadconfig5"] {
        let plasma = tools.output(
            reader,
            &["--file", "plasmanotifyrc", "--group", "DoNotDisturb", "--key", "Until"],
        );

        if let Ok(text) = plasma {
            return Reading::known(parse_plasma_until(&text));
        }
    }

    Reading::unavailable("no desktop on this machine reports its notification state to slidx")
}

/// Whether a Focus assertion is live, from the file macOS keeps it in.
///
/// The envelope is checked before anything is concluded, and that is the only
/// reason an absent key may be read as "nothing is on": within a file that is
/// still the shape slidx knows, macOS writes the assertion list only while
/// something is silencing the machine. A file that is no longer that shape
/// yields `None`, and the check reports an unknown rather than a green line.
fn parse_assertions(text: &str) -> Option<Notifications> {
    if !has_envelope(text) {
        return None;
    }

    let Some((_, rest)) = text.split_once("\"storeAssertionRecords\"") else {
        return Some(Notifications::Allowed);
    };

    let value = rest.trim_start().strip_prefix(':')?.trim_start();
    let list = value.strip_prefix('[')?.trim_start();

    Some(if list.starts_with(']') { Notifications::Allowed } else { Notifications::Silenced })
}

/// True when the file still opens the way the one slidx was written against
/// does. Whitespace is dropped first, because the writer is free to indent.
fn has_envelope(text: &str) -> bool {
    let opening: String = text.chars().filter(|c| !c.is_whitespace()).take(9).collect();

    opening == "{\"data\":["
}

/// Parses `gsettings get org.gnome.desktop.notifications show-banners`.
///
/// Banners hidden is what a speaker means by Do Not Disturb, whatever GNOME
/// calls the switch. Anything that is not one of the two literals is not an
/// answer, so it becomes no reading rather than a default.
fn parse_show_banners(output: &str) -> Option<Notifications> {
    match output.trim() {
        "false" => Some(Notifications::Silenced),
        "true" => Some(Notifications::Allowed),
        _ => None,
    }
}

/// Parses Plasma's `DoNotDisturb/Until`.
///
/// The key is written only while the mode is on and removed when it is turned
/// off, so an empty answer from a reader that ran is a real "off" rather than a
/// missing reading — which is why this returns a state and not an option.
fn parse_plasma_until(output: &str) -> Notifications {
    if output.trim().is_empty() {
        Notifications::Allowed
    } else {
        Notifications::Silenced
    }
}

/// Parses the Windows toast switch.
///
/// `0` means banners are off entirely, which is at least as quiet as Focus
/// assist and is a real answer. Anything else means only that banners are not
/// switched off — Focus assist could still be on, and Windows will not say — so
/// it is no reading rather than "notifications will appear".
fn parse_windows_toasts(output: &str) -> Option<Notifications> {
    (output.trim() == "0").then_some(Notifications::Silenced)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOCUS_ON: &str = r#"{"data":[{"storeAssertionRecords":[{"assertionDetails":{}}]}]}"#;
    const FOCUS_OFF: &str = r#"{"data":[{}]}"#;

    fn fixture(name: &str, contents: &str) -> std::path::PathBuf {
        let home = std::env::temp_dir().join(format!("slidx-doctor-focus-{name}"));
        let _ = fs::remove_dir_all(&home);

        let file = home.join(MACOS_ASSERTIONS);
        if let Some(parent) = file.parent() {
            let _ = fs::create_dir_all(parent);
            let _ = fs::write(&file, contents);
        }

        home
    }

    #[test]
    fn every_platforms_notification_reading_is_parsed_on_every_platform() {
        // The seam. A Linux runner drives the Windows registry answer and the
        // macOS file, so neither can quietly stop working between releases.
        let home = fixture("all-platforms", FOCUS_ON);
        let quiet = Tools::answering(|program, _| {
            Ok(match program {
                "gsettings" => "false\n",
                _ => "0\n",
            }
            .to_string())
        });

        for platform in [Platform::MacOs, Platform::Linux, Platform::Windows] {
            let reading = read(platform, Some(&home), &quiet);

            assert_eq!(
                reading.value(),
                Some(&Notifications::Silenced),
                "{platform:?} did not read its own answer"
            );
        }

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn a_platform_slidx_cannot_ask_says_so() {
        let reading = read(Platform::Unknown, None, &Tools::absent());

        assert!(!reading.is_known());
        assert!(reading.reason().is_some_and(|why| why.contains("no way")));
    }

    #[test]
    fn a_live_focus_assertion_reads_as_a_silenced_machine() {
        assert_eq!(parse_assertions(FOCUS_ON), Some(Notifications::Silenced));
    }

    #[test]
    fn a_file_with_no_assertion_in_it_reads_as_a_machine_that_will_interrupt() {
        // macOS writes the assertion list only while something is on, and this
        // is only trusted because the envelope was checked first.
        assert_eq!(parse_assertions(FOCUS_OFF), Some(Notifications::Allowed));
    }

    #[test]
    fn an_empty_assertion_list_is_also_a_machine_that_will_interrupt() {
        let empty = r#"{"data":[{"storeAssertionRecords":[]}]}"#;

        assert_eq!(parse_assertions(empty), Some(Notifications::Allowed));
    }

    #[test]
    fn a_file_that_is_no_longer_the_shape_slidx_knows_yields_no_reading_at_all() {
        // The guard the whole macOS branch rests on. Apple has moved this file
        // once; a build that stopped recognising it must report an unknown
        // rather than announce that nothing is silencing the machine.
        for text in ["{}", "[]", r#"{"assertions":[]}"#, "", "not json"] {
            assert_eq!(parse_assertions(text), None, "{text} was read as an answer");
        }
    }

    #[test]
    fn a_focus_file_that_is_indented_is_still_recognised() {
        // The writer is free to pretty-print it, and a reading that depended on
        // there being no spaces would go dark the day it did.
        let indented = "{\n  \"data\" : [ { } ]\n}";

        assert_eq!(parse_assertions(indented), Some(Notifications::Allowed));
    }

    #[test]
    fn a_mac_with_no_focus_file_where_slidx_looks_reports_unknown() {
        let reading = read_macos(Some(Path::new("/slidx-no-such-home")));

        assert!(!reading.is_known());
        assert!(reading.reason().is_some_and(|why| why.contains("Focus state")));
    }

    #[test]
    fn a_mac_that_refused_the_read_says_so_rather_than_that_there_is_nothing_there() {
        // Recent macOS protects that folder, so a terminal without Full Disk
        // Access is refused. Telling a speaker who has Do Not Disturb switched
        // on that their machine keeps no Focus state is exactly the wrong
        // answer, and the two errors are the only thing that tells them apart.
        let refused = why_unreadable(std::io::ErrorKind::PermissionDenied);
        let absent = why_unreadable(std::io::ErrorKind::NotFound);

        assert!(refused.contains("Full Disk Access"), "got: {refused}");
        assert!(absent.contains("keeps no Focus state"), "got: {absent}");
        assert_ne!(refused, absent);
    }

    #[test]
    fn a_mac_with_no_home_directory_at_all_reports_unknown() {
        assert!(!read_macos(None).is_known());
    }

    #[test]
    fn a_focus_file_read_off_disk_answers_the_same_way_the_parser_does() {
        // The one path that touches the filesystem, driven from a fixture so it
        // runs on every platform rather than on a Mac somebody put into Do Not
        // Disturb by hand.
        let home = fixture("on-disk", FOCUS_ON);
        let reading = read_macos(Some(&home));
        let _ = fs::remove_dir_all(&home);

        assert_eq!(reading.value(), Some(&Notifications::Silenced));
    }

    #[test]
    fn gnome_hiding_its_banners_is_what_a_speaker_means_by_do_not_disturb() {
        assert_eq!(parse_show_banners("false\n"), Some(Notifications::Silenced));
        assert_eq!(parse_show_banners("true\n"), Some(Notifications::Allowed));
    }

    #[test]
    fn a_gsettings_answer_that_is_neither_literal_is_not_an_answer() {
        // A missing schema, an error on standard output, a future value. None
        // of them may become a verdict.
        assert_eq!(parse_show_banners("no such key"), None);
        assert_eq!(parse_show_banners(""), None);
    }

    #[test]
    fn plasma_writes_the_key_only_while_the_mode_is_on() {
        assert_eq!(parse_plasma_until("+4294967295-12-31T23:59:59\n"), Notifications::Silenced);
        assert_eq!(parse_plasma_until("\n"), Notifications::Allowed);
    }

    #[test]
    fn plasma_is_asked_when_gnome_is_not_installed() {
        // A KDE machine has no gsettings schema. Falling through to the second
        // desktop is what makes the Linux answer worth having at all.
        let kde = Tools::answering(|program, _| match program {
            "gsettings" => Err("`gsettings` could not be run".to_string()),
            _ => Ok("+4294967295-12-31T23:59:59\n".to_string()),
        });

        assert_eq!(read_linux(&kde).value(), Some(&Notifications::Silenced));
    }

    #[test]
    fn a_linux_desktop_that_neither_tool_knows_reports_unknown_rather_than_allowed() {
        let reading = read_linux(&Tools::absent());

        assert!(!reading.is_known());
        assert!(reading.reason().is_some_and(|why| why.contains("no desktop")));
    }

    #[test]
    fn windows_banners_switched_off_entirely_is_a_real_answer() {
        assert_eq!(parse_windows_toasts("0\n"), Some(Notifications::Silenced));
    }

    #[test]
    fn windows_with_banners_on_will_not_say_whether_focus_assist_is_running() {
        // The honest half-answer. Reading a `1` as "notifications will appear"
        // would report a speaker who turned Focus assist on as unprotected, and
        // the correction for that is silence rather than a guess.
        assert_eq!(parse_windows_toasts("1\n"), None);
        assert_eq!(parse_windows_toasts(""), None);
    }

    #[test]
    fn a_windows_machine_that_will_not_say_explains_which_half_was_readable() {
        let tools = Tools::answering(|_, _| Ok("1\n".to_string()));
        let reading = read(Platform::Windows, None, &tools);

        assert!(reading.reason().is_some_and(|why| why.contains("Focus assist")), "{reading:?}");
    }
}
