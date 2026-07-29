//! The machine's time zone, and how far its clock has drifted.
//!
//! `std` knows about instants and about seconds since the epoch, and nothing at
//! all about time zones — so the offset comes from the platform, and the drift
//! comes from asking a time server directly.
//!
//! The NTP query is thirty lines rather than a dependency because it is the
//! only way to answer the question honestly: without a reference clock, a
//! machine cannot tell that it is wrong. It is skippable
//! ([`Request::offline`](crate::probe::Request::offline)) and it degrades to an
//! unavailable reading at a venue with no network — which is the normal case,
//! and why the skew check's remedy is "compare it with your phone".

use std::net::UdpSocket;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::environment::{Clock, Reading, Skew};
#[cfg(any(unix, windows))]
use crate::probe::command;

/// Seconds between the NTP epoch (1900) and the Unix epoch (1970).
const NTP_TO_UNIX: f64 = 2_208_988_800.0;

/// 2^32, the denominator of an NTP fractional second.
const NTP_FRACTION: f64 = 4_294_967_296.0;

pub fn read_zone(timeout: Duration) -> Reading<Clock> {
    #[cfg(unix)]
    {
        let Some(offset) = command::try_output("date", &["+%z"], timeout)
            .and_then(|output| parse_iso_offset(output.trim()))
        else {
            return Reading::unavailable("the machine would not report its UTC offset");
        };

        // The zone *name* is a nicety, so it is read separately and its
        // absence never costs us the offset, which is the part that matters.
        Reading::known(match zone_name_from_localtime() {
            Some(zone) => Clock::in_zone(offset, zone),
            None => Clock::at_offset(offset),
        })
    }

    #[cfg(windows)]
    {
        const SCRIPT: &str = "$t = [TimeZoneInfo]::Local; \
                              '{0} {1}' -f [int]$t.GetUtcOffset([DateTime]::Now).TotalMinutes, $t.Id";

        match command::output(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", SCRIPT],
            timeout,
        ) {
            Ok(output) => match parse_windows_zone(&output) {
                Some(clock) => Reading::known(clock),
                None => Reading::unavailable("Windows reported a time zone this build cannot read"),
            },
            Err(reason) => Reading::unavailable(reason),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = timeout;
        Reading::unavailable("this platform will not report its time zone")
    }
}

/// The IANA zone id, from the symlink every Unix keeps at `/etc/localtime`.
///
/// Only a label — the offset is read separately — so a machine that keeps a
/// copy rather than a symlink simply goes unnamed.
#[cfg_attr(not(unix), allow(dead_code))]
fn zone_name_from_localtime() -> Option<String> {
    let target = std::fs::read_link("/etc/localtime").ok()?;
    let text = target.to_string_lossy();

    text.split_once("zoneinfo/").map(|(_, zone)| zone.to_string())
}

/// Parses `+0900` or `-0330` into minutes east of UTC.
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_iso_offset(text: &str) -> Option<i32> {
    let text = text.trim();
    let (sign, digits) = text.split_at_checked(1)?;

    let sign = match sign {
        "+" => 1,
        "-" => -1,
        _ => return None,
    };

    // Some platforms write `+09:00`; both spellings mean the same thing.
    let digits: String = digits.chars().filter(char::is_ascii_digit).collect();
    if digits.len() != 4 {
        return None;
    }

    let hours = digits.get(0..2)?.parse::<i32>().ok()?;
    let minutes = digits.get(2..4)?.parse::<i32>().ok()?;

    Some(sign * (hours * 60 + minutes))
}

/// Parses "540 Tokyo Standard Time" from the PowerShell one-liner.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_windows_zone(output: &str) -> Option<Clock> {
    let line = output.lines().map(str::trim).find(|line| !line.is_empty())?;
    let (offset, zone) = line.split_once(char::is_whitespace)?;
    let offset = offset.trim().parse::<i32>().ok()?;

    let zone = zone.trim();
    Some(if zone.is_empty() { Clock::at_offset(offset) } else { Clock::in_zone(offset, zone) })
}

