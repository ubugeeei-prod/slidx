//! The one door out of this crate.
//!
//! `@slidxjs/publish` is a wrapper rather than a second implementation, so every
//! function it exports has to reach this crate somehow. Twenty-odd
//! `#[wasm_bindgen]` functions would be twenty-odd places for the two sides to
//! drift, and each one would have to be declared again in the binding crate.
//! One tagged request instead: the set of operations is declared here, in Rust,
//! and a spelling JavaScript gets wrong fails to deserialise with a message
//! that lists what it could have said.
//!
//! Nothing here decides anything. Every variant is a name, a payload, and one
//! call into the module that owns the behaviour — if a rule appears in this
//! file, it is in the wrong place.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::links::{collect_links, DeckLink};
use crate::plan::{format_plan, is_ready, plan_publish, PlanOptions, PublishPlan};
use crate::talks::{build_talk_index, TalkIndex, TalkIndexOptions};
use crate::targets::{
    compose_archive, compose_blog, compose_cloudflare, compose_docswell, compose_resources,
    compose_social, compose_speaker_deck, describe_archive, describe_blog, describe_cloudflare,
    describe_docswell, describe_resources, describe_social, describe_speaker_deck, ArchiveRecord,
    BlogScaffold, CloudflarePages, DocswellUpload, ResourcesPage, SocialOptions, SocialPost,
    SpeakerDeckUpload,
};
use crate::text;
use crate::types::{Composed, DeckSource};

/// What a caller is asking for.
///
/// Reaches TypeScript as `PublishCall`, so the wrapper is type-checked against
/// the operations that exist rather than against a string it hopes is right.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "op", rename_all = "camelCase")]
#[ts(rename = "PublishCall")]
pub enum Call {
    Plan(PlanOptions),
    FormatPlan { plan: PublishPlan },
    IsReady { plan: PublishPlan },

    ComposeSpeakerDeck(DeckSource),
    ComposeDocswell(DeckSource),
    ComposeSocial { source: DeckSource, options: SocialOptions },
    ComposeBlog(DeckSource),
    ComposeResources(DeckSource),
    ComposeCloudflare(DeckSource),
    ComposeArchive(DeckSource),

    DescribeSpeakerDeck { upload: SpeakerDeckUpload },
    DescribeDocswell { upload: DocswellUpload },
    DescribeSocial { post: SocialPost },
    DescribeBlog { scaffold: BlogScaffold },
    DescribeResources { page: ResourcesPage },
    DescribeCloudflare { pages: CloudflarePages },
    DescribeArchive { record: ArchiveRecord },

    CollectLinks(DeckSource),
    BuildTalkIndex { records: Vec<ArchiveRecord>, options: TalkIndexOptions },

    CountCharacters { text: String },
    Truncate { text: String, limit: usize },
    AsciiSlug { text: String },
    FileSlug { text: String },
    FitSlug { slug: String, limit: usize },
    NormalizeTag { tag: String },
    UniqueTags { tags: Vec<String> },
    TidyBlock { text: String },
}

/// What one answers with.
///
/// Untagged, so each operation's result crosses as itself rather than wrapped
/// in a discriminator no caller would ever read: the caller already knows which
/// question it asked.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Answer {
    Plan(PublishPlan),
    Text(String),
    Flag(bool),
    Count(usize),
    Tags(Vec<String>),
    Links(Vec<DeckLink>),
    Index(TalkIndex),
    SpeakerDeck(Composed<SpeakerDeckUpload>),
    Docswell(Composed<DocswellUpload>),
    Social(Composed<SocialPost>),
    Blog(Composed<BlogScaffold>),
    Resources(Composed<ResourcesPage>),
    Cloudflare(Composed<CloudflarePages>),
    Archive(Composed<ArchiveRecord>),
}

