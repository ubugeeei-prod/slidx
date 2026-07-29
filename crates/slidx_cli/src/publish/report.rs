//! What `slidx publish` prints.
//!
//! The same shape `doctor` and `lint` print, for the same reason: somebody who
//! has learned to read one of them has learned to read this. The status is the
//! first word, the destination is next, what would happen is underneath, and
//! the line marked `->` is the one they act on.
//!
//! The statuses are the whole report in one column. `wrote` is done and needs
//! nothing; `ready` is composed and waiting on an account slidx does not have;
//! `blocked` is a field to add. Under `--plan` nothing is `wrote`, because
//! nothing was written — which is what makes a plan readable before it is
//! meant.

use slidx_publish::{PublishPlan, PublishStep};

use crate::report;
use crate::style::{Ink, Style};

use super::hand_off::HandOff;
use super::write::Written;

/// What happened to one step, once the command has run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    /// Written to disk. Carries the files.
    Wrote(Vec<Written>),
    /// Composed, and waiting on an account.
    Handed(HandOff),
    /// Planned only. Nothing was written and nothing was opened.
    Planned,
    /// Not composed at all.
    Blocked,
}

impl Done {
    fn status(&self) -> (&'static str, Ink) {
        match self {
            Self::Wrote(_) => ("wrote", Ink::Pass),
            Self::Handed(_) => ("ready", Ink::Warn),
            Self::Planned => ("planned", Ink::Faint),
            Self::Blocked => ("blocked", Ink::Fail),
        }
    }
}

/// The report, as a person reads it.
pub fn render(plan: &PublishPlan, done: &[Done], style: &Style) -> String {
    let mut text = format!(
        "{} {}\n\n  {}\n",
        style.paint(Ink::Strong, "slidx publish"),
        style.paint(Ink::Faint, &plan.deck),
        verdict(done, style)
    );

    for (step, done) in plan.steps.iter().zip(done) {
        text.push('\n');
        text.push_str(&entry(step, done, style));
    }

    text
}

/// One destination, and everything under it.
///
/// A blocked step prints one block per reason rather than one block with the
/// reasons run together. Each reason is a separate thing to go and do, and the
/// report is read by somebody deciding what to type next.
fn entry(step: &PublishStep, done: &Done, style: &Style) -> String {
    let (status, ink) = done.status();
    let subject = step.target().as_token();

    if step.reasons().is_empty() {
        return report::block(status, ink, subject, step.summary(), remedy(done).as_deref(), style);
    }

    step.reasons()
        .iter()
        .map(|reason| {
            report::block(status, ink, subject, step.summary(), Some(&reason.message), style)
        })
        .collect()
}

/// The line somebody acts on.
fn remedy(done: &Done) -> Option<String> {
    match done {
        Done::Wrote(files) => {
            let names: Vec<String> =
                files.iter().map(|file| file.path.display().to_string()).collect();
            Some(format!("wrote {}", names.join(", ")))
        }
        Done::Handed(hand) => Some(match hand.page {
            Some(page) => format!("this one needs your account — {page}"),
            // A post has no one destination, so the action is the paste itself.
            None => "copy the text below and post it wherever you post".to_string(),
        }),
        Done::Planned => None,
        Done::Blocked => None,
    }
}

/// The payloads, printed under the report.
///
/// Only for the destinations slidx will not finish. A page it wrote itself is
/// on disk, and printing its contents to a terminal would bury the two things
/// somebody still has to do.
pub fn payloads(done: &[Done], style: &Style) -> String {
    let mut text = String::new();

    for hand in done.iter().filter_map(|done| match done {
        Done::Handed(hand) => Some(hand),
        _ => None,
    }) {
        text.push('\n');
        text.push_str(&payload(hand, style));
    }

    text
}

fn payload(hand: &HandOff, style: &Style) -> String {
    let width = hand.written().map(|(name, _)| name.len()).max().unwrap_or(0);
    let mut text = format!("  {}\n", style.paint(Ink::Strong, hand.platform));

    for (name, value) in hand.written() {
        text.push_str(&report::line("", Ink::Faint, name, value, width, style));
    }

    text
}

/// The one line somebody reads if they read only one.
fn verdict(done: &[Done], style: &Style) -> String {
    let count = |wanted: fn(&Done) -> bool| done.iter().filter(|entry| wanted(entry)).count();

    let wrote = count(|done| matches!(done, Done::Wrote(_)));
    let handed = count(|done| matches!(done, Done::Handed(_)));
    let planned = count(|done| matches!(done, Done::Planned));
    let blocked = count(|done| matches!(done, Done::Blocked));

    if done.is_empty() {
        return style.paint(Ink::Faint, "Nothing to publish: no destination was asked for.");
    }

    let mut said = Vec::new();
    if wrote > 0 {
        said.push(style.paint(Ink::Pass, format!("{wrote} written")));
    }
    if handed > 0 {
        said.push(style.paint(Ink::Warn, format!("{handed} waiting on you")));
    }
    if planned > 0 {
        said.push(style.paint(Ink::Faint, format!("{planned} ready")));
    }
    if blocked > 0 {
        said.push(style.paint(Ink::Fail, format!("{blocked} blocked")));
    }

    format!("{} across {}.", said.join(", "), destinations(done.len()))
}

