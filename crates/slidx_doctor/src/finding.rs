//! What a check concluded, and what to do about it.
//!
//! A doctor report is read standing up, in a room, with a minute to spare. That
//! single constraint decides the shape of everything here:
//!
//! - a finding that is not a pass **must** carry a remedy, so the constructors
//!   take one rather than leaving it optional — the invariant is structural,
//!   not a convention someone has to remember;
//! - [`Status::Unknown`] exists because "I could not read this" and "this is
//!   fine" are different sentences, and reporting the second when you mean the
//!   first is how a speaker walks on stage confident about something nobody
//!   ever checked.

use serde::{Deserialize, Serialize};

/// The verdict of one check.
///
/// Deliberately not `Ord`. Sorting a report by severity is a presentation
/// decision that belongs in [`urgency`](Status::urgency); deriving `Ord` here
/// would tie it to the order the variants happen to be written in, so adding a
/// variant or tidying the enum would silently reorder every report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Nothing to do.
    Pass,
    /// Will not stop the talk, but it is worth the thirty seconds.
    Warn,
    /// Fix this before you start.
    Fail,
    /// The reading was not available. Not a pass — go and look yourself.
    Unknown,
}

impl Status {
    /// Reading order: worst first, `Pass` last.
    ///
    /// `Unknown` sorts above `Pass` because it still asks something of the
    /// speaker, and below `Warn` because a measured problem outranks an
    /// unmeasured one when there is only time for the top of the list.
    pub fn urgency(self) -> u8 {
        match self {
            Self::Fail => 0,
            Self::Warn => 1,
            Self::Unknown => 2,
            Self::Pass => 3,
        }
    }

    /// Stable lowercase name, for terminal output and JSON.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "fail",
            Self::Unknown => "unknown",
        }
    }

    pub fn is_pass(self) -> bool {
        self == Self::Pass
    }

    /// True when the speaker has something to do about it.
    pub fn needs_attention(self) -> bool {
        !self.is_pass()
    }

    /// The worse of two verdicts, used to fold several signals into one line.
    pub fn worst(self, other: Self) -> Self {
        if other.urgency() < self.urgency() {
            other
        } else {
            self
        }
    }
}

/// One check's conclusion about this machine, right now.
///
/// Serialises but does not deserialise: `check` is a `&'static str` because it
/// always comes from the registry, and a finding read back from JSON could name
/// a check that does not exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// The [`Check::id`](crate::Check::id) that produced this.
    pub check: &'static str,
    pub status: Status,
    /// What was observed, in the speaker's terms rather than the machine's.
    pub detail: String,
    /// What to do about it. Always present unless the status is `Pass`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
}

impl Finding {
    /// Nothing to report. The only constructor that does not take a remedy,
    /// because it is the only case where there is nothing to act on.
    pub fn pass(check: &'static str, detail: impl Into<String>) -> Self {
        Self { check, status: Status::Pass, detail: detail.into(), remedy: None }
    }

    pub fn warn(check: &'static str, detail: impl Into<String>, remedy: impl Into<String>) -> Self {
        Self::actionable(check, Status::Warn, detail, remedy)
    }

    pub fn fail(check: &'static str, detail: impl Into<String>, remedy: impl Into<String>) -> Self {
        Self::actionable(check, Status::Fail, detail, remedy)
    }

    /// The reading could not be taken.
    ///
    /// Takes a remedy like the others: "I could not measure this" is only
    /// useful next to "here is how you check it by hand in five seconds".
    pub fn unknown(
        check: &'static str,
        detail: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self::actionable(check, Status::Unknown, detail, remedy)
    }

    fn actionable(
        check: &'static str,
        status: Status,
        detail: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self { check, status, detail: detail.into(), remedy: Some(remedy.into()) }
    }

    /// True when this finding is useless to the person reading it: something is
    /// wrong and there is no stated next action. Asserted across every check in
    /// the crate's tests, because a report that fails this is noise.
    pub fn is_noise(&self) -> bool {
        self.status.needs_attention() && self.remedy.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pass_carries_no_remedy_because_there_is_nothing_to_do() {
        let finding = Finding::pass("power", "on mains power");

        assert_eq!(finding.remedy, None);
        assert!(!finding.is_noise());
    }

    #[test]
    fn every_non_pass_constructor_requires_a_remedy() {
        // The invariant is enforced by the type signatures, so this test is
        // really a guard against someone adding a constructor that is not.
        for finding in [
            Finding::warn("a", "d", "r"),
            Finding::fail("a", "d", "r"),
            Finding::unknown("a", "d", "r"),
        ] {
            assert_eq!(finding.remedy.as_deref(), Some("r"));
            assert!(!finding.is_noise());
        }
    }

    #[test]
    fn a_non_pass_finding_without_a_remedy_is_recognised_as_noise() {
        // Only reachable by building the struct literally, which is exactly
        // the case the report-wide test has to catch.
        let finding =
            Finding { check: "a", status: Status::Fail, detail: "d".into(), remedy: None };

        assert!(finding.is_noise());
    }

    #[test]
    fn urgency_puts_failures_first_and_passes_last() {
        // The order a speaker reads under time pressure: act, consider, verify,
        // ignore.
        assert!(Status::Fail.urgency() < Status::Warn.urgency());
        assert!(Status::Warn.urgency() < Status::Unknown.urgency());
        assert!(Status::Unknown.urgency() < Status::Pass.urgency());
    }

    #[test]
    fn an_unknown_reading_is_not_treated_as_a_pass() {
        // The whole point of the variant: an unavailable reading must keep
        // asking for attention rather than disappearing into the green.
        assert!(!Status::Unknown.is_pass());
        assert!(Status::Unknown.needs_attention());
    }

    #[test]
    fn folding_two_signals_keeps_the_worse_one() {
        // Checks that read more than one thing report a single line, so they
        // need a defined way to collapse verdicts.
        assert_eq!(Status::Pass.worst(Status::Warn), Status::Warn);
        assert_eq!(Status::Fail.worst(Status::Warn), Status::Fail);
        assert_eq!(Status::Unknown.worst(Status::Pass), Status::Unknown);
        assert_eq!(Status::Pass.worst(Status::Pass), Status::Pass);
    }

    #[test]
    fn statuses_serialise_to_stable_lowercase_tokens() {
        // Terminal output and any `--json` consumer both key off these, so the
        // spelling is part of the contract.
        for status in [Status::Pass, Status::Warn, Status::Fail, Status::Unknown] {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{}\"", status.as_token()));
        }
    }

    #[test]
    fn a_pass_omits_the_remedy_field_from_json_entirely() {
        let json = serde_json::to_string(&Finding::pass("power", "fine")).unwrap();
        assert!(!json.contains("remedy"), "got: {json}");
    }
}