impl Call {
    /// Performs the call. Pure, like everything under it.
    pub fn answer(self) -> Answer {
        match self {
            Self::Plan(options) => Answer::Plan(plan_publish(&options)),
            Self::FormatPlan { plan } => Answer::Text(format_plan(&plan)),
            Self::IsReady { plan } => Answer::Flag(is_ready(&plan)),

            Self::ComposeSpeakerDeck(source) => Answer::SpeakerDeck(compose_speaker_deck(&source)),
            Self::ComposeDocswell(source) => Answer::Docswell(compose_docswell(&source)),
            Self::ComposeSocial { source, options } => {
                Answer::Social(compose_social(&source, &options))
            }
            Self::ComposeBlog(source) => Answer::Blog(compose_blog(&source)),
            Self::ComposeResources(source) => Answer::Resources(compose_resources(&source)),
            Self::ComposeCloudflare(source) => Answer::Cloudflare(compose_cloudflare(&source)),
            Self::ComposeArchive(source) => Answer::Archive(compose_archive(&source)),

            Self::DescribeSpeakerDeck { upload } => Answer::Text(describe_speaker_deck(&upload)),
            Self::DescribeDocswell { upload } => Answer::Text(describe_docswell(&upload)),
            Self::DescribeSocial { post } => Answer::Text(describe_social(&post)),
            Self::DescribeBlog { scaffold } => Answer::Text(describe_blog(&scaffold)),
            Self::DescribeResources { page } => Answer::Text(describe_resources(&page)),
            Self::DescribeCloudflare { pages } => Answer::Text(describe_cloudflare(&pages)),
            Self::DescribeArchive { record } => Answer::Text(describe_archive(&record)),

            Self::CollectLinks(source) => Answer::Links(collect_links(&source)),
            Self::BuildTalkIndex { records, options } => {
                Answer::Index(build_talk_index(&records, &options))
            }

            Self::CountCharacters { text } => Answer::Count(text::count_characters(&text)),
            Self::Truncate { text, limit } => Answer::Text(text::truncate(&text, limit)),
            Self::AsciiSlug { text } => Answer::Text(text::ascii_slug(&text)),
            Self::FileSlug { text } => Answer::Text(text::file_slug(&text)),
            Self::FitSlug { slug, limit } => Answer::Text(text::fit_slug(&slug, limit)),
            Self::NormalizeTag { tag } => Answer::Text(text::normalize_tag(&tag)),
            Self::UniqueTags { tags } => Answer::Tags(text::unique_tags(&tags)),
            Self::TidyBlock { text } => Answer::Text(text::tidy_block(&text)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn answer(request: serde_json::Value) -> serde_json::Value {
        let call: Call = serde_json::from_value(request).expect("a call");

        serde_json::to_value(call.answer()).expect("an answer")
    }

    #[test]
    fn a_call_carries_its_payload_beside_the_name_of_the_operation() {
        let plan = answer(json!({
            "op": "plan",
            "meta": { "title": "Zero-JavaScript Slides" },
            "targets": ["blog"],
        }));

        assert_eq!(plan["deck"], "Zero-JavaScript Slides");
        assert_eq!(plan["steps"][0]["target"], "blog");
    }

    #[test]
    fn a_plan_goes_back_across_the_boundary_and_prints_the_same_either_side() {
        // `formatPlan` is handed a plan JavaScript is holding rather than one
        // this crate still has, so the whole shape has to survive the return
        // trip.
        let plan = answer(json!({
            "op": "plan",
            "meta": { "title": "Zero-JavaScript Slides", "url": "https://slidx.dev/t" },
        }));
        let printed = answer(json!({ "op": "formatPlan", "plan": plan }));

        assert!(
            printed.as_str().expect("text").starts_with("publish plan: Zero-JavaScript Slides"),
            "{printed}"
        );
    }

    #[test]
    fn an_operation_nobody_declared_is_refused_with_the_ones_that_exist() {
        // The reason the set of operations is declared here rather than as a
        // free string: a wrapper that misspells one finds out immediately.
        let error = serde_json::from_value::<Call>(json!({ "op": "tweet" })).expect_err("refused");

        assert!(error.to_string().contains("composeSocial"), "{error}");
    }

    #[test]
    fn a_composed_payload_crosses_as_the_union_javascript_narrows_on() {
        let blocked = answer(json!({ "op": "composeArchive", "meta": {} }));

        assert_eq!(blocked["ok"], false);
        assert_eq!(blocked["reasons"][0]["field"], "title");
    }

    #[test]
    fn a_pages_project_crosses_as_a_file_rather_than_a_login() {
        let ready = answer(json!({ "op": "composeCloudflare", "meta": { "title": "A talk" } }));

        assert_eq!(ready["ok"], true);
        assert_eq!(ready["value"]["command"], "wrangler pages deploy");
        assert!(
            ready["value"]["toml"].as_str().expect("toml").contains("pages_build_output_dir"),
            "{ready}"
        );
    }

    #[test]
    fn the_text_helpers_are_reachable_so_the_counting_rule_exists_once() {
        assert_eq!(answer(json!({ "op": "countCharacters", "text": "🎤" })), json!(1));
        assert_eq!(answer(json!({ "op": "truncate", "text": "abcdef", "limit": 3 })), json!("ab…"));
        assert_eq!(
            answer(json!({ "op": "uniqueTags", "tags": ["Rust", "rust"] })),
            json!(["rust"])
        );
    }
}
