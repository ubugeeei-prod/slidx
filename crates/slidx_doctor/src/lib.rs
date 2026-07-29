//! # slidx doctor
//!
//! The pre-flight a speaker runs in the room, minutes before starting.
//!
//! Everything this crate looks at is something that happens on stage and never
//! in the editor: the laptop nobody plugged in, the disk with no room for the
//! recording, the machine still on last week's time zone, the font the venue
//! does not have, the conferencing app that grabs the screen mid-demo. None of
//! it is visible from the deck source, so the linter cannot catch any of it —
//! which is why this is a separate crate rather than another rule.
//!
//! ## Nothing here talks to the machine
//!
//! A check is a pure function from an [`Environment`] — a struct of injected
//! readings — to one [`Finding`]. [`probe`] is the only module that touches the
//! operating system, and it produces an `Environment` and nothing else.
//!
//! That split is the whole design. A doctor's job is to be right about *other
//! people's* laptops: flat batteries, full disks, locked-down platforms where a
//! reading is simply not available. A suite that called the OS from inside its
//! checks could only ever be tested against the one machine the tests happen to
//! run on, and every interesting case would be untestable.
//!
//! ## Two rules the tests hold to
//!
//! **Every non-pass finding carries a remedy.** A speaker with ninety seconds
//! cannot act on "disk space low". The constructors on [`Finding`] make the
//! remedy mandatory for anything that is not a pass, and the suite asserts it
//! across every check against hundreds of environments.
//!
//! **An unavailable reading is [`Status::Unknown`], never [`Status::Pass`].**
//! Platforms differ, and some readings cannot be taken at all on some of them.
//! Reporting green for a thing nobody could measure is the one failure that
//! would make the whole report worse than useless.
//!
//! ```
//! use slidx_doctor::{Environment, Expectation, Reading, Status};
//! use slidx_doctor::environment::{Disk, Power};
//!
//! let environment = Environment::new()
//!     .with_power(Reading::known(Power::on_battery(12)))
//!     .with_disk(Reading::known(Disk::new("/", 400_000_000_000, 500_000_000_000)))
//!     .expecting(Expectation::default().at_venue_offset(540));
//!
//! let report = slidx_doctor::run(&environment);
//!
//! // Worst first: a flat battery leads, whatever else is on the list.
//! assert_eq!(report.findings()[0].check, "power");
//! assert_eq!(report.status(), Status::Fail);
//!
//! // Nothing red is ever left without a next action.
//! assert!(report.attention().all(|finding| finding.remedy.is_some()));
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod check;
pub mod environment;
pub mod finding;
pub mod report;

/// The only module that talks to the operating system.
///
/// Denied the panicking shortcuts on purpose: a probe that cannot take a
/// reading has somewhere to put that fact — [`Reading::unavailable`] — so there
/// is never a reason for one to unwrap. A doctor that panics is a doctor that
/// tells a speaker nothing at the moment they most need something.
#[deny(clippy::unwrap_used, clippy::expect_used)]
pub mod probe;

pub use check::{Check, CheckFn};
pub use environment::{Environment, Expectation, Reading};
pub use finding::{Finding, Status};
pub use report::Report;

/// Runs every check against one set of readings.
///
/// Pure: the same `Environment` always produces the same `Report`, which is
/// what makes a bug report reproducible from a captured environment rather than
/// from a description of a room.
pub fn run(environment: &Environment) -> Report {
    Report::new(check::ALL.iter().map(|check| check.run(environment)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_has_one_finding_per_check_always() {
        // A fixed-length report can be scanned; a variable one has to be read.
        // Two minutes before a talk, only the first is any use.
        assert_eq!(run(&Environment::new()).len(), check::ALL.len());
    }

    #[test]
    fn a_machine_nobody_could_read_reports_unknowns_rather_than_a_clean_bill() {
        // The default environment knows nothing. Every check must say so.
        let report = run(&Environment::new());

        assert_eq!(report.status(), Status::Unknown);
        assert!(!report.is_healthy());
    }
}
