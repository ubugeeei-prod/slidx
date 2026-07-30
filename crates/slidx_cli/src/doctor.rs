//! `slidx doctor` — the pre-flight, printed.
//!
//! The split [`slidx_doctor`] is built on holds here: this module asks
//! [`slidx_doctor::probe`] for one [`Environment`] and hands it to the checks,
//! which stay pure functions of it. Nothing in this file reads the machine, and
//! nothing in the check suite does either. That is what makes a report
//! reproducible from a captured environment rather than from somebody's
//! description of a room.
//!
//! ## The output is the feature
//!
//! This is read on a laptop propped on a lectern, under stage lighting, by
//! somebody with ninety seconds who is about to speak in front of people. So:
//!
//! **The verdict is the first line.** Not the last. Somebody who reads one line
//! and stops has to have read the one that matters.
//!
//! **Ink follows urgency.** Anything that needs doing gets a paragraph of its
//! own — status, check, what was seen, what to do — separated by blank lines so
//! the eye lands on it. Everything that passed gets one line in a tight list
//! underneath. The page then *looks* like what it says before a word of it has
//! been read.
//!
//! **Every finding that is not green carries its next action**, on its own
//! line, marked. The remedy is the only part anybody can act on, and it is what
//! the [`Finding`](slidx_doctor::Finding) constructors make mandatory.
//!
//! **Every check appears, always.** Seven checks, seven entries, in the same
//! order — so the report can be scanned rather than read. One that got shorter
//! on a healthy machine would make "fonts are fine" and "fonts were never
//! looked at" look identical.
//!
//! **Wrapped at a fixed width**, never at the window's edge. See
//! [`style::WIDTH`](crate::style::WIDTH).
//!
//! ## Exit code
//!
//! `1` when something **failed**, `0` otherwise. Warnings and unmeasured
//! readings exit `0` on purpose: a doctor that went red because a locked-down
//! laptop would not enumerate its fonts is a doctor people learn to ignore, and
//! an ignored pre-flight is worse than none. The report still says so on screen,
//! where a person will actually act on it.

use std::path::{Path, PathBuf};

use slidx_core::{parse_deck, DeckParseOptions};
use slidx_doctor::probe::{self, Request};
use slidx_doctor::{check, Expectation, Finding, Report, Status};

use crate::args::Matches;
use crate::lint::source;
use crate::project;
use crate::report::{self, INDENT};
use crate::style::{Ink, Style};
use crate::{Outcome, FOUND, OK};

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let report = slidx_doctor::run(&probe::read(&request(matches)));

    let text = if matches.is_set("json") {
        match serde_json::to_string_pretty(&report) {
            Ok(json) => format!("{json}\n"),
            Err(error) => {
                return Outcome::misuse(format!("could not serialise the report: {error}\n"))
            }
        }
    } else {
        render(&report, style, matches.is_set("explain"))
    };

    Outcome::out(text).with_code(exit_code(&report))
}

/// Builds the probe request from the command line.
fn request(matches: &Matches) -> Request {
    let base = if matches.is_set("offline") { Request::offline() } else { Request::default() };

    let base = match matches.value("dir") {
        Some(path) => base.in_workspace(PathBuf::from(path)),
        None => base,
    };

    let expected = expectation(&base.workspace);
    base.expecting(expected)
}

/// What the deck in this directory asks of the machine.
///
/// The half of the pre-flight that cannot be read off the laptop. A camera is
/// only worth a line when a slide places one, so the deck has to be consulted
/// before the report can decide whether a machine with no webcam is a problem
/// or the ordinary case.
///
/// Best effort and silent. Running `slidx doctor` from a directory with no deck
/// in it is a normal thing to do — the power, disk and clock checks are about
/// the room, not the talk — so a deck that is absent or unreadable expects
/// nothing rather than producing an error.
fn expectation(workspace: &Path) -> Expectation {
    Expectation::default().wanting_camera_on(camera_slides(workspace))
}

