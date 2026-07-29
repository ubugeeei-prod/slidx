//! `slidx publish` — the chore after the talk, done from the frontmatter.
//!
//! Publishing a talk is five jobs: the PDF onto two slide hosts, a post that
//! links to it, a write-up nobody starts, and the page of links the audience
//! photographed off the screen. All five are already described by the
//! frontmatter written at proposal time, so none of them should need typing
//! twice — and all of them are done by somebody who has just come off stage.
//!
//! The planning is [`slidx_publish`]'s, entirely. This module reads a deck,
//! hands it over, and then does the part a plan cannot do by itself.
//!
//! ## What it performs, and what it refuses to
//!
//! It writes the four destinations that are files on the author's own disk: the
//! blog scaffold, the resources page, the archive record, and the talk index
//! built from every record beside it.
//!
//! It will not upload, post, or authenticate. **There is no token store and no
//! HTTP client anywhere under this command.** A tool that can post as you is a
//! tool that has to be trusted with a credential, and the cost of not holding
//! one is a paste — see [`hand_off`]. For those two destinations slidx prints
//! the composed payload as fields somebody can read off, and names the page
//! they paste it into.
//!
//! `--plan` writes nothing, opens nothing, and prints the same report. That is
//! what makes a plan reviewable before it is meant, and comparable against what
//! was done last time.
//!
//! ## Exit codes
//!
//! The crate's three, read the same way as `lint`'s. `1` means something was
//! blocked — a field to add before this deck can be published — and `2` means
//! the command could not run: a deck it could not read, a flag naming a file
//! that is not there, or a directory it could not write.

pub mod deck;
pub mod hand_off;
pub mod report;
pub mod write;

use std::path::{Path, PathBuf};

use slidx_core::{parse_deck, DeckParseOptions};
use slidx_publish::{plan_publish, PlanOptions, PublishPlan, PublishTarget};

use crate::args::Matches;
use crate::home::Home;
use crate::index::{self, Entry};
use crate::lint::source;
use crate::style::Style;
use crate::{Outcome, FOUND, OK};

use report::Done;

