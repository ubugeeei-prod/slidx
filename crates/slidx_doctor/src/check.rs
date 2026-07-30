//! The check registry.
//!
//! Every check has the same shape — read the [`Environment`], return one
//! [`Finding`] — so the suite is a list rather than a framework. Adding a check
//! is adding a module and one entry in [`ALL`].
//!
//! **One check, one finding, always.** A check that returns nothing when it is
//! happy would make the report shorter on a healthy machine, which sounds like
//! a kindness and is not: the speaker cannot tell "fonts are fine" from "fonts
//! were never looked at" by staring at a line that is not there.
//!
//! **One check, one subject.** The time zone and the clock's accuracy are two
//! readings that fail independently — at a venue with no network the zone is
//! knowable and the skew is not — so they are two checks. Folding them into one
//! would force a single verdict to speak for a reading that was taken and one
//! that was not.

pub mod audio;
pub mod clock;
pub mod disk;
pub mod display;
pub mod fonts;
pub mod network;
pub mod notifications;
pub mod power;
pub mod processes;

use crate::environment::Environment;
use crate::finding::Finding;

/// A check: readings in, one finding out. No I/O, no clock, no randomness.
pub type CheckFn = fn(&Environment) -> Finding;

/// One thing the doctor looks at.
#[derive(Debug, Clone, Copy)]
pub struct Check {
    /// Stable id, also the [`Finding::check`] field. Grouped with a slash where
    /// two checks share a subject, matching the linter's diagnostic codes.
    pub id: &'static str,
    /// One line, for the report.
    pub title: &'static str,
    /// What goes wrong on stage when this is not green. Written for a `doctor
    /// --explain` and for the docs, and kept next to the check so the two
    /// cannot drift.
    pub matters: &'static str,
    run: CheckFn,
}

impl Check {
    pub fn run(&self, environment: &Environment) -> Finding {
        (self.run)(environment)
    }
}

/// Every check, in the order their findings are reported within one severity.
///
/// Ordered by how cheaply a speaker standing in the room can act on it. Plugging
/// in a laptop takes five seconds and removes the most complete failure on the
/// list, so power leads; the network is informational and cannot be acted on at
/// all, so it trails.
///
/// Mirroring, notifications and the output level sit immediately behind power
/// for the same reason it leads: each is a switch, each takes about ten seconds,
/// and each removes something the room would otherwise see or fail to hear. The
/// resolution line is further down because it is mostly informational — it says
/// which screens are attached rather than asking for anything.
pub const ALL: &[Check] = &[
    Check {
        id: "power",
        title: "Power",
        matters: "A laptop that reaches 0% mid-talk ends the talk. Nobody plans \
                  for it, because at 60% the battery still feels like plenty.",
        run: power::check,
    },
    Check {
        id: "display/mirroring",
        title: "Display mirroring",
        matters: "A mirrored pair is one screen wearing two cables, so presenter \
                  view has nowhere to open — the notes, the clock and the next \
                  slide are all on the wall behind you or nowhere.",
        run: display::mirroring,
    },
    Check {
        id: "notifications",
        title: "Notifications",
        matters: "A banner arrives on the slide the room is reading, and the \
                  messages that turn up during a talk are the ones you least \
                  want projected.",
        run: notifications::check,
    },
    Check {
        id: "audio",
        title: "Audio output",
        matters: "A demo or a video that plays silently is a failure nobody in \
                  the room mentions for the first thirty seconds, and by then \
                  the moment has gone.",
        run: audio::check,
    },
    Check {
        id: "disk",
        title: "Disk space",
        matters: "A recording that stops at minute twelve, or an export that \
                  fails after the audience has gone home.",
        run: disk::check,
    },
    Check {
        id: "clock/zone",
        title: "Time zone",
        matters: "A laptop that flew in on the wrong zone puts every schedule, \
                  timer and calendar reminder out by hours.",
        run: clock::zone,
    },
    Check {
        id: "fonts",
        title: "Fonts",
        matters: "A substituted font is wider than the one the deck was laid \
                  out against. The title wraps, and the speaker finds out from \
                  the back row.",
        run: fonts::check,
    },
    Check {
        id: "screen-capture",
        title: "Screen capture and conferencing",
        matters: "Anything that can grab the screen, pull focus, or pop a join \
                  prompt in the middle of a demo.",
        run: processes::check,
    },
    Check {
        id: "display/resolution",
        title: "Display resolution",
        matters: "Mostly the line it prints when it passes: which screens are \
                  attached and how big each one is. It only warns where a \
                  screen is small enough to resample the deck rather than \
                  scale it.",
        run: display::resolution,
    },
    Check {
        id: "clock/skew",
        title: "Clock accuracy",
        matters: "A clock minutes out joins the stream late and can fail TLS \
                  on a live demo, with an error that names certificates rather \
                  than the clock.",
        run: clock::skew,
    },
    Check {
        id: "network",
        title: "Network",
        matters: "Informational only. A slidx deck renders offline by design, \
                  so this line exists to tell a speaker whether a live demo \
                  will work — never to hold up a talk.",
        run: network::check,
    },
];

