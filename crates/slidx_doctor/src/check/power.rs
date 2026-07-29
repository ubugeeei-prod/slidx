//! Power.
//!
//! The only check here with an opinion the reading itself does not support:
//! **being on battery at all is a warning, at any charge.** A machine at 85%
//! looks fine and is not — a projector over USB-C, an external display, a
//! screen recorder and a video call between them drain a laptop several times
//! faster than the estimate in the menu bar, which was measured while the
//! speaker read email. The talk that dies at what the speaker remembers as 60%
//! is the normal shape of this failure, and the fix is five seconds of cable.

use crate::environment::{Environment, PowerSource};
use crate::finding::Finding;

const ID: &str = "power";

/// Below this, the machine may not survive a slot even plugged in, because
/// charging from flat while driving a display is slower than draining.
const CRITICAL_PERCENT: u8 = 20;

/// Below this, a 45-minute slot with a demo is genuinely at risk rather than
/// merely unwise.
const LOW_PERCENT: u8 = 50;

pub fn check(environment: &Environment) -> Finding {
    let power =
        match environment.power.value() {
            Some(power) => power,
            None => return Finding::unknown(
                ID,
                format!(
                    "the power source could not be read: {}",
                    environment.power.reason().unwrap_or("no reason given")
                ),
                "look at the battery icon in the menu bar or system tray — and plug in anyway, \
                 since it costs nothing",
            ),
        };

    if power.source == PowerSource::Ac {
        return match power.charge_percent {
            Some(percent) => Finding::pass(ID, format!("on mains power, battery at {percent}%")),
            // A desktop, or a laptop docked through something that hides the
            // battery. Either way there is nothing left to worry about.
            None => Finding::pass(ID, "on mains power, no battery in this machine"),
        };
    }

    let Some(percent) = power.charge_percent else {
        return Finding::warn(
            ID,
            "running on battery, and the charge level could not be read",
            "plug in — an unknown charge is not a reason to find out how much there was",
        );
    };

    if percent < CRITICAL_PERCENT {
        return Finding::fail(
            ID,
            format!("running on battery at {percent}%"),
            "plug in now, and give it a few minutes before you start — driving a projector from \
             a nearly flat battery can discharge faster than the charger refills it",
        );
    }

    if percent < LOW_PERCENT {
        return Finding::warn(
            ID,
            format!("running on battery at {percent}%"),
            "plug in — a projector, an external display and a screen recorder together will not \
             leave this enough for a full slot",
        );
    }

    Finding::warn(
        ID,
        format!("running on battery at {percent}%"),
        "plug in — the menu-bar estimate was measured while you read email, not while you drove \
         a projector, and it is the reason talks die at what the speaker remembers as plenty",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Power, Reading};
    use crate::finding::Status;

    fn status_on_battery(percent: u8) -> Status {
        let environment = Environment::new().with_power(Reading::known(Power::on_battery(percent)));

        check(&environment).status
    }

    #[test]
    fn a_machine_on_mains_power_passes() {
        let environment = Environment::new().with_power(Reading::known(Power::on_mains(64)));
        let finding = check(&environment);

        assert_eq!(finding.status, Status::Pass);
        assert!(finding.detail.contains("mains"));
    }

    #[test]
    fn a_desktop_with_no_battery_passes_rather_than_reporting_an_unknown() {
        // There is no battery to read, which is not the same as a battery we
        // failed to read. Reporting Unknown here would put a permanent amber
        // line on every lectern machine.
        let environment = Environment::new().with_power(Reading::known(Power::mains_only()));

        assert_eq!(check(&environment).status, Status::Pass);
    }

    #[test]
    fn a_full_battery_on_no_charger_is_still_a_warning() {
        // The opinion this module exists for. 100% and unplugged is the state
        // every speaker who ran out of power was in an hour earlier.
        assert_eq!(status_on_battery(100), Status::Warn);
    }

    #[test]
    fn the_warning_at_a_comfortable_charge_says_why_the_estimate_lies() {
        // "Plug in" without a reason is advice a confident speaker overrules.
        let environment = Environment::new().with_power(Reading::known(Power::on_battery(85)));
        let remedy = check(&environment).remedy.unwrap();

        assert!(remedy.contains("projector"), "got: {remedy}");
    }

    #[test]
    fn the_boundary_between_low_and_comfortable_is_at_fifty_percent() {
        // Both sides warn; the boundary decides which advice is given, so it
        // is pinned rather than left to drift.
        assert_eq!(status_on_battery(LOW_PERCENT), Status::Warn);
        assert_eq!(status_on_battery(LOW_PERCENT - 1), Status::Warn);
    }

    #[test]
    fn a_battery_below_twenty_percent_fails_rather_than_warns() {
        // The point where plugging in during the talk stops being enough.
        assert_eq!(status_on_battery(CRITICAL_PERCENT - 1), Status::Fail);
        assert_eq!(status_on_battery(CRITICAL_PERCENT), Status::Warn);
    }

    #[test]
    fn a_failing_battery_is_told_to_charge_before_starting_not_just_to_plug_in() {
        // From flat, plugging in at the moment the talk begins loses the race.
        let environment = Environment::new().with_power(Reading::known(Power::on_battery(4)));
        let remedy = check(&environment).remedy.unwrap();

        assert!(remedy.contains("before you start"), "got: {remedy}");
    }

    #[test]
    fn a_battery_whose_level_is_unreadable_still_warns_about_being_unplugged() {
        // The charge is unknown; being on battery is not. The known half is
        // enough to act on, so this is a warning rather than an unknown.
        let unreadable = Power { source: PowerSource::Battery, charge_percent: None };
        let environment = Environment::new().with_power(Reading::known(unreadable));

        assert_eq!(check(&environment).status, Status::Warn);
    }

    #[test]
    fn an_unreadable_power_source_is_unknown_and_never_a_pass() {
        // A platform with no battery interface must not be reported as a
        // machine that is safely plugged in.
        let environment =
            Environment::new().with_power(Reading::unavailable("no battery interface here"));
        let finding = check(&environment);

        assert_eq!(finding.status, Status::Unknown);
        assert!(finding.detail.contains("no battery interface here"));
        assert!(finding.remedy.is_some());
    }
}
