//! Publishing a deck, as a plan rather than an act.
//!
//! The chore after a talk is not one job, it is five: the PDF onto two slide
//! hosts, a post that links to it, a write-up nobody starts, and the page of
//! links the audience photographed off the screen. All five are already
//! described by the frontmatter written at proposal time — the title, the
//! event, the hashtag, the url — so none of them should need typing twice.
//!
//! **This crate makes no network calls and takes no credentials.** It turns a
//! deck into typed payloads and returns them; something else — a CLI, a CI job,
//! the author with a browser open — performs them. That is a deliberate
//! boundary, not an unfinished one. A crate that can post as you is a crate
//! that has to be trusted with a token, and every token stored is a token that
//! can leak, be committed, or be used by a dependency you did not audit. There
//! is no HTTP client here to make it easy to cross that line later.
//!
//! What is left is a pure function from deck to plan, which has the pleasant
//! side effect of being trivially testable and diffable: the same deck plans
//! the same way every time, so `slidx publish --plan` is something you can read
//! before you mean it, and compare against what you did last time.
//!
//! # Determinism
//!
//! No clock, no filesystem, no iteration order that depends on anything but the
//! deck. Slides are sorted by index on the way in, tags keep the order the
//! author wrote them, and every collection this crate builds is a `Vec` rather
//! than a hash set. Two plans of the same deck are byte-identical, which is
//! what makes them diffable — and a plan that cannot be diffed cannot be
//! reviewed before it is meant.
//!
//! # One implementation
//!
//! `@slidxjs/publish` is a wrapper over this crate through
//! [`call::Call`], so the caps, the wording of every reason, and the order of
//! every list exist once. A second implementation in TypeScript would be a
//! second set of answers to "will Speaker Deck accept this title".

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod call;
pub mod fields;
pub mod links;
pub mod plan;
pub mod talks;
pub mod targets;
pub mod text;
pub mod types;

pub use call::{Answer, Call};
pub use links::{collect_links, DeckLink};
pub use plan::{
    blocked_steps, format_plan, is_ready, plan_publish, ready_steps, PlanOptions, PublishPlan,
    PublishStep, PublishTarget, ReadyPayload, PUBLISH_TARGETS,
};
pub use talks::{build_talk_index, TalkIndex, TalkIndexOptions};
pub use targets::{
    compose_archive, compose_blog, compose_docswell, compose_resources, compose_social,
    compose_speaker_deck, read_record, ArchiveRecord, BlogScaffold, BlogSection, DocswellUpload,
    ResourcesPage, SocialOptions, SocialPost, SpeakerDeckUpload, DEFAULT_POST_LIMIT,
};
pub use types::{
    reason, Artifact, ArtifactKind, BlockedReason, Composed, DeckMetadata, DeckSlide, DeckSource,
};