fn camera_slides(workspace: &Path) -> usize {
    let Some(path) = project::primary_deck(workspace) else { return 0 };
    let Ok(deck_source) = source::read(&path, "---") else { return 0 };

    parse_deck(&deck_source.source, &DeckParseOptions::default())
        .slides
        .iter()
        .filter(|slide| slide.camera.is_some())
        .count()
}

/// `1` only for an outright failure. See the module docs.
fn exit_code(report: &Report) -> u8 {
    if report.status() == Status::Fail {
        FOUND
    } else {
        OK
    }
}

/// The whole report, as a person reads it.
///
/// Pure, so every interesting machine — flat battery, full disk, nothing
/// readable at all — is one line of test setup rather than a laptop somebody
/// has to physically arrange.
pub fn render(report: &Report, style: &Style, explain: bool) -> String {
    let mut text =
        format!("{}\n\n  {}\n", style.paint(Ink::Strong, "slidx doctor"), verdict(report, style));

    // The report is already sorted worst first, so this partition preserves the
    // reading order rather than imposing a second one.
    let (attention, passed): (Vec<&Finding>, Vec<&Finding>) =
        report.iter().partition(|finding| finding.status.needs_attention());

    let width = check::ids().iter().map(|id| id.chars().count()).max().unwrap_or(0);

    for finding in &attention {
        text.push('\n');
        text.push_str(&report::block(
            &token(finding),
            ink_for(finding.status),
            finding.check,
            &finding.detail,
            finding.remedy.as_deref(),
            style,
        ));

        if explain {
            text.push_str(&report::flowed(matters(finding.check), INDENT, Ink::Faint, style));
        }
    }

    if !passed.is_empty() {
        text.push('\n');
    }

    for finding in &passed {
        text.push_str(&report::line(
            &token(finding),
            Ink::Pass,
            finding.check,
            &finding.detail,
            width,
            style,
        ));

        if explain {
            text.push_str(&report::flowed(
                matters(finding.check),
                INDENT + width + 2,
                Ink::Faint,
                style,
            ));
        }
    }

    text
}

/// The status, as the report prints it.
///
/// Uppercased from [`Status::as_token`] rather than spelled again here, so the
/// word on screen and the word in `--json` cannot disagree.
fn token(finding: &Finding) -> String {
    finding.status.as_token().to_uppercase()
}

/// What this check exists to catch, from the registry.
///
/// Empty for a check the registry has never heard of, which is how an
/// embedder's own finding lands here. It prints without explanation rather than
/// being dropped.
fn matters(check: &str) -> &'static str {
    check::find(check).map(|entry| entry.matters).unwrap_or_default()
}

/// The one line somebody reads if they read only one.
fn verdict(report: &Report, style: &Style) -> String {
    let counts = [
        (report.tally(Status::Fail), "to fix before you start", Ink::Fail),
        (report.tally(Status::Warn), "worth thirty seconds", Ink::Warn),
        (report.tally(Status::Unknown), "nobody could measure", Ink::Unknown),
    ];

    let said: Vec<String> = counts
        .iter()
        .filter(|(count, _, _)| *count > 0)
        .map(|(count, phrase, ink)| style.paint(*ink, format!("{count} {phrase}")))
        .collect();

    if said.is_empty() {
        return style
            .paint(Ink::Pass, format!("Nothing to do. All {} checks are clear.", report.len()));
    }

    format!("{}.", said.join(", "))
}

