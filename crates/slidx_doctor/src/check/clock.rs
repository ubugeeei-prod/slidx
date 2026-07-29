//! The clock, as two checks.
//!
//! **Which zone the machine thinks it is in** is answerable from the machine
//! alone, and getting it wrong is the classic travelling-speaker failure: the
//! laptop never picked up the new zone, so the calendar reminder, the countdown
//! timer and the shared schedule all disagree with the room by a whole number
//! of hours.
//!
//! **How accurate the clock is** needs something to compare against, and at a
//! venue with no network there is nothing. Keeping the two apart means the zone
//! can still be checked when the skew cannot — folding them into one line would
//! force a single verdict to speak for a reading that was taken and one that
//! was not.

use crate::environment::{machine::format_offset, Clock, Environment};
use crate::finding::Finding;

const ZONE_ID: &str = "clock/zone";
const SKEW_ID: &str = "clock/skew";

/// Below this, nothing a speaker touches will notice.
const TOLERANCE_SECONDS: u64 = 60;

/// Past this, scheduled joins land in the wrong minute and TLS handshakes on a
/// live demo start failing with errors that name certificates rather than the
/// clock — which is the worst kind of failure to debug on stage.
const CRITICAL_SECONDS: u64 = 300;

/// Does the machine agree with the room about what time zone it is in?
pub fn zone(environment: &Environment) -> Finding {
    let Some(clock) = environment.clock.value() else {
        return Finding::unknown(
            ZONE_ID,
            format!(
                "the machine's time zone could not be read: {}",
                environment.clock.reason().unwrap_or("no reason given")
            ),
            "check the clock in the menu bar against the local time here before you trust any \
             timer or calendar reminder",
        );
    };

    let machine = describe_machine_zone(clock);

    let Some(venue_offset) = environment.expected.venue_offset_minutes else {
        // Not a pass. Nothing was compared, and the whole failure mode is a
        // speaker who never compared.
        return Finding::unknown(
            ZONE_ID,
            format!("{machine}, and the deck does not say what zone the talk is scheduled in"),
            "declare the venue's UTC offset in the deck so this can be checked; meanwhile, \
             confirm your start time in the venue's local time rather than your laptop's",
        );
    };

    let venue = describe_venue_zone(venue_offset, environment.expected.venue_zone.as_deref());

    if clock.utc_offset_minutes == venue_offset {
        return Finding::pass(ZONE_ID, format!("{machine}, which matches {venue}"));
    }

    let drift = clock.utc_offset_minutes - venue_offset;

    Finding::fail(
        ZONE_ID,
        format!("{machine}, but the talk is scheduled in {venue} — {} apart", hours(drift)),
        "set the machine to the venue's time zone now. Until you do, every reminder, countdown \
         and shared schedule on this laptop is out by the same amount",
    )
}

/// How far the clock is from a reference clock.
pub fn skew(environment: &Environment) -> Finding {
    let Some(skew) = environment.skew.value() else {
        return Finding::unknown(
            SKEW_ID,
            format!(
                "clock accuracy was not measured: {}",
                environment.skew.reason().unwrap_or("no reason given")
            ),
            "hold your phone next to the laptop and compare — more than a minute out will make a \
             scheduled join land late and can fail TLS on a live demo",
        );
    };

    let magnitude = skew.magnitude_seconds();
    let observed = format!(
        "the machine clock is {} {} {}",
        duration(magnitude),
        skew.direction(),
        skew.reference
    );

    if magnitude <= TOLERANCE_SECONDS {
        return Finding::pass(
            SKEW_ID,
            format!("within {} of {}", duration(TOLERANCE_SECONDS), skew.reference),
        );
    }

    if magnitude < CRITICAL_SECONDS {
        return Finding::warn(
            SKEW_ID,
            observed,
            "turn automatic time synchronisation back on — a clock this far out joins a scheduled \
             stream at visibly the wrong moment",
        );
    }

    Finding::fail(
        SKEW_ID,
        observed,
        "fix the clock before any live demo. At this distance TLS handshakes start failing, and \
         they fail with an error about certificates that will send you looking in the wrong place",
    )
}

fn describe_machine_zone(clock: &Clock) -> String {
    match &clock.zone {
        Some(zone) => format!("the machine is on {} ({zone})", clock.offset_label()),
        None => format!("the machine is on {}", clock.offset_label()),
    }
}

fn describe_venue_zone(offset_minutes: i32, zone: Option<&str>) -> String {
    match zone {
        Some(zone) => format!("{} ({zone})", format_offset(offset_minutes)),
        None => format_offset(offset_minutes),
    }
}

/// "16 hours", "5 hours 45 minutes" — how far apart two zones are, said the way
/// a person says it.
fn hours(minutes: i32) -> String {
    let magnitude = minutes.unsigned_abs();
    let (whole, rest) = (magnitude / 60, magnitude % 60);

    match (whole, rest) {
        (0, rest) => format!("{rest} minutes"),
        (whole, 0) => format!("{whole} hours"),
        (whole, rest) => format!("{whole} hours {rest} minutes"),
    }
}