/// Asks a time server what time it is.
///
/// Runs on its own thread with a channel deadline as well as a socket read
/// timeout, because resolving the server's hostname has no timeout of its own
/// and a broken resolver at a venue will otherwise sit there for thirty
/// seconds. If that happens the thread is abandoned rather than waited for: a
/// leaked thread that finishes on its own beats a doctor that never prints.
pub fn read_skew(server: Option<&str>, timeout: Duration) -> Reading<Skew> {
    let Some(server) = server else {
        return Reading::unavailable("no reference clock was configured");
    };

    let (sender, receiver) = mpsc::channel();
    let owned = server.to_string();

    thread::spawn(move || {
        let _ = sender.send(query(&owned, timeout));
    });

    // Twice the socket timeout: the socket bounds the wait for a reply, this
    // bounds the wait for everything around it, resolution included.
    match receiver.recv_timeout(timeout * 2) {
        Ok(Ok(offset_seconds)) => Reading::known(Skew::new(server, offset_seconds)),
        Ok(Err(reason)) => Reading::unavailable(reason),
        Err(_) => Reading::unavailable(format!("{server} did not answer in time")),
    }
}

fn query(server: &str, timeout: Duration) -> Result<i64, String> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|error| format!("no socket for a time query ({error})"))?;

    socket
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("the time query could not be bounded ({error})"))?;
    socket.connect(server).map_err(|_| {
        format!("{server} could not be reached — normal at a venue with no network")
    })?;

    // Leap indicator 0, version 3, mode 3 (client). The rest of the packet is
    // zeroes, which is what a client request is.
    let mut request = [0_u8; 48];
    request[0] = 0x1b;

    let sent_at = SystemTime::now();
    let started = Instant::now();
    socket.send(&request).map_err(|error| format!("the time query was not sent ({error})"))?;

    let mut response = [0_u8; 48];
    let received = socket
        .recv(&mut response)
        .map_err(|_| format!("{server} did not reply — normal at a venue with no network"))?;

    if received < 48 {
        return Err(format!("{server} replied with something that is not an NTP packet"));
    }

    let seconds = u32::from_be_bytes([response[40], response[41], response[42], response[43]]);
    let fraction = u32::from_be_bytes([response[44], response[45], response[46], response[47]]);

    let local = sent_at
        // Halfway through the round trip is the moment the server's timestamp
        // describes, near enough. Without it every reading is biased by the
        // latency to the server, which at a venue can be hundreds of
        // milliseconds.
        .checked_add(started.elapsed() / 2)
        .ok_or_else(|| "the machine clock is too far out to compare".to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "the machine clock is set before 1970".to_string())?
        .as_secs_f64();

    Ok((local - ntp_to_unix_seconds(seconds, fraction)).round() as i64)
}

