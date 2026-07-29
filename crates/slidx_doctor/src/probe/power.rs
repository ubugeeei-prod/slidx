//! Whether the machine is plugged in.
//!
//! Three completely different sources: `pmset` on macOS, sysfs on Linux, and a
//! CIM query on Windows. Linux needs no subprocess at all — the kernel exposes
//! the battery as files — which is worth the extra branch, because it is the
//! one reading here that can be taken with no process startup cost.

use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::environment::{Power, PowerSource, Reading};
// Linux answers from the filesystem, so it starts no subprocess and needs no
// command runner.
#[cfg(any(target_os = "macos", windows))]
use crate::probe::command;

pub fn read(timeout: Duration) -> Reading<Power> {
    #[cfg(target_os = "macos")]
    {
        match command::output("pmset", &["-g", "batt"], timeout) {
            Ok(output) => match parse_pmset(&output) {
                Some(power) => Reading::known(power),
                None => Reading::unavailable("`pmset` reported no battery this build can read"),
            },
            Err(reason) => Reading::unavailable(reason),
        }
    }

    #[cfg(target_os = "linux")]
    {
        let _ = timeout;
        read_sysfs(Path::new("/sys/class/power_supply"))
    }

    #[cfg(windows)]
    {
        const SCRIPT: &str = "$b = Get-CimInstance Win32_Battery | Select-Object -First 1; \
                              if ($b) { '{0} {1}' -f $b.BatteryStatus, $b.EstimatedChargeRemaining } \
                              else { 'none' }";

        match command::output(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", SCRIPT],
            timeout,
        ) {
            Ok(output) => match parse_win32_battery(&output) {
                Some(power) => Reading::known(power),
                None => {
                    Reading::unavailable("Windows reported a battery state this build cannot read")
                }
            },
            Err(reason) => Reading::unavailable(reason),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = timeout;
        Reading::unavailable("this platform has no battery interface slidx knows about")
    }
}

/// Reads the kernel's own view, one file per fact.
///
/// Takes the directory as an argument so the walk can be tested against a
/// fixture rather than against whatever battery the test machine happens to
/// have — which on CI is usually none.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn read_sysfs(root: &Path) -> Reading<Power> {
    let Ok(entries) = fs::read_dir(root) else {
        return Reading::unavailable("the kernel exposes no power supplies here");
    };

    let mut seen_mains = false;

    for entry in entries.flatten() {
        let path = entry.path();
        let kind = read_trimmed(&path.join("type")).unwrap_or_default();

        if kind == "Mains" {
            seen_mains = true;
            continue;
        }

        if kind != "Battery" {
            continue;
        }

        let status = read_trimmed(&path.join("status")).unwrap_or_default();
        let capacity = read_trimmed(&path.join("capacity"));

        return match parse_sysfs_battery(&status, capacity.as_deref()) {
            Some(power) => Reading::known(power),
            None => Reading::unavailable("the kernel reported a battery with no readable state"),
        };
    }

    if seen_mains {
        // A power supply that is not a battery, and no battery beside it: a
        // desktop. That is an answer, not a failed reading.
        return Reading::known(Power::mains_only());
    }

    Reading::unavailable("the kernel lists no battery or mains supply")
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|text| text.trim().to_string())
}

/// Turns a sysfs `status` and `capacity` pair into a reading.
///
/// `Unknown` is a real value the kernel writes, and it must not be read as
/// "discharging" — a machine reported as running down when it is plugged in
/// would put a permanent warning on a lectern desktop.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_sysfs_battery(status: &str, capacity: Option<&str>) -> Option<Power> {
    let charge = capacity.and_then(|capacity| capacity.parse::<u8>().ok());

    let source = match status {
        "Discharging" => PowerSource::Battery,
        "Charging" | "Full" | "Not charging" => PowerSource::Ac,
        // Includes the literal "Unknown" and anything a future kernel adds.
        _ => return None,
    };

    Some(Power { source, charge_percent: charge.map(|charge| charge.min(100)) })
}

/// Parses `pmset -g batt`.
///
/// The first line names the source, a later line carries the percentage:
///
/// ```text
/// Now drawing from 'Battery Power'
///  -InternalBattery-0 (id=1234567)    78%; discharging; 2:41 remaining present: true
/// ```
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_pmset(output: &str) -> Option<Power> {
    let source = if output.contains("'AC Power'") {
        PowerSource::Ac
    } else if output.contains("'Battery Power'") {
        PowerSource::Battery
    } else {
        return None;
    };

    Some(Power { source, charge_percent: first_percentage(output) })
}

/// The first `NN%` in a block of text.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn first_percentage(output: &str) -> Option<u8> {
    output.split('%').next().and_then(|before| {
        // Read backwards from the percent sign: the digits immediately in
        // front of it are the number, whatever precedes them.
        let reversed: String = before.chars().rev().take_while(char::is_ascii_digit).collect();
        let digits: String = reversed.chars().rev().collect();

        digits.parse::<u8>().ok().map(|percent| percent.min(100))
    })
}

