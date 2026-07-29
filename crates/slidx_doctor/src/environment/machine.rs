//! The readings themselves.
//!
//! Plain data with no behaviour beyond arithmetic that has one obviously right
//! answer. Anything that involves a judgement — how little disk is too little,
//! whether being on battery is acceptable — belongs to a check, so that the
//! threshold is stated once, next to the reason for it, and can be moved
//! without touching how the reading was taken.

use serde::{Deserialize, Serialize};

/// Where the machine's electricity is coming from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PowerSource {
    /// Plugged in.
    Ac,
    /// Running down.
    Battery,
}

/// Power state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Power {
    pub source: PowerSource,
    /// Remaining charge, 0-100. `None` on a machine with no battery at all,
    /// which is a fact about the machine rather than a failed reading — a
    /// desktop in a lecture theatre is not a laptop whose battery we could not
    /// see.
    pub charge_percent: Option<u8>,
}

impl Power {
    pub fn on_mains(charge_percent: u8) -> Self {
        Self { source: PowerSource::Ac, charge_percent: Some(charge_percent.min(100)) }
    }

    pub fn on_battery(charge_percent: u8) -> Self {
        Self { source: PowerSource::Battery, charge_percent: Some(charge_percent.min(100)) }
    }

    /// A machine with no battery: a desk tower, or a laptop reporting through a
    /// dock that hides one.
    pub fn mains_only() -> Self {
        Self { source: PowerSource::Ac, charge_percent: None }
    }

    pub fn is_on_battery(self) -> bool {
        self.source == PowerSource::Battery
    }
}

/// Free space on the volume a recording or an export would land on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
    pub free_bytes: u64,
    pub total_bytes: u64,
    /// What was measured, so the speaker knows which volume to clear.
    pub volume: String,
}

impl Disk {
    pub fn new(volume: impl Into<String>, free_bytes: u64, total_bytes: u64) -> Self {
        Self { free_bytes, total_bytes, volume: volume.into() }
    }

    /// Free space as a fraction of the volume, or `None` on a volume that
    /// reports no size — a network mount or a container overlay, where the
    /// fraction would be a made-up number.
    pub fn free_fraction(&self) -> Option<f64> {
        (self.total_bytes > 0).then(|| self.free_bytes as f64 / self.total_bytes as f64)
    }
}

/// The machine's idea of what time zone it is in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clock {
    /// Offset from UTC in minutes, positive east.
    pub utc_offset_minutes: i32,
    /// IANA or platform zone id, when the platform will name it.
    pub zone: Option<String>,
}

impl Clock {
    pub fn at_offset(utc_offset_minutes: i32) -> Self {
        Self { utc_offset_minutes, zone: None }
    }

    pub fn in_zone(utc_offset_minutes: i32, zone: impl Into<String>) -> Self {
        Self { utc_offset_minutes, zone: Some(zone.into()) }
    }

    /// `+09:00`, the way a schedule writes it.
    pub fn offset_label(&self) -> String {
        format_offset(self.utc_offset_minutes)
    }
}

/// Formats a UTC offset the way a conference schedule does.
pub fn format_offset(minutes: i32) -> String {
    let sign = if minutes < 0 { '-' } else { '+' };
    let magnitude = minutes.unsigned_abs();

    format!("{sign}{:02}:{:02}", magnitude / 60, magnitude % 60)
}

/// How far the machine clock is from a reference clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skew {
    /// Machine time minus reference time, in seconds. Positive means the
    /// machine is running ahead.
    pub offset_seconds: i64,
    /// What it was compared against, so the speaker can judge the claim.
    pub reference: String,
}

impl Skew {
    pub fn new(reference: impl Into<String>, offset_seconds: i64) -> Self {
        Self { offset_seconds, reference: reference.into() }
    }

    pub fn magnitude_seconds(&self) -> u64 {
        self.offset_seconds.unsigned_abs()
    }

    /// `ahead` or `behind`, for the message.
    pub fn direction(&self) -> &'static str {
        if self.offset_seconds < 0 {
            "behind"
        } else {
            "ahead of"
        }
    }
}

/// What the network looked like from this machine, one moment ago.
///
/// Two signals rather than one, because they fail apart at venues: a captive
/// portal will happily complete a TCP handshake and then refuse to resolve
/// anything until you have agreed to its terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Network {
    /// A TCP connection to a known address completed.
    pub tcp_reachable: bool,
    /// A hostname resolved.
    pub dns_resolves: bool,
    /// Handshake time, when there was one.
    pub round_trip_ms: Option<u64>,
    /// What was dialled, for the message.
    pub target: String,
}

