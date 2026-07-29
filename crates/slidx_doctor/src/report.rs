//! The findings, in the order they should be read.
//!
//! A report is one line per check — always the same lines, always all of them.
//! A list whose length changes with the machine's mood has to be *read*; a
//! fixed list can be *scanned*, and scanning is all anyone does two minutes
//! before they walk on.
//!
//! Order is severity first, then the registry order from [`crate::check`].
//! Sorting by severity alone would shuffle the lines between runs, and a report
//! whose rows move is one you have to re-read from the top every time.

use serde::Serialize;

use crate::check;
use crate::finding::{Finding, Status};

/// Everything the doctor found, worst first.
///
/// Serialises, for a `--json` flag; it does not deserialise. A report is an
/// observation of one machine at one moment, and reading one back would mean
/// accepting a check id the registry has never heard of — which the sort order
/// and every lookup are built on top of.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(transparent)]
pub struct Report {
    findings: Vec<Finding>,
}

impl Report {
    /// Sorts on construction, so there is no way to hold an unsorted report.
    pub fn new(findings: impl IntoIterator<Item = Finding>) -> Self {
        let mut findings: Vec<Finding> = findings.into_iter().collect();

        // A stable sort, so findings from a check that is not in the registry —
        // an embedder's own — keep the order they were handed over in rather
        // than being shuffled among themselves.
        findings.sort_by_key(|finding| (finding.status.urgency(), check::order_of(finding.check)));

        Self { findings }
    }

    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Finding> {
        self.findings.iter()
    }

    pub fn len(&self) -> usize {
        self.findings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }

    /// The finding for one check, by id.
    pub fn get(&self, check: &str) -> Option<&Finding> {
        self.findings.iter().find(|finding| finding.check == check)
    }

    /// The worst thing in the report — the headline.
    ///
    /// An empty report is a pass: nothing was checked, so nothing is wrong. The
    /// suite never produces one, but an embedder filtering to a subset can.
    pub fn status(&self) -> Status {
        self.findings.iter().fold(Status::Pass, |worst, finding| worst.worst(finding.status))
    }

    /// True when every check passed and there is nothing left to do.
    pub fn is_healthy(&self) -> bool {
        self.findings.iter().all(|finding| finding.status.is_pass())
    }

    /// The findings that need doing something about, in reading order.
    pub fn attention(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|finding| finding.status.needs_attention())
    }

    /// How many findings landed on each status, for a one-line summary.
    pub fn tally(&self, status: Status) -> usize {
        self.findings.iter().filter(|finding| finding.status == status).count()
    }
}

impl<'a> IntoIterator for &'a Report {
    type Item = &'a Finding;
    type IntoIter = std::slice::Iter<'a, Finding>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for Report {
    type Item = Finding;
    type IntoIter = std::vec::IntoIter<Finding>;

