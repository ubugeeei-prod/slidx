//! The post that says the slides are up.
//!
//! A social post is the one target with a hard character budget, so it is the
//! one target that composes rather than maps. The composition rule is fixed and
//! worth stating plainly, because it is what the tests are about:
//!
//! **The link and the hashtag are never what gets cut.** A post that loses a
//! clause is a slightly worse post. A post that loses its URL is a post that
//! did not do the one thing it existed for, and a post that loses its hashtag
//! is invisible to everyone following the conference. So the budget is spent on
//! those first, then on the title, and the description gets whatever is left.
//!
//! There is no invented sentence around it — no "Slides are up!", no "Thanks
//! everyone!". The deck's own words are the post. Boilerplate here would be
//! English text inserted into the timeline of an author who wrote their deck in
//! Japanese, in slidx's voice rather than theirs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::text::{count_characters, normalize_tag, truncate};
use crate::types::{reason, ArtifactKind, BlockedReason, Composed, DeckSource};

/// The default budget.
///
/// 280 is the shortest limit among the networks people announce talks on, so a
/// post composed for it fits everywhere without a per-network variant. Callers
/// with a longer budget pass one.
pub const DEFAULT_POST_LIMIT: usize = 280;

/// Below this, a description is not worth the space.
///
/// A four-word fragment ending in an ellipsis reads as a bug, and costs the
/// characters that made the title readable. Under the floor the description is
/// dropped whole.
const DESCRIPTION_FLOOR: usize = 24;

/// Blank line between the parts.
const SEPARATOR: &str = "\n\n";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SocialPost {
    pub text: String,
    /// Characters, counted as the platform counts them. Never above `limit`.
    pub length: usize,
    pub limit: usize,
    /// True when the description was shortened or dropped to fit.
    pub truncated: bool,
    /// Card image to attach, when the build produced one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub image: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct SocialOptions {
    /// Character budget. Defaults to [`DEFAULT_POST_LIMIT`].
    #[ts(optional)]
    pub limit: Option<usize>,
}

pub fn compose_social(source: &DeckSource, options: &SocialOptions) -> Composed<SocialPost> {
    let limit = options.limit.unwrap_or(DEFAULT_POST_LIMIT);
    let meta = &source.meta;
    let mut reasons: Vec<BlockedReason> = Vec::new();

    let title = meta.title.as_deref().unwrap_or_default().trim();
    let url = meta.url.as_deref().unwrap_or_default().trim();

    if title.is_empty() {
        reasons
            .push(reason("title", "a post needs a title — add `title:` to the deck frontmatter"));
    }

    // The whole point of the post is the link. Emitting one without it would be
    // an announcement of slides nobody can reach.
    if url.is_empty() {
        reasons.push(reason(
            "url",
            "a post needs somewhere to send people — add `url:` with the published deck's address",
        ));
    }

    if !reasons.is_empty() {
        return Composed::Blocked(reasons);
    }

    let hashtag = meta.hashtag.as_deref().map(normalize_tag).unwrap_or_default();
    let event = meta.event.as_deref().unwrap_or_default().trim();

    let lead = if event.is_empty() { title.to_string() } else { format!("{title} — {event}") };
    let tail = if hashtag.is_empty() { url.to_string() } else { format!("{url} #{hashtag}") };

    let fixed = count_characters(&lead) + SEPARATOR.len() + count_characters(&tail);

    if fixed > limit {
        return Composed::Blocked(vec![mandatory_parts_do_not_fit(&lead, &tail, limit)]);
    }

    let description = meta.description.as_deref().unwrap_or_default().trim();

    // Saturating, because the mandatory parts are allowed to spend the budget
    // exactly. A post that fits with nothing to spare leaves no room for a
    // separator either, which is a description of zero rather than an error.
    let available = limit.saturating_sub(fixed).saturating_sub(SEPARATOR.len());
    let body = if available >= DESCRIPTION_FLOOR {
        truncate(description, available)
    } else {
        String::new()
    };
    let truncated = !description.is_empty() && body != description;

    let text = [lead, body, tail]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(SEPARATOR);

    Composed::Ready(SocialPost {
        length: count_characters(&text),
        text,
        limit,
        truncated,
        image: source.artifact(ArtifactKind::Card).map(|card| card.path.clone()),
    })
}