fn duration(seconds: u64) -> String {
    let (whole, rest) = (seconds / 60, seconds % 60);

    match (whole, rest) {
        (0, rest) => format!("{rest}s"),
        (whole, 0) => format!("{whole}m"),
        (whole, rest) => format!("{whole}m{rest}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Expectation, Reading, Skew};
    use crate::finding::Status;

    fn in_tokyo_scheduled_at(venue_offset: i32) -> Environment {
        Environment::new()
            .with_clock(Reading::known(Clock::in_zone(540, "Asia/Tokyo")))
            .expecting(Expectation::default().at_venue_offset(venue_offset))
    }

    #[test]
    fn a_machine_in_the_venues_zone_passes() {
        assert_eq!(zone(&in_tokyo_scheduled_at(540)).status, Status::Pass);
    }

    #[test]
    fn a_machine_that_flew_in_on_the_wrong_zone_fails() {
        // The travelling speaker's failure: the laptop is still on home time,
        // so every reminder about the talk is hours out.
        let finding = zone(&in_tokyo_scheduled_at(-420));

        assert_eq!(finding.status, Status::Fail);
        assert!(finding.detail.contains("16 hours"), "got: {}", finding.detail);
    }

    #[test]
    fn the_wrong_zone_names_both_sides_so_the_speaker_can_tell_which_is_wrong() {
        let finding = zone(&in_tokyo_scheduled_at(0));

        assert!(finding.detail.contains("+09:00"), "got: {}", finding.detail);
        assert!(finding.detail.contains("+00:00"), "got: {}", finding.detail);
    }

    #[test]
    fn a_half_hour_zone_difference_is_reported_in_hours_and_minutes() {
        // India is +05:30 and Nepal +05:45. Rounding to the hour would report
        // the wrong distance, or none at all.
        let environment = Environment::new()
            .with_clock(Reading::known(Clock::at_offset(330)))
            .expecting(Expectation::default().at_venue_offset(345));

        let finding = zone(&environment);
        assert_eq!(finding.status, Status::Fail);
        assert!(finding.detail.contains("15 minutes"), "got: {}", finding.detail);
    }

    #[test]
    fn a_deck_that_does_not_declare_a_venue_zone_is_unknown_rather_than_a_pass() {
        // Nothing was compared. Reporting a pass here would tell a speaker
        // their zone had been checked when it had not.
        let environment =
            Environment::new().with_clock(Reading::known(Clock::in_zone(540, "Asia/Tokyo")));

        let finding = zone(&environment);
        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.remedy.unwrap().contains("declare"));
    }

    #[test]
    fn an_unreadable_zone_is_unknown_and_never_a_pass() {
        let environment = Environment::new()
            .with_clock(Reading::unavailable("no way to read the zone here"))
            .expecting(Expectation::default().at_venue_offset(540));

        let finding = zone(&environment);
        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.detail.contains("no way to read the zone here"));
    }

    #[test]
    fn the_venue_zone_name_is_used_in_the_message_when_the_deck_gives_one() {
        let environment = Environment::new()
            .with_clock(Reading::known(Clock::at_offset(0)))
            .expecting(Expectation::default().at_venue_offset(540).with_venue_zone("Asia/Tokyo"));

        assert!(zone(&environment).detail.contains("Asia/Tokyo"));
    }

    fn skew_status(offset_seconds: i64) -> Status {
        let environment =
            Environment::new().with_skew(Reading::known(Skew::new("pool.ntp.org", offset_seconds)));

        skew(&environment).status
    }

    #[test]
    fn a_clock_within_a_minute_of_the_reference_passes() {
        assert_eq!(skew_status(0), Status::Pass);
        assert_eq!(skew_status(-30), Status::Pass);
    }

    #[test]
    fn the_boundary_into_a_warning_is_one_minute() {
        // Under a minute nothing a speaker touches notices; over it, a
        // scheduled join is visibly late.
        assert_eq!(skew_status(TOLERANCE_SECONDS as i64), Status::Pass);
        assert_eq!(skew_status(TOLERANCE_SECONDS as i64 + 1), Status::Warn);
    }

    #[test]
    fn the_boundary_into_a_failure_is_five_minutes() {
        assert_eq!(skew_status(CRITICAL_SECONDS as i64 - 1), Status::Warn);
        assert_eq!(skew_status(CRITICAL_SECONDS as i64), Status::Fail);
    }

    #[test]
    fn a_clock_wrong_in_either_direction_is_treated_the_same() {
        // Fast and slow both break a scheduled join; only the wording differs.
        assert_eq!(skew_status(600), Status::Fail);
        assert_eq!(skew_status(-600), Status::Fail);
    }

    #[test]
    fn a_badly_wrong_clock_warns_about_certificates_by_name() {
        // The error a speaker will actually see mentions certificates, not the
        // clock. Naming it here is what stops ten minutes of debugging.
        let environment =
            Environment::new().with_skew(Reading::known(Skew::new("pool.ntp.org", 3600)));

        assert!(skew(&environment).remedy.unwrap().contains("certificates"));
    }

    #[test]
    fn the_skew_message_says_which_way_the_clock_is_wrong() {
        let environment =
            Environment::new().with_skew(Reading::known(Skew::new("pool.ntp.org", -134)));
        let detail = skew(&environment).detail;

        assert!(detail.contains("behind"), "got: {detail}");
        assert!(detail.contains("2m14s"), "got: {detail}");
    }

    #[test]
    fn an_unmeasured_skew_is_unknown_and_the_remedy_takes_five_seconds() {
        // This is the normal state at an offline venue, so the advice has to
        // be something a speaker can do standing at the lectern.
        let environment =
            Environment::new().with_skew(Reading::unavailable("no reference clock reachable"));
        let finding = skew(&environment);

        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.remedy.unwrap().contains("phone"));
    }

    #[test]
    fn durations_are_written_the_way_people_say_them() {
        assert_eq!(duration(45), "45s");
        assert_eq!(duration(60), "1m");
        assert_eq!(duration(134), "2m14s");
    }

    #[test]
    fn zone_distances_are_written_the_way_people_say_them() {
        assert_eq!(hours(960), "16 hours");
        assert_eq!(hours(-960), "16 hours");
        assert_eq!(hours(45), "45 minutes");
        assert_eq!(hours(345), "5 hours 45 minutes");
    }
}
