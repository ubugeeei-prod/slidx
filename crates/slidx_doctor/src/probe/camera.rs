//! What cameras this machine has.
//!
//! Whether that matters is the check's business. This module only lists what
//! the operating system will admit to, and says why when it will not.
//!
//! Enumerating devices is the only way to answer the question *before* anybody
//! is on stage. A browser could answer it more accurately by opening one, and
//! that is exactly the thing a pre-flight must not do: it would put a
//! permission prompt in front of a speaker minutes before they speak, for a
//! reading. So this is the operating system's own list, which is one step
//! removed from what the browser will actually get and available with no
//! prompt at all.

use std::time::Duration;

use crate::environment::{Cameras, Reading};
#[cfg(any(target_os = "macos", windows))]
use crate::probe::command;

pub fn read(timeout: Duration) -> Reading<Cameras> {
    #[cfg(target_os = "macos")]
    {
        match command::output("system_profiler", &["SPCameraDataType"], timeout) {
            Ok(output) => Reading::known(parse_system_profiler(&output)),
            Err(reason) => Reading::unavailable(reason),
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = timeout;
        read_video4linux()
    }

    #[cfg(windows)]
    {
        // `PNPClass` rather than a name match: a camera is whatever Windows has
        // classified as one, and matching on words like "webcam" would miss
        // every built-in camera whose name is the laptop model.
        const SCRIPT: &str = "Get-CimInstance Win32_PnPEntity \
                              -Filter \"PNPClass='Camera' OR PNPClass='Image'\" | \
                              ForEach-Object { $_.Name }";

        match command::output(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", SCRIPT],
            timeout,
        ) {
            Ok(output) => Reading::known(parse_lines(&output)),
            Err(reason) => Reading::unavailable(reason),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = timeout;
        Reading::unavailable("this platform will not list its cameras")
    }
}

/// Video4Linux devices, read from `sysfs` rather than from a subprocess.
///
/// No command exists on every Linux that would answer this — `v4l2-ctl` is a
/// separate package — and the kernel already publishes the answer as files. A
/// missing directory means no video subsystem rather than a failed reading, but
/// the two are told apart: a kernel without the driver loaded has no directory,
/// which is genuinely "not measured" rather than "no cameras".
#[cfg(all(unix, not(target_os = "macos")))]
fn read_video4linux() -> Reading<Cameras> {
    const ROOT: &str = "/sys/class/video4linux";

    let Ok(entries) = std::fs::read_dir(ROOT) else {
        return Reading::unavailable(format!("{ROOT} could not be read"));
    };

    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read_to_string(entry.path().join("name")).ok())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();

    // A single UVC camera registers several nodes — one for video, one for
    // metadata — under the same name. Counting them would tell a speaker they
    // have three cameras when they have one.
    names.sort();
    names.dedup();

    Reading::known(names.into_iter().collect())
}

/// Parses `system_profiler SPCameraDataType`.
///
/// The report is a tree drawn with indentation: `Camera:` at the left margin,
/// each device indented under it, and that device's properties indented again.
/// A device is therefore a line at exactly one level of nesting — matching on
/// "ends with a colon" alone would collect `Model ID:` as a camera.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_system_profiler(output: &str) -> Cameras {
    const DEVICE_INDENT: usize = 4;

    output
        .lines()
        .filter(|line| line.len() - line.trim_start().len() == DEVICE_INDENT)
        .filter_map(|line| line.trim().strip_suffix(':'))
        .filter(|name| !name.is_empty())
        .collect()
}

/// One name per line, blank lines dropped.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_lines(output: &str) -> Cameras {
    output.lines().map(str::trim).filter(|line| !line.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real report from a laptop with one built-in camera.
    const MAC_REPORT: &str = "Camera:\n\n    FaceTime HD Camera:\n\n      Model ID: \
                              UVC Camera VendorID_1452\n      Unique ID: 0x8020000005ac8514\n\n";

    #[test]
    fn a_camera_report_yields_the_device_and_not_its_properties() {
        // `Model ID:` also ends in a colon. Matching on that alone would report
        // a machine with one camera as having three.
        let cameras = parse_system_profiler(MAC_REPORT);

        assert_eq!(cameras.names(), ["FaceTime HD Camera"]);
    }

    #[test]
    fn a_machine_with_two_cameras_lists_both() {
        let output = "Camera:\n\n    FaceTime HD Camera:\n\n      Model ID: a\n\n    \
                      Studio Display Camera:\n\n      Model ID: b\n";

        assert_eq!(parse_system_profiler(output).len(), 2);
    }

    #[test]
    fn a_machine_with_no_camera_lists_none_rather_than_failing() {
        // A desktop in a lecture theatre. An empty list is an answer, and the
        // check is what decides whether it matters.
        assert!(parse_system_profiler("Camera:\n\n").is_empty());
    }

    #[test]
    fn output_that_is_not_a_report_is_ignored_rather_than_misread() {
        assert!(parse_system_profiler("").is_empty());
        assert!(parse_system_profiler("something went wrong").is_empty());
    }

    #[test]
    fn a_windows_listing_is_one_camera_per_line() {
        let cameras = parse_lines("Integrated Webcam\r\nUSB Video Device\r\n\r\n");

        assert_eq!(cameras.names(), ["Integrated Webcam", "USB Video Device"]);
    }

    #[test]
    fn reading_this_machine_answers_one_way_or_the_other_without_panicking() {
        // The one test that touches the operating system. It cannot assert what
        // this machine has — that is the whole reason the check takes injected
        // readings — only that the probe answers inside its deadline.
        let _ = format!("{:?}", read(Duration::from_secs(5)));
    }
}