/// Parses the `Win32_Battery` one-liner: a status code and a percentage, or
/// the literal `none` on a machine with no battery.
///
/// Status 2 is "AC power"; 1 is "discharging". The rest are charging states of
/// one kind or another, which all mean a cable is attached.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_win32_battery(output: &str) -> Option<Power> {
    let line = output.lines().map(str::trim).find(|line| !line.is_empty())?;

    if line.eq_ignore_ascii_case("none") {
        return Some(Power::mains_only());
    }

    let (status, charge) = line.split_once(char::is_whitespace)?;
    let charge = charge.trim().parse::<u8>().ok().map(|charge| charge.min(100));

    let source = match status.trim() {
        "1" => PowerSource::Battery,
        "2" => PowerSource::Ac,
        // 3-5 are charging, 6-11 are charging variants and error states that
        // still mean a cable. An unrecognised code is not guessed at.
        "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" | "11" => PowerSource::Ac,
        _ => return None,
    };

    Some(Power { source, charge_percent: charge })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const ON_BATTERY: &str = "Now drawing from 'Battery Power'\n \
        -InternalBattery-0 (id=12345)\t78%; discharging; 2:41 remaining present: true\n";

    const ON_MAINS: &str = "Now drawing from 'AC Power'\n \
        -InternalBattery-0 (id=12345)\t100%; charged; 0:00 remaining present: true\n";

    #[test]
    fn pmset_on_battery_is_read_with_its_charge() {
        let power = parse_pmset(ON_BATTERY).unwrap();

        assert_eq!(power.source, PowerSource::Battery);
        assert_eq!(power.charge_percent, Some(78));
    }

    #[test]
    fn pmset_on_mains_is_read_as_plugged_in() {
        let power = parse_pmset(ON_MAINS).unwrap();

        assert_eq!(power.source, PowerSource::Ac);
        assert_eq!(power.charge_percent, Some(100));
    }

    #[test]
    fn pmset_output_that_names_no_source_is_rejected() {
        // Better an unknown than a guess: reporting "on mains" for output we
        // did not understand is the one answer that could lose a talk.
        assert!(parse_pmset("").is_none());
        assert!(parse_pmset("Now drawing from 'UPS Power'").is_none());
    }

    #[test]
    fn sysfs_discharging_is_read_as_running_on_battery() {
        let power = parse_sysfs_battery("Discharging", Some("64")).unwrap();

        assert_eq!(power.source, PowerSource::Battery);
        assert_eq!(power.charge_percent, Some(64));
    }

    #[test]
    fn sysfs_charging_and_full_both_mean_a_cable_is_attached() {
        for status in ["Charging", "Full", "Not charging"] {
            let power = parse_sysfs_battery(status, Some("100")).unwrap();
            assert_eq!(power.source, PowerSource::Ac, "{status} was misread");
        }
    }

    #[test]
    fn the_kernels_literal_unknown_status_is_not_read_as_discharging() {
        // "Unknown" is a value the kernel really writes. Treating it as
        // discharging would put a permanent warning on a docked machine.
        assert!(parse_sysfs_battery("Unknown", Some("100")).is_none());
        assert!(parse_sysfs_battery("", None).is_none());
    }

    #[test]
    fn a_battery_with_no_readable_capacity_still_reports_its_source() {
        // Half a reading is worth having: being unplugged is actionable even
        // when the percentage is not.
        let power = parse_sysfs_battery("Discharging", None).unwrap();

        assert_eq!(power.source, PowerSource::Battery);
        assert_eq!(power.charge_percent, None);
    }

    #[test]
    fn a_machine_with_only_a_mains_supply_reads_as_a_desktop() {
        let directory = std::env::temp_dir().join("slidx-doctor-power-mains");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(directory.join("AC")).unwrap();
        fs::write(directory.join("AC/type"), "Mains\n").unwrap();

        let reading = read_sysfs(&directory);
        let _ = fs::remove_dir_all(&directory);

        assert_eq!(reading.value().map(|power| power.source), Some(PowerSource::Ac));
        assert_eq!(reading.value().and_then(|power| power.charge_percent), None);
    }

    #[test]
    fn a_sysfs_directory_that_does_not_exist_is_unavailable_rather_than_a_guess() {
        let reading = read_sysfs(Path::new("/slidx-no-such-power-supply"));

        assert!(!reading.is_known());
        assert!(reading.reason().is_some());
    }

    #[test]
    fn a_sysfs_battery_directory_is_preferred_over_the_mains_shortcut() {
        let directory = std::env::temp_dir().join("slidx-doctor-power-battery");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(directory.join("BAT0")).unwrap();
        fs::write(directory.join("BAT0/type"), "Battery\n").unwrap();
        fs::write(directory.join("BAT0/status"), "Discharging\n").unwrap();
        fs::write(directory.join("BAT0/capacity"), "41\n").unwrap();

        let reading = read_sysfs(&directory);
        let _ = fs::remove_dir_all(&directory);

        assert_eq!(reading.value().map(|power| power.charge_percent), Some(Some(41)));
    }

    #[test]
    fn windows_status_two_means_plugged_in_and_one_means_discharging() {
        assert_eq!(parse_win32_battery("2 100").unwrap().source, PowerSource::Ac);
        assert_eq!(parse_win32_battery("1 55").unwrap().source, PowerSource::Battery);
        assert_eq!(parse_win32_battery("1 55").unwrap().charge_percent, Some(55));
    }

    #[test]
    fn windows_reporting_no_battery_reads_as_a_desktop() {
        // A tower in a lecture theatre. An unavailable reading here would put
        // a permanent amber line on every venue machine.
        assert_eq!(parse_win32_battery("none\n").unwrap(), Power::mains_only());
    }

    #[test]
    fn an_unrecognised_windows_status_code_is_not_guessed_at() {
        assert!(parse_win32_battery("99 50").is_none());
        assert!(parse_win32_battery("").is_none());
    }

    #[test]
    fn reading_this_machine_answers_one_way_or_the_other() {
        let reading = read(Duration::from_secs(5));

        // Whatever this machine is, the probe has to produce something the
        // check can report, without hanging or panicking.
        let _ = format!("{reading:?}");
    }
}