/// "1 destination" rather than "1 destinations".
fn destinations(count: usize) -> String {
    format!("{count} destination{}", if count == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::hand_off::hand_off;
    use slidx_publish::{plan_publish, DeckMetadata, PlanOptions, PublishTarget};
    use std::path::PathBuf;

    fn plan(targets: Vec<PublishTarget>) -> PublishPlan {
        plan_publish(&PlanOptions {
            meta: DeckMetadata {
                title: Some("Zero-JavaScript Slides".into()),
                url: Some("https://slidx.dev/talks/zero-js".into()),
                event: Some("SlidxConf 2026".into()),
                repo: Some("https://github.com/ubugeeei-prod/slidx".into()),
                ..DeckMetadata::default()
            },
            targets: Some(targets),
            ..PlanOptions::default()
        })
    }

    fn wrote(path: &str) -> Done {
        Done::Wrote(vec![Written { path: PathBuf::from(path) }])
    }

    #[test]
    fn the_verdict_is_the_first_thing_under_the_title() {
        let plan = plan(vec![PublishTarget::Social]);
        let done = vec![Done::Handed(hand_off(&plan.steps[0]).expect("a hand-off"))];
        let text = render(&plan, &done, &Style::plain());
        let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();

        assert!(lines[1].contains("1 waiting on you"), "{text}");
    }

    #[test]
    fn a_written_page_says_where_it_went() {
        let plan = plan(vec![PublishTarget::Resources]);
        let text = render(&plan, &[wrote("./resources.md")], &Style::plain());

        assert!(text.contains("wrote"), "{text}");
        assert!(text.contains("./resources.md"), "{text}");
    }

    #[test]
    fn a_destination_slidx_will_not_finish_says_so_on_the_line_you_act_on() {
        let plan = plan(vec![PublishTarget::Social]);
        let done = vec![Done::Handed(hand_off(&plan.steps[0]).expect("a hand-off"))];
        let text = render(&plan, &done, &Style::plain());

        assert!(text.contains(report::REMEDY), "{text}");
        assert!(text.contains("post it wherever you post"), "{text}");
    }

    #[test]
    fn a_blocked_step_prints_one_line_to_act_on_per_reason() {
        // Each reason is a separate thing to go and do, and running them
        // together would read as one paragraph nobody finishes.
        let plan = plan_publish(&PlanOptions {
            meta: DeckMetadata::default(),
            targets: Some(vec![PublishTarget::Blog]),
            ..PlanOptions::default()
        });
        let text = render(&plan, &[Done::Blocked], &Style::plain());

        assert_eq!(text.matches(report::REMEDY).count(), plan.steps[0].reasons().len());
        assert!(text.contains("`title:`"), "{text}");
    }

    #[test]
    fn a_plan_writes_nothing_and_says_nothing_was_written() {
        // What makes `--plan` worth having: it is the same report, minus every
        // claim that something happened.
        let plan = plan(vec![PublishTarget::Resources]);
        let text = render(&plan, &[Done::Planned], &Style::plain());

        assert!(!text.contains("wrote"), "{text}");
        assert!(text.contains("planned"), "{text}");
    }

    #[test]
    fn the_payload_is_printed_as_fields_a_person_pastes_rather_than_as_a_blob() {
        let plan = plan(vec![PublishTarget::Social]);
        let done = vec![Done::Handed(hand_off(&plan.steps[0]).expect("a hand-off"))];
        let text = payloads(&done, &Style::plain());

        assert!(text.contains("Post"), "{text}");
        assert!(text.contains("text"), "{text}");
    }

    #[test]
    fn a_page_slidx_wrote_itself_has_no_payload_printed_under_it() {
        // Its contents are on disk. Printing them would bury the two things
        // somebody still has to do.
        assert_eq!(payloads(&[wrote("./resources.md")], &Style::plain()), "");
    }

    #[test]
    fn the_verdict_counts_one_destination_as_a_destination() {
        assert_eq!(destinations(1), "1 destination");
        assert_eq!(destinations(6), "6 destinations");
    }

    #[test]
    fn asking_for_no_destination_is_reported_rather_than_printed_as_an_empty_page() {
        let text = render(&plan(Vec::new()), &[], &Style::plain());

        assert!(text.contains("Nothing to publish"), "{text}");
    }

    #[test]
    fn a_report_lines_up_the_same_coloured_and_plain() {
        let plan = plan(vec![PublishTarget::Social]);
        let done = vec![Done::Handed(hand_off(&plan.steps[0]).expect("a hand-off"))];

        let plain = render(&plan, &done, &Style::plain());
        let colored = render(&plan, &done, &Style::colored());

        assert_eq!(plain.lines().count(), colored.lines().count());
        assert!(!plain.contains('\u{1b}'));
    }

    #[test]
    fn nothing_the_report_prints_runs_past_the_fixed_width() {
        let plan = plan(vec![PublishTarget::Social, PublishTarget::Resources]);
        let done = vec![Done::Handed(hand_off(&plan.steps[0]).expect("a hand-off")), Done::Planned];
        let text = format!(
            "{}{}",
            render(&plan, &done, &Style::plain()),
            payloads(&done, &Style::plain())
        );

        for line in text.lines() {
            // The post's text is one long string the author copies whole, and
            // breaking it would break the paste. Everything else wraps.
            if line.trim_start().starts_with("text") {
                continue;
            }
            assert!(line.chars().count() <= crate::style::WIDTH, "{line}");
        }
    }
}
