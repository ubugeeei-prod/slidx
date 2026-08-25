//! What publishing would do, before any of it is done.
//!
//! Planning is separated from performing for the same reason a build has a dry
//! run: publishing is a set of one-way operations, spread across services that
//! do not agree on what an edit means, carried out by someone who has just come
//! off stage. A plan is the thing that can be read, diffed against last time,
//! and argued with while everything is still reversible.
//!
//! A plan is data. Every step is either *ready*, carrying a complete payload for
//! its destination, or *blocked*, carrying reasons that name the field to add.
//! One missing field never stops the rest of the plan being produced — the whole
//! point is to learn everything that is wrong in one pass rather than one
//! failure at a time.
//!
//! The same deck plans the same way every time: no clock, no filesystem, no
//! network, no iteration order that depends on anything but the deck. That is
//! what makes two plans comparable, and it is why this module reaches for
//! nothing outside the crate.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::targets::{
    compose_archive, compose_blog, compose_cloudflare, compose_docswell, compose_resources,
    compose_social, compose_speaker_deck, describe_archive, describe_blog, describe_cloudflare,
    describe_docswell, describe_resources, describe_social, describe_speaker_deck, ArchiveRecord,
    BlogScaffold, CloudflarePages, DocswellUpload, ResourcesPage, SocialOptions, SocialPost,
    SpeakerDeckUpload,
};
use crate::types::{Artifact, BlockedReason, Composed, DeckMetadata, DeckSlide, DeckSource};

/// One destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum PublishTarget {
    Speakerdeck,
    Docswell,
    Social,
    Blog,
    Resources,
    Cloudflare,
    Archive,
}

impl PublishTarget {
    /// The name a plan prints, which is also the name a `--target` flag takes.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Speakerdeck => "speakerdeck",
            Self::Docswell => "docswell",
            Self::Social => "social",
            Self::Blog => "blog",
            Self::Resources => "resources",
            Self::Cloudflare => "cloudflare",
            Self::Archive => "archive",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        PUBLISH_TARGETS.iter().copied().find(|target| target.as_token() == token)
    }
}

/// Every destination, in the order a plan lists them.
///
/// The order is the order the work happens in: the uploads first, because the
/// URL they produce is what the post links to, and the written pages last. A
/// caller asking for a subset gets it in this order regardless of how they
/// asked, so two people planning the same deck get the same plan.
///
/// `archive` is last because it is the only one that will be run again. It
/// records what the others produced, and it is re-run months later when the
/// conference finally publishes the video.
pub const PUBLISH_TARGETS: [PublishTarget; 7] = [
    PublishTarget::Speakerdeck,
    PublishTarget::Docswell,
    PublishTarget::Social,
    PublishTarget::Blog,
    PublishTarget::Resources,
    PublishTarget::Cloudflare,
    PublishTarget::Archive,
];

/// A destination's own payload, which is what makes a ready step worth having.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub enum ReadyPayload {
    SpeakerDeck(SpeakerDeckUpload),
    Docswell(DocswellUpload),
    Social(SocialPost),
    Blog(BlogScaffold),
    Resources(ResourcesPage),
    Cloudflare(CloudflarePages),
    Archive(ArchiveRecord),
}

/// One step of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum PublishStep {
    /// Everything it needs, and the payload to prove it.
    ///
    /// The payload is boxed because a plan is mostly blocked steps on exactly
    /// the decks this crate exists for, and a blocked step should not carry the
    /// footprint of the largest payload it does not have. The box is invisible
    /// on the wire: serde writes through it.
    Ready {
        target: PublishTarget,
        /// One line, for a printed plan.
        summary: String,
        payload: Box<ReadyPayload>,
    },
    /// Nothing will happen, and the fields that would unblock it.
    Blocked { target: PublishTarget, summary: String, reasons: Vec<BlockedReason> },
}

impl PublishStep {
    pub fn target(&self) -> PublishTarget {
        match self {
            Self::Ready { target, .. } | Self::Blocked { target, .. } => *target,
        }
    }