pub fn run(matches: &Matches, style: &Style) -> Outcome {
    let separator =
        matches.value("separator").map(str::to_string).unwrap_or_else(|| "---".to_string());

    let path = matches
        .first_positional()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source::DEFAULT_DIR));

    let deck_source = match source::read(&path, &separator) {
        Ok(deck_source) => deck_source,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    let deck = parse_deck(
        &deck_source.source,
        &DeckParseOptions { separator, ..DeckParseOptions::default() },
    );

    // The index fills itself: running a command on a deck is what puts it in
    // the list. Best-effort in the strongest sense — see `index::remember`.
    if let Some(root) = crate::lint::project_root(&path) {
        index::remember(&Home::discover().index(), Entry::new(root).describing(&deck));
    }

    let source = match deck::source(&deck, matches) {
        Ok(source) => source,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    let targets = match targets(matches) {
        Ok(targets) => targets,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    let plan = plan_publish(&PlanOptions {
        meta: source.meta,
        slides: source.slides,
        artifacts: source.artifacts,
        targets,
        ..PlanOptions::default()
    });

    // `--plan` is a promise about side effects, so it is checked before any are
    // taken rather than threaded through the code that takes them.
    let planning = matches.is_set("plan");
    let out = Path::new(matches.value("out").unwrap_or("."));

    let done = match perform(&plan, out, planning) {
        Ok(done) => done,
        Err(message) => return Outcome::misuse(format!("{message}\n")),
    };

    if matches.is_set("open") && !planning {
        for page in done.iter().filter_map(opened) {
            hand_off::open(page);
        }
    }

    print(&plan, &done, matches, style)
}

/// Which destinations were asked for, in the plan's own order.
///
/// An unknown name is a misuse rather than an empty plan: `--target speakderdeck`
/// that published nothing and exited zero would look exactly like a deck with
/// nothing to publish.
fn targets(matches: &Matches) -> Result<Option<Vec<PublishTarget>>, String> {
    let mut targets = Vec::new();

    for name in matches.values("target") {
        match PublishTarget::parse(name) {
            Some(target) => targets.push(target),
            None => {
                return Err(format!(
                    "`{name}` is not a destination slidx publishes to.\n\n\
                     It has: {}\n\n\
                     Try: slidx publish --help",
                    slidx_publish::PUBLISH_TARGETS
                        .iter()
                        .map(|target| target.as_token())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }

    Ok((!targets.is_empty()).then_some(targets))
}

/// Does the half that needs no account, unless asked only to plan.
fn perform(plan: &PublishPlan, out: &Path, planning: bool) -> Result<Vec<Done>, String> {
    let mut done = Vec::with_capacity(plan.steps.len());

    for step in &plan.steps {
        if !step.is_ready() {
            done.push(Done::Blocked);
            continue;
        }

        if planning {
            done.push(Done::Planned);
            continue;
        }

        done.push(match write::perform(step, out) {
            Some(written) => Done::Wrote(written?),
            None => match hand_off::hand_off(step) {
                Some(hand) => Done::Handed(hand),
                None => Done::Planned,
            },
        });
    }

    Ok(done)
}

/// The page to put on screen for a step, if it has one.
fn opened(done: &Done) -> Option<&'static str> {
    match done {
        Done::Handed(hand) => hand.page,
        _ => None,
    }
}

fn print(plan: &PublishPlan, done: &[Done], matches: &Matches, style: &Style) -> Outcome {
    // The plan as data, for a CI job that diffs one run against the last. The
    // plan is a pure function of the deck, so two runs produce the same bytes.
    if matches.is_set("json") {
        return match serde_json::to_string_pretty(plan) {
            Ok(json) => Outcome::out(format!("{json}\n")).with_code(exit_code(done)),
            Err(error) => Outcome::misuse(format!("could not serialise the plan: {error}\n")),
        };
    }

    let text = format!("{}{}", report::render(plan, done, style), report::payloads(done, style));

    Outcome::out(text).with_code(exit_code(done))
}

/// `1` when a destination is blocked, which is what makes this usable in CI.
///
/// A step waiting on an account is not a failure: the payload is composed and
/// correct, and the only thing left is a person. Exiting non-zero for that
/// would make every successful publish look like a broken one.
fn exit_code(done: &[Done]) -> u8 {
    if done.iter().any(|entry| matches!(entry, Done::Blocked)) {
        FOUND
    } else {
        OK
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MISUSE;
    use std::fs;

    /// A scratch project with a deck in it.
    struct Project(PathBuf);

    impl Project {
        fn new(name: &str, frontmatter: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-cmd-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("slides")).expect("scratch directory");
            fs::write(
                path.join("slides/0001.md"),
                format!(
                    "---\n{frontmatter}---\n\n# Why plain HTML\n\n\
                     See [the docs](https://slidx.dev/docs).\n\n\
                     <!-- notes: a deck is a document -->\n"
                ),
            )
            .expect("write");

            // The build's output, which slidx names and never produces. A
            // deck with no PDF blocks the two slide hosts, which is a different
            // test from this one.
            fs::create_dir_all(path.join("dist")).expect("dist");
            fs::write(path.join("dist/deck.pdf"), "%PDF-1.7\n").expect("write");

            Self(path)
        }

        fn deck(&self) -> String {
            self.0.join("slides").display().to_string()
        }

        fn pdf(&self) -> String {
            self.0.join("dist/deck.pdf").display().to_string()
        }

        fn out(&self) -> String {
            self.0.display().to_string()
        }

        fn read(&self, path: &str) -> String {
            fs::read_to_string(self.0.join(path)).unwrap_or_default()
        }

        fn exists(&self, path: &str) -> bool {
            self.0.join(path).exists()
        }

        /// The deck, edited the way an author edits one weeks later.
        fn add(&self, frontmatter: &str) {
            let path = self.0.join("slides/0001.md");
            let deck = fs::read_to_string(&path).expect("a deck");

            fs::write(&path, deck.replacen("---\n\n", &format!("{frontmatter}---\n\n"), 1))
                .expect("write");
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const TALK: &str = "title: Zero-JavaScript Slides\n\
        description: Why a deck should be plain HTML.\n\
        event: SlidxConf 2026\n\
        date: 2026-07-29\n\
        hashtag: slidxconf\n\
        url: https://slidx.dev/talks/zero-js\n\
        tags: [rust, slides]\n";

    fn run_line(line: &str) -> Outcome {
        let argv: Vec<String> =
            format!("publish {line}").split_whitespace().map(String::from).collect();

        crate::run(&argv, &Style::plain())
    }

    #[test]
    fn the_pages_that_need_no_account_are_written_to_disk() {
        let project = Project::new("writes", TALK);
        let outcome = run_line(&format!(
            "{} --out {} --pdf {}",
            project.deck(),
            project.out(),
            project.pdf()
        ));

        assert_eq!(outcome.code, OK, "{}", outcome.stdout);
        assert!(project.exists("2026-07-29-zero-javascript-slides.md"));
        assert!(project.exists("resources.md"));
        assert!(project.exists("talks/zero-javascript-slides.md"));
        assert!(project.exists("talks/index.md"));
    }

    #[test]
    fn a_plan_writes_nothing_at_all() {
        // The whole reason `--plan` exists: it can be read before it is meant.
        let project = Project::new("plan", TALK);
        let outcome = run_line(&format!("{} --out {} --plan", project.deck(), project.out()));

        assert!(!project.exists("resources.md"), "{}", outcome.stdout);
        assert!(!project.exists("talks"), "{}", outcome.stdout);
        assert!(outcome.stdout.contains("planned"), "{}", outcome.stdout);
    }

    #[test]
    fn a_destination_that_needs_an_account_is_handed_over_rather_than_performed() {
        // slidx has no credential and will not grow one. The payload is printed
        // so the last step is a paste rather than a retype.
        let project = Project::new("handed", TALK);
        let outcome = run_line(&format!(
            "{} --out {} --pdf {}",
            project.deck(),
            project.out(),
            project.pdf()
        ));

        assert!(outcome.stdout.contains("needs your account"), "{}", outcome.stdout);
        assert!(outcome.stdout.contains("#slidxconf"), "{}", outcome.stdout);
    }

    #[test]
    fn a_blocked_destination_exits_one_so_a_ci_job_can_fail_on_it() {
        // A deck with no url has no post to make, and a run that reported that
        // as success would be a green build over a talk nobody can find.
        let project = Project::new("blocked", "title: A talk\n");
        let outcome = run_line(&format!(
            "{} --out {} --pdf {}",
            project.deck(),
            project.out(),
            project.pdf()
        ));

        assert_eq!(outcome.code, FOUND, "{}", outcome.stdout);
        assert!(outcome.stdout.contains("blocked"), "{}", outcome.stdout);
    }

    #[test]
    fn waiting_on_a_person_is_not_a_failure() {
        // The payload is composed and correct; the only thing left is somebody
        // pasting it. Exiting non-zero would make every publish look broken.
        let project = Project::new("waiting", TALK);
        let outcome = run_line(&format!(
            "{} --out {} --pdf {}",
            project.deck(),
            project.out(),
            project.pdf()
        ));

        assert_eq!(outcome.code, OK, "{}", outcome.stdout);
    }

    #[test]
    fn a_deck_that_is_not_there_exits_two_rather_than_reporting_a_clean_publish() {
        let outcome = run_line("/nowhere/at/all");

        assert_eq!(outcome.code, MISUSE);
        assert!(outcome.stdout.is_empty());
    }

    #[test]
    fn a_destination_nobody_has_is_a_misuse_rather_than_an_empty_plan() {
        // `--target speakderdeck` that published nothing and exited zero would
        // look exactly like a deck with nothing to publish.
        let project = Project::new("typo", TALK);
        let outcome = run_line(&format!("{} --target speakderdeck --plan", project.deck()));

        assert_eq!(outcome.code, MISUSE);
        assert!(outcome.stderr.contains("speakerdeck"), "{}", outcome.stderr);
    }

    #[test]
    fn a_subset_is_planned_in_the_plans_order_however_it_was_asked_for() {
        let project = Project::new("subset", TALK);
        let outcome = run_line(&format!(
            "{} --out {} --target resources --target social",
            project.deck(),
            project.out()
        ));

        let social = outcome.stdout.find("social").expect("social");
        let resources = outcome.stdout.find("resources").expect("resources");
        assert!(social < resources, "{}", outcome.stdout);
    }

    #[test]
    fn the_plan_as_json_is_the_same_bytes_for_the_same_deck() {
        // What a CI job diffs against the last run. The plan is a pure function
        // of the deck, so two runs cannot differ.
        let project = Project::new("json", TALK);
        let line = format!("{} --plan --json", project.deck());

        assert_eq!(run_line(&line).stdout, run_line(&line).stdout);
        assert!(run_line(&line).stdout.starts_with('{'));
    }

    #[test]
    fn the_recording_added_months_later_changes_the_record_and_nothing_else() {
        // The one edit the archive target exists to survive, end to end: the
        // conference publishes the video, the author adds one line, and running
        // the command again updates the record and the index they already have.
        let project = Project::new("recording", TALK);
        let publish = format!("{} --out {} --pdf {}", project.deck(), project.out(), project.pdf());

        run_line(&publish);
        let before = project.read("talks/zero-javascript-slides.md");
        assert!(!before.contains("recording:"), "{before}");
        assert!(!project.read("talks/index.md").contains("[video]"));

        project.add("recording: https://youtu.be/abc123\n");
        run_line(&publish);
        let after = project.read("talks/zero-javascript-slides.md");

        assert_eq!(
            after.lines().filter(|line| !before.lines().any(|old| old == *line)).count(),
            1,
            "{after}"
        );
        assert!(after.contains("recording: \"https://youtu.be/abc123\""), "{after}");
        assert!(project.read("talks/index.md").contains("[video]"));
    }

    #[test]
    fn the_report_carries_no_escape_sequences_when_colour_is_off() {
        let project = Project::new("plain", TALK);
        let outcome = run_line(&format!("{} --out {} --plan", project.deck(), project.out()));

        assert!(!outcome.stdout.contains('\u{1b}'), "{}", outcome.stdout);
    }
}
