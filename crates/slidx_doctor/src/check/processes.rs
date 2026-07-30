//! What else on this machine can take the screen.
//!
//! Informational, and it never fails: a speaker running a hybrid talk *wants*
//! the conferencing app open, and a doctor that calls that a fault is a doctor
//! whose red lines get ignored. The job here is to say what is running and what
//! it can do, and let the speaker decide.
//!
//! The watch list is deliberately short. A list of everything that could
//! conceivably pop a window would fire on every machine, and a check that
//! always fires teaches the speaker to skip the line — which is worse than not
//! having it. Slack is the clearest example and is deliberately absent: it is
//! running on essentially every laptop, so flagging it would cost the whole
//! check its credibility.

use crate::environment::Environment;
use crate::finding::Finding;

const ID: &str = "screen-capture";

/// Things that record or stream the screen.
///
/// Normalised spellings — lowercase, letters and digits only — so that
/// `zoom.us`, `Zoom.exe` and `zoom` all reduce to the same key.
const RECORDERS: &[(&str, &str)] = &[
    ("obs", "OBS"),
    ("obs64", "OBS"),
    ("screenflow", "ScreenFlow"),
    ("camtasia", "Camtasia"),
    ("quicktimeplayer", "QuickTime Player"),
    ("screenstudio", "Screen Studio"),
    ("loom", "Loom"),
    ("kap", "Kap"),
    ("simplescreenrecorder", "SimpleScreenRecorder"),
    ("screenrec", "ScreenRec"),
];

/// Things that can start sharing your screen, or put a window in front of it.
const CONFERENCING: &[(&str, &str)] = &[
    ("zoom", "Zoom"),
    ("zoomus", "Zoom"),
    ("teams", "Microsoft Teams"),
    ("msteams", "Microsoft Teams"),
    ("webex", "Webex"),
    ("webexmta", "Webex"),
    ("discord", "Discord"),
    ("gotomeeting", "GoToMeeting"),
    ("bluejeans", "BlueJeans"),
];

pub fn check(environment: &Environment) -> Finding {
    let Some(processes) = environment.processes.value() else {
        return Finding::unknown(
            ID,
            format!(
                "the running processes could not be listed: {}",
                environment.processes.reason().unwrap_or("no reason given")
            ),
            "glance at the menu bar or system tray for a recording indicator, and quit anything \
             that can share your screen unless this talk is being streamed",
        );
    };

    let found = watched(processes.names());

    if found.is_empty() {
        return Finding::pass(ID, "nothing known to record or share the screen is running");
    }

    Finding::warn(
        ID,
        format!("running: {}", found.join(", ")),
        "quit these unless the talk is being streamed — a conferencing app can pull focus, pop a \
         join prompt, or start sharing the wrong window in the middle of a demo",
    )
}

/// Watched applications currently running, by display name, without repeats.
///
/// Deduplicated because a single application shows up in the process table
/// several times — helpers, renderers, a crash reporter — and a finding that
/// says "Zoom, Zoom, Zoom" reads like a bug.
fn watched(names: &[String]) -> Vec<&'static str> {
    matching(RECORDERS.iter().chain(CONFERENCING), names)
}

/// Running conferencing applications, by display name.
///
/// Public because a conferencing app is also the most likely reason a webcam
/// will not open, and the camera check needs to name it. It reads this list
/// rather than keeping a second one: an application added here has to be known
/// to both checks at once, or one of them quietly stops being right.
pub fn conferencing(names: &[String]) -> Vec<&'static str> {
    matching(CONFERENCING.iter(), names)
}

fn matching<'a>(
    watch_list: impl Iterator<Item = &'a (&'static str, &'static str)> + Clone,
    names: &[String],
) -> Vec<&'static str> {
    let mut found = Vec::new();

    for name in names {
        let key = normalise(name);

        if let Some((_, label)) = watch_list.clone().find(|(marker, _)| *marker == key) {
            if !found.contains(label) {
                found.push(*label);
            }
        }
    }

    found
}