    pub fn summary(&self) -> &str {
        match self {
            Self::Ready { summary, .. } | Self::Blocked { summary, .. } => summary,
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    pub fn payload(&self) -> Option<&ReadyPayload> {
        match self {
            Self::Ready { payload, .. } => Some(payload.as_ref()),
            Self::Blocked { .. } => None,
        }
    }

    pub fn reasons(&self) -> &[BlockedReason] {
        match self {
            Self::Ready { .. } => &[],
            Self::Blocked { reasons, .. } => reasons,
        }
    }

    /// The status token a report prints, and the tag JavaScript narrows on.
    pub fn status(&self) -> &'static str {
        if self.is_ready() {
            "ready"
        } else {
            "blocked"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PublishPlan {
    /// The deck's title, or a stand-in, for the plan's header line.
    pub deck: String,
    #[ts(type = "Array<PublishStep>")]
    pub steps: Vec<PublishStep>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct PlanOptions {
    pub meta: DeckMetadata,
    /// In any order; everything derived per slide is sorted by index.
    pub slides: Vec<DeckSlide>,
    /// What the build produced. Absent is normal, and is reported per target.
    pub artifacts: Vec<Artifact>,
    /// A subset to plan. Absent means all of [`PUBLISH_TARGETS`].
    #[ts(optional)]
    pub targets: Option<Vec<PublishTarget>>,
    pub social: SocialOptions,
}

/// Plans every requested target, in [`PUBLISH_TARGETS`] order.
pub fn plan_publish(options: &PlanOptions) -> PublishPlan {
    let source = DeckSource {
        meta: options.meta.clone(),
        slides: options.slides.clone(),
        artifacts: options.artifacts.clone(),
    };

    let requested = options.targets.clone().unwrap_or_else(|| PUBLISH_TARGETS.to_vec());
    let steps = PUBLISH_TARGETS
        .iter()
        .filter(|target| requested.contains(target))
        .map(|target| plan_step(*target, &source, options))
        .collect();

    let title = source.meta.title.as_deref().unwrap_or_default().trim();

    PublishPlan {
        deck: if title.is_empty() { "Untitled deck".to_string() } else { title.to_string() },
        steps,
    }
}

/// One step.
///
/// Written out per target rather than driven by a table: the payload type is
/// what makes a ready step worth having, and a table that produced them all
/// would have to erase it to have one signature.
fn plan_step(target: PublishTarget, source: &DeckSource, options: &PlanOptions) -> PublishStep {
    match target {
        PublishTarget::Speakerdeck => step(
            target,
            compose_speaker_deck(source),
            describe_speaker_deck,
            ReadyPayload::SpeakerDeck,
        ),
        PublishTarget::Docswell => {
            step(target, compose_docswell(source), describe_docswell, ReadyPayload::Docswell)
        }
        PublishTarget::Social => step(
            target,
            compose_social(source, &options.social),
            describe_social,
            ReadyPayload::Social,
        ),
        PublishTarget::Blog => {
            step(target, compose_blog(source), describe_blog, ReadyPayload::Blog)
        }
        PublishTarget::Resources => {
            step(target, compose_resources(source), describe_resources, ReadyPayload::Resources)
        }
        PublishTarget::Cloudflare => {
            step(target, compose_cloudflare(source), describe_cloudflare, ReadyPayload::Cloudflare)
        }
        PublishTarget::Archive => {
            step(target, compose_archive(source), describe_archive, ReadyPayload::Archive)
        }
    }
}

fn step<T>(
    target: PublishTarget,
    composed: Composed<T>,
    describe: impl Fn(&T) -> String,
    wrap: impl Fn(T) -> ReadyPayload,
) -> PublishStep {
    match composed {
        Composed::Ready(value) => {
            PublishStep::Ready { target, summary: describe(&value), payload: Box::new(wrap(value)) }
        }
        // The summary names the fields, because that is what the author acts on.
        Composed::Blocked(reasons) => {
            let mut fields: Vec<&str> = Vec::new();
            for entry in &reasons {
                if !fields.contains(&entry.field.as_str()) {
                    fields.push(&entry.field);
                }
            }

            PublishStep::Blocked {
                target,
                summary: format!("needs {}", fields.join(", ")),
                reasons,
            }
        }
    }
}

pub fn ready_steps(plan: &PublishPlan) -> Vec<&PublishStep> {
    plan.steps.iter().filter(|step| step.is_ready()).collect()
}

pub fn blocked_steps(plan: &PublishPlan) -> Vec<&PublishStep> {
    plan.steps.iter().filter(|step| !step.is_ready()).collect()
}

/// True when nothing is blocked. An empty plan is not ready: it does nothing.
pub fn is_ready(plan: &PublishPlan) -> bool {
    !plan.steps.is_empty() && plan.steps.iter().all(PublishStep::is_ready)
}

/// Widest target name, so the columns do not move between plans.
const TARGET_COLUMN: usize = 11;

/// Widest status token, for the same reason.
const STATUS_COLUMN: usize = 7;

/// The plan as text, for printing and for diffing against the last one.
///
/// Fixed column widths, taken from the full target list rather than from what
/// this plan happens to contain, so a plan with two steps lines up against a
/// plan with five and a diff shows what changed rather than what moved.
pub fn format_plan(plan: &PublishPlan) -> String {
    let mut lines = vec![format!("publish plan: {}", plan.deck), String::new()];

    for step in &plan.steps {
        lines.push(format!(
            "  {:STATUS_COLUMN$} {:TARGET_COLUMN$}  {}",
            step.status(),
            step.target().as_token(),
            step.summary()
        ));

        for entry in step.reasons() {
            lines.push(format!("{}{}", " ".repeat(TARGET_COLUMN + 12), entry.message));
        }
    }

    let ready = ready_steps(plan).len();
    lines.push(String::new());
    lines.push(format!("{ready} ready, {} blocked", plan.steps.len() - ready));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ArtifactKind;

    fn meta() -> DeckMetadata {
        DeckMetadata {
            title: Some("Zero-JavaScript Slides".into()),
            description: Some(
                "Why a deck should be plain HTML, and what it costs to keep it that way.".into(),
            ),
            author: Some("ubugeeei".into()),
            event: Some("SlidxConf 2026".into()),
            date: Some("2026-07-29".into()),
            venue: Some("Kyoto".into()),
            hashtag: Some("slidxconf".into()),
            url: Some("https://slidx.dev/talks/zero-js".into()),
            repo: Some("https://github.com/ubugeeei-prod/slidx".into()),
            tags: Some(vec!["rust".into(), "slides".into()]),
            ..DeckMetadata::default()
        }
    }

    fn pdf() -> Artifact {
        Artifact {
            kind: ArtifactKind::Pdf,
            path: "dist/deck.pdf".into(),
            bytes: Some(4 * 1024 * 1024),
        }
    }

    fn slides() -> Vec<DeckSlide> {
        vec![
            DeckSlide {
                index: 0,
                title: Some("Why plain HTML".into()),
                notes: Some(vec!["A deck is a document.".into()]),
                ..DeckSlide::default()
            },
            DeckSlide {
                index: 1,
                notes: Some(vec!["The docs: https://slidx.dev/docs".into()]),
                ..DeckSlide::default()
            },
        ]
    }

    fn complete() -> PublishPlan {
        plan_publish(&PlanOptions {
            meta: meta(),
            slides: slides(),
            artifacts: vec![pdf()],
            ..PlanOptions::default()
        })
    }

    fn targets_of(plan: &PublishPlan) -> Vec<&'static str> {
        plan.steps.iter().map(|step| step.target().as_token()).collect()
    }

    #[test]
    fn every_destination_is_planned_in_the_order_the_work_happens() {
        // The uploads first: the URL they produce is what the post links to.
        assert_eq!(
            targets_of(&complete()),
            ["speakerdeck", "docswell", "social", "blog", "resources", "cloudflare", "archive"]
        );
    }

    #[test]
    fn a_subset_is_planned_in_the_plans_order_rather_than_the_order_it_was_asked_in() {
        // Two people planning the same deck get the same plan, whichever way
        // they typed the flags.
        let plan = plan_publish(&PlanOptions {
            meta: meta(),
            artifacts: vec![pdf()],
            targets: Some(vec![
                PublishTarget::Resources,
                PublishTarget::Speakerdeck,
                PublishTarget::Resources,
            ]),
            ..PlanOptions::default()
        });

        assert_eq!(targets_of(&plan), ["speakerdeck", "resources"]);
    }

    #[test]
    fn asking_for_nothing_plans_nothing_and_is_not_ready() {
        // An empty plan publishes nothing, which is not the same as being
        // ready.
        let plan = plan_publish(&PlanOptions {
            meta: meta(),
            targets: Some(Vec::new()),
            ..PlanOptions::default()
        });

        assert!(plan.steps.is_empty());
        assert!(!is_ready(&plan));
    }

    #[test]
    fn a_deck_with_no_title_is_named_rather_than_left_blank_in_the_header() {
        assert_eq!(plan_publish(&PlanOptions::default()).deck, "Untitled deck");
        assert_eq!(complete().deck, "Zero-JavaScript Slides");
    }

    #[test]
    fn a_ready_step_carries_the_destinations_own_payload() {
        let plan = complete();
        let step = plan.steps.iter().find(|step| step.target() == PublishTarget::Speakerdeck);

        assert!(matches!(
            step.and_then(PublishStep::payload),
            Some(ReadyPayload::SpeakerDeck(upload)) if upload.pdf == "dist/deck.pdf"
        ));
        assert!(is_ready(&plan));
    }

    #[test]
    fn one_missing_field_does_not_stop_the_targets_that_are_fine() {
        // A deck built without a PDF can still have its post composed, which is
        // the whole reason blocking is per step.
        let plan =
            plan_publish(&PlanOptions { meta: meta(), slides: slides(), ..PlanOptions::default() });

        assert_eq!(
            blocked_steps(&plan).iter().map(|s| s.target().as_token()).collect::<Vec<_>>(),
            ["speakerdeck", "docswell"]
        );
        assert_eq!(
            ready_steps(&plan).iter().map(|s| s.target().as_token()).collect::<Vec<_>>(),
            ["social", "blog", "resources", "cloudflare", "archive"]
        );
        assert!(!is_ready(&plan));
    }

    #[test]
    fn a_blocked_step_names_the_fields_to_add_in_its_summary() {
        let plan =
            plan_publish(&PlanOptions { meta: meta(), slides: slides(), ..PlanOptions::default() });

        assert_eq!(blocked_steps(&plan)[0].summary(), "needs pdf");
    }

    #[test]
    fn a_field_is_named_once_however_many_reasons_mention_it() {
        // Both too many tags and one that is too long, which is two reasons
        // about a single line of frontmatter.
        let tags: Vec<String> =
            (0..21).map(|index| format!("{}-{index}", "a".repeat(25))).collect();
        let plan = plan_publish(&PlanOptions {
            meta: DeckMetadata { tags: Some(tags), ..meta() },
            artifacts: vec![pdf()],
            targets: Some(vec![PublishTarget::Docswell]),
            ..PlanOptions::default()
        });

        assert_eq!(blocked_steps(&plan)[0].reasons().len(), 2);
        assert_eq!(blocked_steps(&plan)[0].summary(), "needs tags");
    }

    #[test]
    fn the_write_ups_a_bare_deck_cannot_produce_are_the_only_ones_blocked() {
        let plan = plan_publish(&PlanOptions {
            meta: meta(),
            artifacts: vec![pdf()],
            ..PlanOptions::default()
        });

        assert_eq!(
            blocked_steps(&plan).iter().map(|s| s.target().as_token()).collect::<Vec<_>>(),
            ["blog"]
        );
    }

    #[test]
    fn the_same_deck_plans_identically_however_the_slides_arrived() {
        let forwards = complete();
        let backwards = plan_publish(&PlanOptions {
            meta: meta(),
            slides: slides().into_iter().rev().collect(),
            artifacts: vec![pdf()],
            ..PlanOptions::default()
        });

        assert_eq!(
            serde_json::to_string(&backwards).unwrap(),
            serde_json::to_string(&forwards).unwrap()
        );
    }

    #[test]
    fn planning_leaves_the_callers_slides_in_the_order_they_gave_them() {
        // A plan is a function of the deck, not a mutation of one.
        let options = PlanOptions {
            meta: meta(),
            slides: slides().into_iter().rev().collect(),
            artifacts: vec![pdf()],
            ..PlanOptions::default()
        };
        plan_publish(&options);

        assert_eq!(options.slides[0].index, 1);
    }

    #[test]
    fn a_plans_key_order_is_the_same_on_every_run_so_two_of_them_can_be_diffed() {
        // A field that moved between runs would be a diff that says nothing.
        assert_eq!(
            serde_json::to_string(&complete()).unwrap(),
            serde_json::to_string(&complete()).unwrap()
        );
    }

    #[test]
    fn a_plan_survives_the_round_trip_a_printed_plan_makes_across_the_boundary() {
        // `formatPlan` is handed a plan JavaScript is holding, so every payload
        // has to come back as the variant it left as. The untagged union is
        // resolved by which fields are required, which is a property worth
        // asserting rather than assuming.
        let plan = complete();
        let json = serde_json::to_string(&plan).unwrap();

        assert_eq!(serde_json::from_str::<PublishPlan>(&json).unwrap(), plan);
    }

    #[test]
    fn a_printed_plan_is_headed_by_the_deck_and_ended_by_a_count() {
        let text = format_plan(&complete());
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(lines[0], "publish plan: Zero-JavaScript Slides");
        assert_eq!(*lines.last().unwrap(), "7 ready, 0 blocked");
        assert!(text.contains("cloudflare"), "{text}");
        assert!(text.contains("wrangler.toml"), "{text}");
    }

    #[test]
    fn a_printed_plan_counts_what_is_blocked_and_says_what_to_write() {
        let plan = plan_publish(&PlanOptions {
            meta: DeckMetadata { url: None, ..meta() },
            targets: Some(vec![PublishTarget::Social]),
            ..PlanOptions::default()
        });

        assert!(format_plan(&plan).contains("add `url:`"), "{}", format_plan(&plan));
        assert_eq!(format_plan(&plan).lines().last().unwrap(), "0 ready, 1 blocked");
    }

    #[test]
    fn the_columns_of_a_two_step_plan_line_up_against_a_full_one() {
        // Or a diff shows what moved instead of what changed.
        let whole = format_plan(&complete());
        let part = format_plan(&plan_publish(&PlanOptions {
            meta: meta(),
            slides: slides(),
            artifacts: vec![pdf()],
            targets: Some(vec![PublishTarget::Blog]),
            ..PlanOptions::default()
        }));

        let line = |text: &str| {
            text.lines().find(|line| line.contains("blog")).map(str::to_string).expect("a line")
        };

        assert_eq!(line(&part), line(&whole));
    }

    #[test]
    fn a_shorter_budget_reaches_the_post() {
        let plan = plan_publish(&PlanOptions {
            meta: meta(),
            targets: Some(vec![PublishTarget::Social]),
            social: SocialOptions { limit: Some(120) },
            ..PlanOptions::default()
        });

        assert!(matches!(
            plan.steps[0].payload(),
            Some(ReadyPayload::Social(post)) if post.limit == 120
        ));
    }

    #[test]
    fn a_target_is_named_by_the_same_token_a_flag_takes() {
        for target in PUBLISH_TARGETS {
            assert_eq!(PublishTarget::parse(target.as_token()), Some(target));
        }
        assert_eq!(PublishTarget::parse("twitter"), None);
    }

    #[test]
    fn the_widest_target_name_is_the_column_the_plan_reserves_for_it() {
        // Measured rather than trusted: a destination added with a longer name
        // would otherwise shear every printed plan by one column.
        let widest = PUBLISH_TARGETS.iter().map(|t| t.as_token().len()).max().unwrap();

        assert_eq!(widest, TARGET_COLUMN);
        assert_eq!("blocked".len(), STATUS_COLUMN);
    }
}