/// Which field to name when even the mandatory parts overflow.
///
/// The URL is not shortenable by the author in any useful sense, so when it
/// alone blows the budget the honest report is about the budget. Otherwise the
/// title is the part that can be edited, and saying so is more use than saying
/// the post is too long.
fn mandatory_parts_do_not_fit(lead: &str, tail: &str, limit: usize) -> BlockedReason {
    let tail_length = count_characters(tail);

    if tail_length > limit {
        return reason(
            "url",
            format!(
                "the URL and hashtag need {tail_length} characters of a {limit}-character post \
                 — shorten the URL or raise the budget"
            ),
        );
    }

    reason(
        "title",
        format!(
            "the title, URL, and hashtag need {} characters of a {limit}-character post — \
             shorten `title`",
            count_characters(lead) + SEPARATOR.len() + tail_length
        ),
    )
}

/// One line for a printed plan.
pub fn describe_social(post: &SocialPost) -> String {
    format!(
        "compose a {}/{} character post{}",
        post.length,
        post.limit,
        if post.truncated { ", description shortened" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Artifact, DeckMetadata};

    const URL: &str = "https://slidx.dev/talks/zero-js";
    const TAIL: &str = "https://slidx.dev/talks/zero-js #slidxconf";
    const DESCRIPTION: &str =
        "Why a deck should be plain HTML, and what it costs to keep it that way.";

    fn meta() -> DeckMetadata {
        DeckMetadata {
            title: Some("Zero-JavaScript Slides".into()),
            description: Some(DESCRIPTION.into()),
            event: Some("SlidxConf 2026".into()),
            hashtag: Some("slidxconf".into()),
            url: Some(URL.into()),
            ..DeckMetadata::default()
        }
    }

    fn post(meta: DeckMetadata, limit: Option<usize>) -> SocialPost {
        let source = DeckSource { meta, ..DeckSource::default() };
        let composed = compose_social(&source, &SocialOptions { limit });

        composed.value().cloned().expect("a post")
    }

    fn fields(meta: DeckMetadata, limit: Option<usize>) -> Vec<String> {
        let source = DeckSource { meta, ..DeckSource::default() };

        compose_social(&source, &SocialOptions { limit })
            .reasons()
            .iter()
            .map(|reason| reason.field.clone())
            .collect()
    }

    #[test]
    fn the_talk_leads_the_description_follows_and_the_link_ends_it() {
        assert_eq!(
            post(meta(), None).text,
            format!("Zero-JavaScript Slides — SlidxConf 2026\n\n{DESCRIPTION}\n\n{TAIL}")
        );
    }

    #[test]
    fn a_post_uses_only_the_decks_own_words() {
        // No "Slides are up!". Boilerplate here is slidx's voice inserted into
        // the timeline of an author who wrote their deck in another language.
        let written = post(meta(), None);

        assert!(written.text.starts_with("Zero-JavaScript Slides"));
        assert!(written.text.ends_with(TAIL));
    }

    #[test]
    fn a_deck_naming_no_event_leads_with_the_title_alone() {
        assert!(post(DeckMetadata { event: None, ..meta() }, None)
            .text
            .starts_with("Zero-JavaScript Slides\n\n"));
    }

    #[test]
    fn a_hashtag_is_written_the_way_a_platform_stores_one() {
        let written = post(DeckMetadata { hashtag: Some("#Slidx Conf".into()), ..meta() }, None);

        assert!(written.text.ends_with("#slidx-conf"), "{}", written.text);
        assert!(post(DeckMetadata { hashtag: None, ..meta() }, None).text.ends_with(URL));
    }

    #[test]
    fn the_card_the_build_produced_is_attached_and_nothing_is_when_it_did_not() {
        let source = DeckSource {
            meta: meta(),
            artifacts: vec![Artifact {
                kind: ArtifactKind::Card,
                path: "dist/card.png".into(),
                bytes: None,
            }],
            ..DeckSource::default()
        };
        let composed = compose_social(&source, &SocialOptions::default());

        assert_eq!(composed.value().expect("a post").image.as_deref(), Some("dist/card.png"));
        assert_eq!(post(meta(), None).image, None);
    }

    #[test]
    fn the_description_is_cut_rather_than_the_link() {
        let written = post(DeckMetadata { description: Some("a".repeat(500)), ..meta() }, None);

        assert!(written.text.ends_with(TAIL));
        assert!(written.truncated);
        assert!(written.text.starts_with("Zero-JavaScript Slides — SlidxConf 2026"));
    }

    #[test]
    fn a_post_lands_exactly_on_the_budget_rather_than_under_it() {
        // Characters left unspent are characters of the author's description
        // thrown away for nothing.
        assert_eq!(
            post(DeckMetadata { description: Some("a".repeat(500)), ..meta() }, None).length,
            280
        );
    }

    #[test]
    fn a_description_with_no_word_boundaries_is_still_cut_to_fit() {
        // Japanese has no spaces to cut on, and a rule that needed one would
        // throw away the whole budget.
        let description = "これは日本語の説明文です".repeat(40);
        let written = post(DeckMetadata { description: Some(description), ..meta() }, None);

        assert_eq!(written.length, 280);
        assert!(written.text.contains(TAIL));
    }

    #[test]
    fn a_description_that_would_be_a_stub_is_dropped_whole() {
        let written = post(meta(), Some(100));

        assert!(!written.text.contains("Why a deck"), "{}", written.text);
        assert!(!written.text.contains('…'), "{}", written.text);
        assert!(written.truncated);
    }

    #[test]
    fn no_budget_is_ever_exceeded_and_the_link_survives_all_of_them() {
        for limit in [90, 120, 180, 280, 400] {
            let written =
                post(DeckMetadata { description: Some("a".repeat(900)), ..meta() }, Some(limit));

            assert!(written.length <= limit, "{limit}: {}", written.length);
            assert!(written.text.contains(URL), "{limit}");
            assert!(written.text.contains("#slidxconf"), "{limit}");
        }
    }

    #[test]
    fn an_emoji_in_the_title_costs_one_character_of_the_budget() {
        // Counting UTF-16 units would make this post one over a budget it fits.
        let meta = DeckMetadata {
            title: Some("🎤 Zero-JavaScript Slides".into()),
            description: None,
            ..meta()
        };

        assert_eq!(post(meta.clone(), None).length, 85);
        assert!(fields(meta.clone(), Some(85)).is_empty());
        assert_eq!(fields(meta, Some(84)), ["title"]);
    }

    #[test]
    fn a_deck_with_no_url_is_reported_rather_than_posted_as_a_dangling_announcement() {
        assert_eq!(fields(DeckMetadata { url: None, ..meta() }, None), ["url"]);
        assert_eq!(fields(DeckMetadata { title: None, ..meta() }, None), ["title"]);
        assert_eq!(
            fields(DeckMetadata { title: None, url: None, ..meta() }, None),
            ["title", "url"]
        );
    }

    #[test]
    fn the_url_is_named_when_it_alone_will_not_fit_the_budget() {
        assert_eq!(fields(meta(), Some(30)), ["url"]);
    }

    #[test]
    fn the_title_is_named_when_the_title_is_what_overflows() {
        // The URL cannot usefully be shortened by the author; the title can, so
        // that is the field the message names.
        assert_eq!(
            fields(DeckMetadata { title: Some("a".repeat(300)), ..meta() }, None),
            ["title"]
        );
    }

    #[test]
    fn the_plan_line_says_how_much_of_the_budget_the_post_spends() {
        assert_eq!(describe_social(&post(meta(), None)), "compose a 156/280 character post");
        assert!(describe_social(&post(
            DeckMetadata { description: Some("a".repeat(500)), ..meta() },
            None
        ))
        .ends_with("description shortened"));
    }
}