/// Converts an NTP timestamp to seconds since the Unix epoch.
///
/// NTP counts seconds in a 32-bit field that wraps in February 2036. A server
/// answering after that writes a small number, and reading it literally would
/// report a machine as 136 years fast — so a value before the Unix epoch is
/// taken as the second era rather than as an absurd clock.
fn ntp_to_unix_seconds(seconds: u32, fraction: u32) -> f64 {
    let mut whole = f64::from(seconds);
    if whole < NTP_TO_UNIX {
        whole += NTP_FRACTION;
    }

    whole - NTP_TO_UNIX + f64::from(fraction) / NTP_FRACTION
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn an_iso_offset_is_read_as_minutes_east_of_utc() {
        assert_eq!(parse_iso_offset("+0900"), Some(540));
        assert_eq!(parse_iso_offset("-0800"), Some(-480));
        assert_eq!(parse_iso_offset("+0000"), Some(0));
    }

    #[test]
    fn an_offset_that_is_not_a_whole_hour_survives() {
        // Nepal is +05:45. Losing the minutes would report the machine as
        // agreeing with a venue it does not agree with.
        assert_eq!(parse_iso_offset("+0545"), Some(345));
        assert_eq!(parse_iso_offset("-0330"), Some(-210));
    }

    #[test]
    fn an_offset_written_with_a_colon_is_read_the_same_way() {
        // Not every `date` writes it the same. Rejecting one spelling would
        // lose the reading on the platforms that use it.
        assert_eq!(parse_iso_offset("+09:00"), Some(540));
    }

    #[test]
    fn text_that_is_not_an_offset_is_rejected_rather_than_guessed_at() {
        assert_eq!(parse_iso_offset(""), None);
        assert_eq!(parse_iso_offset("JST"), None);
        assert_eq!(parse_iso_offset("+9"), None);
    }

    #[test]
    fn a_windows_time_zone_is_read_with_its_name() {
        let clock = parse_windows_zone("540 Tokyo Standard Time\n").unwrap();

        assert_eq!(clock.utc_offset_minutes, 540);
        assert_eq!(clock.zone.as_deref(), Some("Tokyo Standard Time"));
    }

    #[test]
    fn a_negative_windows_offset_is_read_correctly() {
        // Half the world is west of Greenwich, and a sign error here would
        // report the whole of the Americas as being in the wrong zone.
        let clock = parse_windows_zone("-480 Pacific Standard Time").unwrap();

        assert_eq!(clock.utc_offset_minutes, -480);
    }

    #[test]
    fn windows_output_that_is_not_a_zone_is_rejected() {
        assert!(parse_windows_zone("").is_none());
        assert!(parse_windows_zone("Get-TimeZone : not recognised").is_none());
    }

    #[test]
    fn an_ntp_timestamp_converts_to_unix_seconds() {
        // 3_913_056_000 in NTP seconds is 2024-01-01T00:00:00Z.
        assert_eq!(ntp_to_unix_seconds(3_913_056_000, 0).round() as i64, 1_704_067_200);
    }

    #[test]
    fn an_ntp_fraction_is_worth_less_than_a_second() {
        let half = ntp_to_unix_seconds(3_913_056_000, (NTP_FRACTION / 2.0) as u32);

        assert!((half - 1_704_067_200.5).abs() < 0.001, "got: {half}");
    }

    #[test]
    fn a_timestamp_from_after_the_2036_rollover_is_not_read_as_a_machine_136_years_fast() {
        // The field wraps in February 2036. Read literally, a server answering
        // the day after would make every machine on earth look badly wrong.
        let after_rollover = ntp_to_unix_seconds(1_000, 0);

        assert!(after_rollover > 2_085_000_000.0, "got: {after_rollover}");
    }

    #[test]
    fn skew_with_no_configured_server_is_unavailable_rather_than_zero() {
        // Zero would claim the clock is perfect. Nobody asked anything.
        let reading = read_skew(None, Duration::from_millis(50));

        assert!(!reading.is_known());
        assert!(reading.reason().unwrap().contains("no reference clock"));
    }

    #[test]
    fn a_time_server_that_does_not_exist_gives_up_quickly_instead_of_hanging() {
        // The venue case: DNS is broken or the port is blocked. This must cost
        // one line of the report and a moment, not the whole run.
        let started = Instant::now();
        let reading = read_skew(Some("slidx-no-such-host.invalid:123"), Duration::from_millis(300));

        assert!(!reading.is_known());
        assert!(started.elapsed() < Duration::from_secs(10), "took {:?}", started.elapsed());
    }

    #[test]
    fn reading_this_machines_zone_answers_one_way_or_the_other() {
        let reading = read_zone(Duration::from_secs(5));

        if let Some(clock) = reading.value() {
            // Every real zone on earth is inside this range; anything outside
            // it means the parser has misread something.
            assert!((-12 * 60..=14 * 60).contains(&clock.utc_offset_minutes));
        }
    }
}