impl Network {
    pub fn reachable(target: impl Into<String>, round_trip_ms: u64) -> Self {
        Self {
            tcp_reachable: true,
            dns_resolves: true,
            round_trip_ms: Some(round_trip_ms),
            target: target.into(),
        }
    }

    pub fn offline(target: impl Into<String>) -> Self {
        Self {
            tcp_reachable: false,
            dns_resolves: false,
            round_trip_ms: None,
            target: target.into(),
        }
    }

    /// Reachable over TCP but nothing resolves: the shape of a captive portal,
    /// and of a resolver that has quietly stopped answering.
    pub fn captive(target: impl Into<String>, round_trip_ms: u64) -> Self {
        Self {
            tcp_reachable: true,
            dns_resolves: false,
            round_trip_ms: Some(round_trip_ms),
            target: target.into(),
        }
    }
}

/// Process names as the operating system spells them.
///
/// The whole table, unfiltered. Which names matter is a policy that belongs to
/// the check, so that adding a conferencing app to the watch list does not
/// touch the code that reads the process table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunningProcesses {
    names: Vec<String>,
}

impl RunningProcesses {
    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

impl<S: Into<String>> FromIterator<S> for RunningProcesses {
    fn from_iter<T: IntoIterator<Item = S>>(iter: T) -> Self {
        Self { names: iter.into_iter().map(Into::into).collect() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_with_no_battery_is_a_fact_not_a_failed_reading() {
        // A desktop is a legitimate answer to "what is the power state". It has
        // to be distinguishable from "the battery could not be read", because
        // one is fine and the other is not.
        let power = Power::mains_only();

        assert_eq!(power.source, PowerSource::Ac);
        assert_eq!(power.charge_percent, None);
        assert!(!power.is_on_battery());
    }

    #[test]
    fn a_charge_over_a_hundred_percent_is_clamped() {
        // Platforms do report 101 while a battery is calibrating; a percentage
        // that cannot happen makes the message look broken.
        assert_eq!(Power::on_battery(140).charge_percent, Some(100));
    }

    #[test]
    fn a_volume_that_reports_no_size_has_no_fraction_to_report() {
        // Network mounts and container overlays do this. Dividing by zero and
        // printing the result is worse than saying nothing.
        assert_eq!(Disk::new("/", 100, 0).free_fraction(), None);
        assert_eq!(Disk::new("/", 25, 100).free_fraction(), Some(0.25));
    }

    #[test]
    fn offsets_are_written_the_way_a_schedule_writes_them() {
        assert_eq!(format_offset(540), "+09:00");
        assert_eq!(format_offset(-480), "-08:00");
        assert_eq!(format_offset(0), "+00:00");
    }

    #[test]
    fn offsets_that_are_not_whole_hours_survive() {
        // Kathmandu is +05:45 and Chatham Island is +12:45. Rounding either to
        // the hour would report a fault that is not there, or hide one.
        assert_eq!(format_offset(345), "+05:45");
        assert_eq!(format_offset(-210), "-03:30");
    }

    #[test]
    fn skew_says_which_way_the_clock_is_wrong() {
        // "Two minutes out" is not actionable; "two minutes fast" is.
        assert_eq!(Skew::new("ntp", 120).direction(), "ahead of");
        assert_eq!(Skew::new("ntp", -120).direction(), "behind");
        assert_eq!(Skew::new("ntp", -120).magnitude_seconds(), 120);
    }

    #[test]
    fn a_captive_portal_is_reachable_but_resolves_nothing() {
        // The signal that separates "the venue wifi wants a login" from "there
        // is no venue wifi", which are different sentences to a speaker.
        let network = Network::captive("1.1.1.1:443", 12);

        assert!(network.tcp_reachable);
        assert!(!network.dns_resolves);
    }

    #[test]
    fn the_process_table_is_stored_unfiltered() {
        // The check owns the watch list, so the reading must not pre-judge.
        let processes: RunningProcesses = ["Finder", "zoom.us"].into_iter().collect();

        assert_eq!(processes.len(), 2);
        assert_eq!(processes.names(), ["Finder", "zoom.us"]);
    }
}