    fn into_iter(self) -> Self::IntoIter {
        self.findings.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(report: &Report) -> Vec<&str> {
        report.iter().map(|finding| finding.check).collect()
    }

    #[test]
    fn failures_are_read_before_warnings_before_unknowns_before_passes() {
        // The one ordering rule that matters: what has to be fixed is at the
        // top, where a speaker with thirty seconds will actually see it.
        let report = Report::new([
            Finding::pass("network", "reachable"),
            Finding::unknown("clock/skew", "not measured", "check your phone"),
            Finding::fail("power", "flat", "plug in"),
            Finding::warn("disk", "tight", "clear space"),
        ]);

        assert_eq!(ids(&report), ["power", "disk", "clock/skew", "network"]);
    }

    #[test]
    fn findings_of_equal_severity_keep_the_registry_order() {
        // Handed over backwards; comes back in the order the checks are
        // registered, so the same machine always prints the same report.
        let report = Report::new([
            Finding::warn("network", "offline", "you do not need it"),
            Finding::warn("disk", "tight", "clear space"),
            Finding::warn("power", "on battery", "plug in"),
        ]);

        assert_eq!(ids(&report), ["power", "disk", "network"]);
    }

    #[test]
    fn severity_outranks_registry_order() {
        // `power` is registered first, but a failing later check still leads.
        let report = Report::new([
            Finding::pass("power", "on mains"),
            Finding::fail("network", "offline", "irrelevant"),
        ]);

        assert_eq!(ids(&report), ["network", "power"]);
    }

    #[test]
    fn a_finding_from_an_unregistered_check_sorts_last_within_its_severity() {
        // Embedders can add checks. They land after the built-ins rather than
        // in an arbitrary spot among them.
        let report = Report::new([
            Finding::warn("venue/hdmi", "unknown adapter", "test it"),
            Finding::warn("power", "on battery", "plug in"),
        ]);

        assert_eq!(ids(&report), ["power", "venue/hdmi"]);
    }

    #[test]
    fn two_unregistered_checks_keep_the_order_they_were_given_in() {
        // Requires a stable sort. Without one, an embedder's checks would
        // reshuffle between runs for no visible reason.
        let report =
            Report::new([Finding::warn("venue/b", "b", "b"), Finding::warn("venue/a", "a", "a")]);

        assert_eq!(ids(&report), ["venue/b", "venue/a"]);
    }

    #[test]
    fn the_headline_status_is_the_worst_finding() {
        let report = Report::new([
            Finding::pass("power", "on mains"),
            Finding::fail("disk", "full", "clear space"),
            Finding::warn("network", "offline", "you do not need it"),
        ]);

        assert_eq!(report.status(), Status::Fail);
        assert!(!report.is_healthy());
    }

    #[test]
    fn an_unknown_alone_is_not_a_healthy_report() {
        // The failure mode this crate exists to avoid: a machine nobody could
        // read reported as a machine that is fine.
        let report = Report::new([Finding::unknown("power", "no battery api", "look at the icon")]);

        assert_eq!(report.status(), Status::Unknown);
        assert!(!report.is_healthy());
    }

    #[test]
    fn an_all_pass_report_is_healthy() {
        let report =
            Report::new([Finding::pass("power", "on mains"), Finding::pass("disk", "plenty")]);

        assert_eq!(report.status(), Status::Pass);
        assert!(report.is_healthy());
    }

    #[test]
    fn an_empty_report_is_a_pass_because_nothing_was_claimed() {
        assert_eq!(Report::default().status(), Status::Pass);
        assert!(Report::default().is_empty());
    }

    #[test]
    fn attention_lists_only_what_the_speaker_has_to_act_on() {
        let report = Report::new([
            Finding::pass("power", "on mains"),
            Finding::warn("disk", "tight", "clear space"),
            Finding::unknown("clock/skew", "not measured", "check your phone"),
        ]);

        let attention: Vec<&str> = report.attention().map(|finding| finding.check).collect();
        assert_eq!(attention, ["disk", "clock/skew"]);
    }

    #[test]
    fn a_finding_can_be_looked_up_by_check_id() {
        let report = Report::new([Finding::pass("power", "on mains")]);

        assert_eq!(report.get("power").map(|finding| finding.status), Some(Status::Pass));
        assert!(report.get("nothing").is_none());
    }

    #[test]
    fn the_tally_counts_findings_by_status() {
        let report = Report::new([
            Finding::pass("power", "on mains"),
            Finding::pass("disk", "plenty"),
            Finding::warn("network", "offline", "you do not need it"),
        ]);

        assert_eq!(report.tally(Status::Pass), 2);
        assert_eq!(report.tally(Status::Warn), 1);
        assert_eq!(report.tally(Status::Fail), 0);
    }

    #[test]
    fn a_report_serialises_as_a_bare_list_of_findings() {
        // The wire shape a `--json` flag would print. A wrapper object would
        // make every consumer reach through one pointless key.
        let json = serde_json::to_string(&Report::new([Finding::pass("power", "on mains")]));

        assert!(json.unwrap().starts_with("[{"));
    }
}