fn ink_for(status: Status) -> Ink {
    match status {
        Status::Fail => Ink::Fail,
        Status::Warn => Ink::Warn,
        Status::Unknown => Ink::Unknown,
        Status::Pass => Ink::Pass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_doctor::Finding;

    fn matches_for(line: &str) -> Matches {
        let argv: Vec<String> =
            format!("doctor {line}").split_whitespace().map(String::from).collect();

        match crate::args::parse(&argv) {
            crate::args::Invocation::Run(_, matches) => matches,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    fn troubled() -> Report {
        Report::new([
            Finding::fail("power", "12% and not charging", "Find a socket now."),
            Finding::warn("disk", "6.2 GB free", "Clear space before you record."),
            Finding::unknown("clock/skew", "no reference clock", "Check against your phone."),
            Finding::pass("network", "online"),
        ])
    }

    #[test]
    fn the_verdict_is_the_first_thing_under_the_title() {
        // Somebody who reads one line has to have read the one that counts.
        let text = render(&troubled(), &Style::plain(), false);
        let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();

        assert!(lines[1].contains("1 to fix before you start"), "{text}");
    }

    #[test]
    fn the_verdict_counts_each_severity_separately() {
        let text = render(&troubled(), &Style::plain(), false);

        assert!(text.contains("1 to fix before you start"), "{text}");
        assert!(text.contains("1 worth thirty seconds"), "{text}");
        assert!(text.contains("1 nobody could measure"), "{text}");
    }

    #[test]
    fn a_clean_machine_is_told_so_in_a_sentence_rather_than_by_absence() {
        let report =
            Report::new([Finding::pass("power", "on mains"), Finding::pass("disk", "plenty")]);

        assert!(render(&report, &Style::plain(), false).contains("Nothing to do"),);
    }

    #[test]
    fn an_unmeasured_reading_never_reads_as_a_clean_bill() {
        // The failure this whole crate exists to avoid, checked at the last
        // place it could still be undone — the printing.
        let report = Report::new([Finding::unknown("power", "no battery api", "look at the icon")]);
        let text = render(&report, &Style::plain(), false);

        assert!(!text.contains("Nothing to do"), "{text}");
        assert!(text.contains("UNKNOWN"), "{text}");
    }

    #[test]
    fn every_finding_that_is_not_a_pass_prints_its_remedy_on_its_own_line() {
        // Ninety seconds is not enough to work out what "disk space low" wants.
        let text = render(&troubled(), &Style::plain(), false);

        for remedy in
            ["Find a socket now.", "Clear space before you record.", "Check against your phone."]
        {
            let marked = format!("{} {remedy}", report::REMEDY);
            assert!(text.contains(&marked), "{remedy} is missing from:\n{text}");
        }
    }

    #[test]
    fn a_pass_takes_one_line_and_says_nothing_further() {
        let report = Report::new([Finding::pass("network", "online")]);
        let text = render(&report, &Style::plain(), false);
        let body: Vec<&str> = text.lines().filter(|line| line.contains("network")).collect();

        assert_eq!(body.len(), 1, "{body:?}");
    }

    #[test]
    fn findings_are_printed_worst_first() {
        // The report sorts; this asserts the printing does not undo it.
        let text = render(&troubled(), &Style::plain(), false);
        let order: Vec<&str> = text
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .filter(|token| matches!(*token, "FAIL" | "WARN" | "UNKNOWN" | "PASS"))
            .collect();

        assert_eq!(order, ["FAIL", "WARN", "UNKNOWN", "PASS"]);
    }

    #[test]
    fn explain_adds_what_a_check_exists_to_catch_and_is_off_by_default() {
        let report = Report::new([Finding::pass("power", "on mains")]);

        assert!(!render(&report, &Style::plain(), false).contains("mid-talk"));
        assert!(render(&report, &Style::plain(), true).contains("mid-talk"));
    }

    #[test]
    fn what_needs_doing_is_given_room_and_what_passed_is_given_a_list() {
        // The page has to look like what it says before a word of it is read.
        // A warning buried in a column of identical lines does not.
        let text = render(&troubled(), &Style::plain(), false);
        let blocks: Vec<&str> = text.split("\n\n").collect();

        // Title, verdict, one paragraph per attention finding, then the passes.
        assert_eq!(blocks.len(), 6, "{text}");
        assert!(blocks[5].contains("PASS"), "{text}");
        assert!(!blocks[5].contains("FAIL"), "{text}");
    }

    #[test]
    fn no_line_of_a_report_runs_past_the_fixed_width() {
        // Findings carry whole sentences. Left unwrapped they run to two
        // hundred columns and the report stops being scannable, which is the
        // only thing it is for.
        let report = Report::new([Finding::warn(
            "disk",
            "4.6 GiB (about 65 minutes of 1080p recording) free on /System/Volumes/Data",
            "enough for a short recording and nothing else. Clear space now if you plan to \
             record, or record to an external drive",
        )]);

        for line in render(&report, &Style::plain(), true).lines() {
            assert!(
                line.chars().count() <= crate::style::WIDTH,
                "{} columns: {line}",
                line.chars().count()
            );
        }
    }

    #[test]
    fn a_status_column_lines_up_whether_or_not_colour_is_on() {
        // Escape codes are zero-width on screen. Counted as characters they
        // shear the whole report one line at a time.
        let plain = render(&troubled(), &Style::plain(), false);
        let colored = render(&troubled(), &Style::colored(), false);

        assert_eq!(plain.lines().count(), colored.lines().count());
        assert!(!plain.contains('\u{1b}'));
        assert!(colored.contains('\u{1b}'));
    }

    #[test]
    fn the_report_adds_no_characters_a_venue_console_could_mangle() {
        // Everything this module puts on the page — markers, status words,
        // indentation — has to render on the worst terminal in the building.
        // The findings' own wording is the doctor crate's to choose.
        assert!(render(&troubled(), &Style::plain(), false).is_ascii());
    }

    #[test]
    fn only_an_outright_failure_exits_non_zero() {
        // A warning or an unreadable font list must not fail somebody's script;
        // that is how a pre-flight becomes a thing people skip.
        assert_eq!(exit_code(&troubled()), FOUND);
        assert_eq!(exit_code(&Report::new([Finding::warn("disk", "tight", "clear space")])), OK);
        assert_eq!(exit_code(&Report::new([Finding::unknown("fonts", "no api", "look")])), OK);
        assert_eq!(exit_code(&Report::new([Finding::pass("power", "on mains")])), OK);
    }

    #[test]
    fn offline_builds_a_request_that_opens_no_sockets() {
        // `--offline` has to reach the probe, or a diagnostic tool dials out on
        // a network somebody asked it to stay off.
        let request = request(&matches_for("--offline"));

        assert!(request.network_target.is_none());
        assert!(request.time_server.is_none());
    }

    #[test]
    fn dir_points_the_disk_check_at_the_volume_the_deck_is_on() {
        // The deck is often on an external drive. Measuring the boot volume
        // would answer a question nobody asked.
        assert_eq!(request(&matches_for("--dir /tmp/talk")).workspace, PathBuf::from("/tmp/talk"));
    }

    #[test]
    fn a_deck_that_places_a_camera_is_what_makes_the_camera_check_say_anything() {
        // The half of the pre-flight that is not on the laptop. Without this
        // the check has nothing to compare a webcam against, and every speaker
        // gets a green line about a feature none of their slides use.
        let deck = std::env::temp_dir().join("slidx-doctor-camera");
        let slides = deck.join("slides");
        std::fs::create_dir_all(&slides).expect("a temporary deck");
        std::fs::write(slides.join("0001.md"), "---\nlayout: aside\ncamera: side\n---\n\n# One\n")
            .expect("a slide");

        assert_eq!(camera_slides(&deck), 1);

        std::fs::remove_dir_all(&deck).ok();
    }

    #[test]
    fn a_directory_with_no_deck_in_it_expects_nothing_rather_than_failing() {
        // Running the pre-flight from anywhere is normal: power, disk and the
        // clock are about the room rather than about the talk.
        assert_eq!(camera_slides(Path::new("/nonexistent-slidx-project")), 0);
    }

    #[test]
    fn reading_this_machine_produces_a_report_of_every_check() {
        // The one test that touches the operating system. It cannot assert what
        // this machine's battery says — that is why the checks take injected
        // readings — only that a real run answers for every check and prints.
        let outcome = run(&matches_for("--offline"), &Style::plain());

        for id in check::ids() {
            assert!(outcome.stdout.contains(id), "{id} is missing from:\n{}", outcome.stdout);
        }
    }

    #[test]
    fn json_prints_the_findings_as_a_list_rather_than_the_rendered_report() {
        let outcome = run(&matches_for("--offline --json"), &Style::plain());

        assert!(outcome.stdout.starts_with("[\n"), "{}", outcome.stdout);
        assert!(outcome.stdout.contains("\"check\""), "{}", outcome.stdout);
    }
}