/// Looks up a check by id.
pub fn find(id: &str) -> Option<&'static Check> {
    ALL.iter().find(|check| check.id == id)
}

/// Position in the registry, for sorting a report.
///
/// An id the registry does not know sorts last rather than first: an
/// embedder's own check should appear after the built-in ones, not scattered
/// through them.
pub fn order_of(id: &str) -> usize {
    ALL.iter().position(|check| check.id == id).unwrap_or(usize::MAX)
}

/// Every check id, for `--help` and for documentation.
pub fn ids() -> Vec<&'static str> {
    ALL.iter().map(|check| check.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Environment;

    #[test]
    fn every_check_is_registered_exactly_once() {
        let mut ids = ids();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();

        assert_eq!(ids.len(), total, "a check id is registered twice");
    }

    #[test]
    fn every_check_reports_a_finding_tagged_with_its_own_id() {
        // The report is keyed by id; a check that tags its finding with
        // someone else's would sort into the wrong place and be looked up as
        // the wrong check.
        let environment = Environment::new();

        for check in ALL {
            assert_eq!(check.run(&environment).check, check.id, "{} mislabels", check.id);
        }
    }

    #[test]
    fn every_check_describes_itself() {
        // `title` and `matters` are what a speaker reads next to a red line.
        // A blank one makes the report say nothing at the moment it matters.
        for check in ALL {
            assert!(!check.id.is_empty());
            assert!(!check.title.is_empty(), "{} has no title", check.id);
            assert!(!check.matters.is_empty(), "{} does not say why it exists", check.id);
        }
    }

    #[test]
    fn check_ids_are_lowercase_and_free_of_spaces() {
        // Ids end up in JSON keys, in `--only` filters and in URLs to docs.
        for check in ALL {
            assert_eq!(check.id.to_lowercase(), check.id, "{} is not lowercase", check.id);
            assert!(!check.id.contains(' '), "{} contains a space", check.id);
        }
    }

    #[test]
    fn a_check_can_be_found_by_id() {
        assert_eq!(find("power").map(|check| check.title), Some("Power"));
        assert!(find("no-such-check").is_none());
    }

    #[test]
    fn registry_order_puts_the_cheapest_fix_first_and_the_informational_last() {
        // Power is five seconds of work and removes the worst outcome on the
        // list. The network line cannot be acted on at all.
        assert_eq!(order_of("power"), 0);
        assert_eq!(order_of("network"), ALL.len() - 1);
    }

    #[test]
    fn an_unregistered_id_sorts_after_every_registered_one() {
        assert!(order_of("venue/hdmi") > order_of("network"));
    }

    #[test]
    fn the_three_switches_a_speaker_can_flick_sit_directly_behind_power() {
        // Mirroring, notifications and the output level are each about ten
        // seconds of work that removes something the room would otherwise see
        // or fail to hear, which puts them with power rather than with disk
        // space and fonts.
        for id in ["display/mirroring", "notifications", "audio"] {
            assert!(order_of(id) < order_of("disk"), "{id} is buried below the disk check");
        }
    }

    #[test]
    fn the_resolution_line_sits_below_the_checks_that_ask_for_something() {
        // It mostly reports which screens are attached. A check that rarely
        // asks for anything belongs where an informational line belongs.
        assert!(order_of("display/resolution") > order_of("display/mirroring"));
        assert!(order_of("display/resolution") < order_of("network"));
    }

    #[test]
    fn two_checks_sharing_a_subject_are_spelled_with_the_same_prefix() {
        // The grouping the report and any `--only` filter key off. A subject
        // split across `display/` and `displays/` would be two subjects.
        let grouped: Vec<&str> =
            ids().into_iter().filter(|id| id.starts_with("display")).collect();

        assert_eq!(grouped, ["display/mirroring", "display/resolution"]);
    }
}
