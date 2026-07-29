//! What is running.
//!
//! The whole table, unfiltered — which application names matter is the check's
//! business, not this module's. Keeping the split means adding a conferencing
//! app to the watch list never touches the code that reads a process table, and
//! the watch list stays testable without a machine that happens to be running
//! the thing.

use std::time::Duration;

use crate::environment::{Reading, RunningProcesses};
use crate::probe::command;

pub fn read(timeout: Duration) -> Reading<RunningProcesses> {
    #[cfg(unix)]
    {
        // `-o comm=` prints the command name with no header, which is all the
        // watch list needs and a fraction of the output of a full listing.
        match command::output("ps", &["-A", "-o", "comm="], timeout) {
            Ok(output) => Reading::known(parse_ps(&output)),
            Err(reason) => Reading::unavailable(reason),
        }
    }

    #[cfg(windows)]
    {
        match command::output("tasklist", &["/fo", "csv", "/nh"], timeout) {
            Ok(output) => Reading::known(parse_tasklist(&output)),
            Err(reason) => Reading::unavailable(reason),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = timeout;
        Reading::unavailable("this platform will not list its running processes")
    }
}

/// Parses `ps -A -o comm=`: one name per line, no header.
///
/// Linux truncates the name to fifteen characters here, which is a real limit
/// on what the watch list can match and the reason its entries are short.
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_ps(output: &str) -> RunningProcesses {
    output.lines().map(str::trim).filter(|line| !line.is_empty()).collect()
}

/// Parses `tasklist /fo csv /nh`, taking the first quoted field of each row.
///
/// Quoted CSV rather than the default table format because an image name can
/// contain spaces, and the table format gives no way to tell where the name
/// ends and the padding begins.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_tasklist(output: &str) -> RunningProcesses {
    output
        .lines()
        .filter_map(|line| line.trim().strip_prefix('"'))
        .filter_map(|line| line.split_once('"'))
        .map(|(name, _)| name)
        .filter(|name| !name.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn ps_output_is_one_process_per_line() {
        let output = "/usr/sbin/cfprefsd\n/Applications/zoom.us.app/Contents/MacOS/zoom.us\nps\n";
        let processes = parse_ps(output);

        assert_eq!(processes.len(), 3);
        assert_eq!(processes.names()[2], "ps");
    }

    #[test]
    fn blank_lines_in_ps_output_are_dropped() {
        // A blank name would normalise to an empty key and match nothing, but
        // it would also inflate the count in any future message.
        assert_eq!(parse_ps("\n  \nFinder\n").len(), 1);
    }

    #[test]
    fn tasklist_csv_yields_the_image_name_of_each_row() {
        let output = "\"explorer.exe\",\"1234\",\"Console\",\"1\",\"90,000 K\"\n\
                      \"Teams.exe\",\"5678\",\"Console\",\"1\",\"400,000 K\"\n";
        let processes = parse_tasklist(output);

        assert_eq!(processes.names(), ["explorer.exe", "Teams.exe"]);
    }

    #[test]
    fn a_tasklist_image_name_containing_a_space_survives() {
        // The reason the CSV format is asked for rather than the default
        // table: a name with a space cannot be split out of a padded column.
        let processes = parse_tasklist("\"Some App.exe\",\"1\",\"Console\",\"1\",\"1 K\"\n");

        assert_eq!(processes.names(), ["Some App.exe"]);
    }

    #[test]
    fn tasklist_output_that_is_not_csv_is_ignored_rather_than_misread() {
        // `tasklist` prints an error line when it cannot query, and reading it
        // as a process name would put nonsense in the report.
        assert!(parse_tasklist("ERROR: The search filter cannot be recognized.").is_empty());
        assert!(parse_tasklist("").is_empty());
    }

    #[test]
    fn reading_this_machine_lists_at_least_this_test_process() {
        // The one thing that is certainly running is this test binary, so an
        // empty successful reading means the parser has misread something.
        let reading = read(Duration::from_secs(10));

        if let Some(processes) = reading.value() {
            assert!(!processes.is_empty(), "a running machine has processes");
        }
    }
}