/// Reduces a process name to a comparable key.
///
/// Process tables disagree wildly: macOS gives an absolute path to a binary
/// inside a bundle, Windows gives `Name.exe`, Linux gives a name truncated to
/// fifteen characters. Taking the last path segment, dropping an extension and
/// keeping letters and digits collapses all three onto the same key.
fn normalise(name: &str) -> String {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = file.rsplit_once('.').map_or(file, |(stem, extension)| {
        // `zoom.us` is a name, `zoom.exe` is a name plus an extension. Only
        // strip what is actually an executable suffix.
        if matches!(extension.to_ascii_lowercase().as_str(), "exe" | "app" | "bin") {
            stem
        } else {
            file
        }
    });

    stem.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Reading, RunningProcesses};
    use crate::finding::Status;

    fn running(names: &[&str]) -> Environment {
        Environment::new()
            .with_processes(Reading::known(names.iter().copied().collect::<RunningProcesses>()))
    }

    #[test]
    fn an_ordinary_desktop_passes() {
        let environment = running(&["Finder", "kernel_task", "/usr/bin/ssh", "code"]);

        assert_eq!(check(&environment).status, Status::Pass);
    }

    #[test]
    fn a_running_conferencing_app_is_reported_by_name() {
        // The speaker has to decide whether it should be running. That is only
        // possible if the finding says which application it means.
        let environment = running(&["Finder", "/Applications/zoom.us.app/Contents/MacOS/zoom.us"]);
        let finding = check(&environment);

        assert_eq!(finding.status, Status::Warn);
        assert!(finding.detail.contains("Zoom"), "got: {}", finding.detail);
    }

    #[test]
    fn a_screen_recorder_is_reported_too() {
        let environment = running(&["obs"]);

        assert!(check(&environment).detail.contains("OBS"));
    }

    #[test]
    fn this_check_never_fails_because_a_hybrid_talk_needs_these_apps() {
        // A speaker streaming their talk has Zoom open on purpose. Calling
        // that a failure is how a doctor's red lines stop being read.
        for names in
            [vec!["zoom"], vec!["obs", "zoom", "teams"], vec!["Camtasia", "discord", "webex"]]
        {
            assert_eq!(check(&running(&names)).status, Status::Warn);
        }
    }

    #[test]
    fn slack_is_deliberately_not_on_the_watch_list() {
        // It runs on every machine. A check that fires every time is a check
        // the speaker learns to skip, which costs the lines that matter.
        let environment = running(&["Slack", "Slack Helper"]);

        assert_eq!(check(&environment).status, Status::Pass);
    }

    #[test]
    fn one_application_with_several_helper_processes_is_named_once() {
        // "Zoom, Zoom, Zoom" reads like a bug and buries the second app.
        let environment = running(&["zoom.us", "zoom.us", "ZoomUs", "obs"]);
        let detail = check(&environment).detail;

        assert_eq!(detail.matches("Zoom").count(), 1, "got: {detail}");
        assert!(detail.contains("OBS"));
    }

    #[test]
    fn a_windows_process_table_matches_the_same_watch_list() {
        // Windows reports `Name.exe`; the executable suffix must not stop the
        // match, or the check silently does nothing on Windows.
        let environment = running(&["Teams.exe", "explorer.exe"]);

        assert!(check(&environment).detail.contains("Microsoft Teams"));
    }

    #[test]
    fn a_dot_in_an_application_name_is_not_mistaken_for_an_extension() {
        // macOS really does name the binary `zoom.us`. Stripping `.us` as if
        // it were a file extension would leave `zoom`, which happens to still
        // match — this pins the intent rather than the accident.
        assert_eq!(normalise("zoom.us"), "zoomus");
        assert_eq!(normalise("obs64.exe"), "obs64");
        assert_eq!(normalise("/Applications/OBS.app/Contents/MacOS/OBS"), "obs");
        assert_eq!(normalise("C:\\Program Files\\Zoom\\bin\\Zoom.exe"), "zoom");
    }

    #[test]
    fn every_watch_list_entry_is_stored_already_normalised() {
        // Markers are compared against normalised process names, so an entry
        // written as `zoom.us` or `MS Teams` would never match anything and
        // the check would quietly do nothing.
        for (marker, label) in RECORDERS.iter().chain(CONFERENCING) {
            assert_eq!(&normalise(marker), marker, "{marker} is not in normal form");
            assert!(!label.is_empty());
        }
    }

    #[test]
    fn a_screen_recorder_is_not_a_reason_a_camera_will_not_open() {
        // The two checks share one watch list and read different halves of it.
        // OBS records a screen; it does not hold a webcam, and telling a
        // speaker to quit it because their camera failed would be wrong.
        let running: Vec<String> = ["obs", "zoom.us"].iter().map(|n| n.to_string()).collect();

        assert_eq!(conferencing(&running), ["Zoom"]);
    }

    #[test]
    fn an_unreadable_process_table_is_unknown_and_never_a_pass() {
        let environment =
            Environment::new().with_processes(Reading::unavailable("ps is not available"));
        let finding = check(&environment);

        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.detail.contains("ps is not available"));
        assert!(finding.remedy.is_some());
    }
}
