//! Disk space for a recording or an export.
//!
//! Thresholds are absolute, not a percentage of the volume. What a talk needs
//! is a number of bytes — an hour of screen recording, or a PDF of a hundred
//! slides — and that number does not get smaller because the machine has a
//! small disk. Ten percent free on a 4 TB drive is fine; ten percent free on a
//! 128 GB laptop is one export away from a failure.
//!
//! Free space is reported in minutes of recording as well as in bytes, because
//! "6.2 GiB" is not a quantity anyone can judge while standing up and "about
//! 80 minutes of recording" is.

use crate::environment::Environment;
use crate::finding::Finding;

const ID: &str = "disk";

/// 1080p screen capture at roughly 10 Mb/s, which is what the default settings
/// of every common recorder land near.
const RECORDING_BYTES_PER_MINUTE: u64 = 75_000_000;

const GIB: u64 = 1024 * 1024 * 1024;

/// Below this there is not room for a short recording, and the operating
/// system itself starts refusing to swap and to write temporary files — which
/// is a way to lose a talk that has nothing to do with slidx.
const CRITICAL_BYTES: u64 = 2 * GIB;

/// Below this a 45-minute recording plus an export has no margin for a slot
/// that overruns.
const LOW_BYTES: u64 = 8 * GIB;

pub fn check(environment: &Environment) -> Finding {
    let Some(disk) = environment.disk.value() else {
        return Finding::unknown(
            ID,
            format!(
                "free disk space could not be read: {}",
                environment.disk.reason().unwrap_or("no reason given")
            ),
            "check free space by hand before you record anything — an export that runs out of \
             room fails after the audience has gone",
        );
    };

    let free = describe(disk.free_bytes);
    let volume = &disk.volume;

    if disk.free_bytes < CRITICAL_BYTES {
        return Finding::fail(
            ID,
            format!("{free} free on {volume}"),
            "clear space before you start — empty the trash and the Downloads folder. Below a \
             couple of gigabytes the machine itself starts struggling, not just the recording",
        );
    }

    if disk.free_bytes < LOW_BYTES {
        return Finding::warn(
            ID,
            format!("{free} free on {volume}"),
            "enough for a short recording and nothing else. Clear space now if you plan to \
             record, or record to an external drive",
        );
    }

    Finding::pass(ID, format!("{free} free on {volume}"))
}

/// "6.2 GiB (about 80 minutes of 1080p recording)".
fn describe(bytes: u64) -> String {
    let minutes = bytes / RECORDING_BYTES_PER_MINUTE;

    format!("{} ({})", gibibytes(bytes), recording_time(minutes))
}

fn gibibytes(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / GIB as f64)
}

fn recording_time(minutes: u64) -> String {
    match minutes {
        0 => "not enough for a recording at all".to_string(),
        1..=90 => format!("about {minutes} minutes of 1080p recording"),
        // Past an hour and a half the precise number stops carrying
        // information: the answer is "plenty", and a speaker should stop
        // reading the line and move on.
        _ => "hours of 1080p recording".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Disk, Reading};
    use crate::finding::Status;

    fn status_with_free(free_bytes: u64) -> Status {
        let environment =
            Environment::new().with_disk(Reading::known(Disk::new("/", free_bytes, 512 * GIB)));

        check(&environment).status
    }

    #[test]
    fn a_disk_with_room_for_a_talk_and_a_recording_passes() {
        assert_eq!(status_with_free(200 * GIB), Status::Pass);
    }

    #[test]
    fn the_boundary_into_a_warning_is_at_eight_gibibytes() {
        // 45 minutes of recording plus an export, with nothing spare.
        assert_eq!(status_with_free(LOW_BYTES), Status::Pass);
        assert_eq!(status_with_free(LOW_BYTES - 1), Status::Warn);
    }

    #[test]
    fn the_boundary_into_a_failure_is_at_two_gibibytes() {
        // Below this the operating system is in trouble too, not just the
        // recording, so it stops being advice and becomes a blocker.
        assert_eq!(status_with_free(CRITICAL_BYTES), Status::Warn);
        assert_eq!(status_with_free(CRITICAL_BYTES - 1), Status::Fail);
    }

    #[test]
    fn a_full_disk_is_told_specifically_what_to_empty() {
        // "Free up space" is not something a speaker can do in ninety seconds;
        // "empty the trash and Downloads" is.
        let environment =
            Environment::new().with_disk(Reading::known(Disk::new("/", 100_000_000, 512 * GIB)));
        let remedy = check(&environment).remedy.unwrap();

        assert!(remedy.contains("trash"), "got: {remedy}");
    }

    #[test]
    fn a_small_disk_that_is_mostly_full_still_passes_when_the_bytes_are_there() {
        // 12 GiB free on a 512 GiB disk is 2% — and it is enough. A percentage
        // rule would fail this machine for no reason.
        let environment =
            Environment::new().with_disk(Reading::known(Disk::new("/", 12 * GIB, 512 * GIB)));

        assert_eq!(check(&environment).status, Status::Pass);
    }

    #[test]
    fn a_huge_disk_with_almost_nothing_free_still_fails() {
        // 1 GiB free on a 4 TB drive is 0.02% and also just one gigabyte. The
        // absolute rule catches it; a percentage rule would too, but for the
        // wrong reason.
        let environment =
            Environment::new().with_disk(Reading::known(Disk::new("/", GIB, 4096 * GIB)));

        assert_eq!(check(&environment).status, Status::Fail);
    }

    #[test]
    fn free_space_is_reported_in_minutes_of_recording_not_only_in_bytes() {
        // A speaker can judge minutes. Nobody can judge gibibytes standing up.
        let environment =
            Environment::new().with_disk(Reading::known(Disk::new("/", 3 * GIB, 512 * GIB)));
        let detail = check(&environment).detail;

        assert!(detail.contains("minutes"), "got: {detail}");
    }

    #[test]
    fn the_volume_that_was_measured_is_named() {
        // A machine has several. Clearing the wrong one is wasted time the
        // speaker does not have.
        let environment = Environment::new().with_disk(Reading::known(Disk::new(
            "/Volumes/Talks",
            GIB,
            64 * GIB,
        )));

        assert!(check(&environment).detail.contains("/Volumes/Talks"));
    }

    #[test]
    fn a_very_large_free_space_stops_quoting_a_precise_recording_time() {
        // "About 4000 minutes" is a number nobody reads. Past ninety minutes
        // the answer is just "plenty".
        assert_eq!(recording_time(4000), "hours of 1080p recording");
        assert_eq!(recording_time(30), "about 30 minutes of 1080p recording");
        assert_eq!(recording_time(0), "not enough for a recording at all");
    }

    #[test]
    fn an_unreadable_disk_is_unknown_and_never_a_pass() {
        let environment =
            Environment::new().with_disk(Reading::unavailable("df is not on this machine"));
        let finding = check(&environment);

        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.detail.contains("df is not on this machine"));
        assert!(finding.remedy.is_some());
    }
}
