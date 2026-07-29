//! Free space on the volume the deck lives on.
//!
//! `std` has no filesystem-statistics call, so this asks the platform's own
//! tool. The parsers below are compiled everywhere, not only on the platform
//! whose output they read, so that the Windows parser is exercised by a test
//! run on macOS — a parser that can only be tested on the machine that can
//! produce its input is a parser nobody has tested.

use std::path::Path;
use std::time::Duration;

use crate::environment::{Disk, Reading};
#[cfg(any(unix, windows))]
use crate::probe::command;

pub fn read(workspace: &Path, timeout: Duration) -> Reading<Disk> {
    let path = workspace.to_string_lossy().to_string();

    #[cfg(unix)]
    {
        // `-P` forces one filesystem per line. Without it a long device name
        // wraps onto a second line and the columns move.
        match command::output("df", &["-kP", &path], timeout) {
            Ok(output) => match parse_df(&output) {
                Some(disk) => Reading::known(disk),
                None => Reading::unavailable("`df` produced output this build cannot read"),
            },
            Err(reason) => Reading::unavailable(reason),
        }
    }

    #[cfg(windows)]
    {
        match command::output(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", &drive_script(&path)],
            timeout,
        ) {
            Ok(output) => match parse_drive_free(&output, &path) {
                Some(disk) => Reading::known(disk),
                None => Reading::unavailable("Windows reported no size for this volume"),
            },
            Err(reason) => Reading::unavailable(reason),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, timeout);
        Reading::unavailable("this platform has no way to report free disk space")
    }
}

/// PowerShell that prints "free total" for the volume holding a path.
#[cfg_attr(not(windows), allow(dead_code))]
fn drive_script(path: &str) -> String {
    // Single quotes are the literal-string form; doubling one escapes it, so a
    // directory with an apostrophe in its name cannot break out of the string.
    let quoted = path.replace('\'', "''");

    format!("$d = (Get-Item -LiteralPath '{quoted}').PSDrive; '{{0}} {{1}}' -f $d.Free, ($d.Free + $d.Used)")
}

/// Parses `df -kP`, whose blocks are 1024 bytes by definition of `-k`.
///
/// Reads the last line rather than the second: `df` on a path prints one
/// filesystem, but some platforms prepend a warning line, and the row that
/// describes the requested path is always the last.
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_df(output: &str) -> Option<Disk> {
    let line = output.lines().rfind(|line| !line.trim().is_empty())?;
    let fields: Vec<&str> = line.split_whitespace().collect();

    if fields.len() < 6 {
        return None;
    }

    // A header row reaches here too and is rejected by the parse below, which
    // is why the numbers are read before anything else is trusted.
    let total = fields[1].parse::<u64>().ok()? * 1024;
    let free = fields[3].parse::<u64>().ok()? * 1024;
    // A mount point can contain spaces, so everything after the capacity
    // column belongs to it.
    let mount = fields[5..].join(" ");

    Some(Disk::new(mount, free, total))
}

/// Parses "free total" from the PowerShell one-liner above.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_drive_free(output: &str, volume: &str) -> Option<Disk> {
    let line = output.lines().map(str::trim).find(|line| !line.is_empty())?;
    let (free, total) = line.split_once(char::is_whitespace)?;

    let free = free.trim().parse::<u64>().ok()?;
    let total = total.trim().parse::<u64>().ok()?;

    Some(Disk::new(volume, free, total))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const MACOS_DF: &str = "\
Filesystem   1024-blocks      Used Available Capacity  Mounted on
/dev/disk3s5   971350180 120000000  20000000    86%    /System/Volumes/Data
";

    const LINUX_DF: &str = "\
Filesystem     1024-blocks     Used Available Capacity Mounted on
/dev/nvme0n1p2   479079112 60000000  25000000      71% /
";

    #[test]
    fn macos_df_output_is_read_in_bytes() {
        let disk = parse_df(MACOS_DF).unwrap();

        assert_eq!(disk.free_bytes, 20_000_000 * 1024);
        assert_eq!(disk.total_bytes, 971_350_180 * 1024);
        assert_eq!(disk.volume, "/System/Volumes/Data");
    }

    #[test]
    fn linux_df_output_is_read_the_same_way() {
        // Same parser, and this test runs on macOS too — which is the point of
        // keeping the parsers free of platform gates.
        let disk = parse_df(LINUX_DF).unwrap();

        assert_eq!(disk.free_bytes, 25_000_000 * 1024);
        assert_eq!(disk.volume, "/");
    }

    #[test]
    fn a_mount_point_containing_spaces_survives() {
        // External drives are routinely called things like "Talks Backup", and
        // stopping at the first space would report the wrong volume name.
        let output = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
                      /dev/disk4s1 1000 500 500 50% /Volumes/Talks Backup\n";

        assert_eq!(parse_df(output).unwrap().volume, "/Volumes/Talks Backup");
    }

    #[test]
    fn a_header_with_no_rows_is_not_mistaken_for_a_reading() {
        // Reporting zero bytes free would fail the disk check on a machine
        // that simply did not answer.
        let output = "Filesystem 1024-blocks Used Available Capacity Mounted on\n";

        assert!(parse_df(output).is_none());
    }

    #[test]
    fn output_that_is_not_df_at_all_is_rejected() {
        assert!(parse_df("").is_none());
        assert!(parse_df("permission denied").is_none());
    }

    #[test]
    fn windows_drive_output_is_read_as_free_and_total() {
        let disk = parse_drive_free("21474836480 512000000000\n", "C:\\").unwrap();

        assert_eq!(disk.free_bytes, 21_474_836_480);
        assert_eq!(disk.total_bytes, 512_000_000_000);
        assert_eq!(disk.volume, "C:\\");
    }

    #[test]
    fn windows_output_that_did_not_produce_numbers_is_rejected() {
        // `Get-PSDrive` returns nothing for a drive it cannot see, and an
        // empty line must not parse as a full disk.
        assert!(parse_drive_free("", "C:\\").is_none());
        assert!(parse_drive_free("Get-Item : Cannot find path", "C:\\").is_none());
    }

    #[test]
    fn a_path_with_an_apostrophe_cannot_break_out_of_the_powershell_string() {
        // `~/Talks/Ann's deck` is an ordinary directory name. Left unescaped it
        // would end the string literal and change what the command does.
        let script = drive_script("C:\\Users\\Ann's deck");

        assert!(script.contains("Ann''s deck"), "got: {script}");
    }

    #[test]
    fn reading_this_machine_answers_one_way_or_the_other() {
        // Cannot assert a number — it is a different one on every machine —
        // only that the probe answers rather than hanging or panicking.
        let reading = read(Path::new("."), Duration::from_secs(5));

        if let Some(disk) = reading.value() {
            assert!(disk.total_bytes > 0, "a real volume has a size");
        }
    }
}
