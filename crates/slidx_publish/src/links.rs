//! Every link a deck mentions, in the order the audience met it.
//!
//! The resources page is built from this, and the thing that makes such a page
//! worth generating is that it is exhaustive: a link an author read out loud
//! from a slide and never wrote down anywhere else is exactly the one an
//! attendee comes looking for afterwards.
//!
//! What a link *looks like* is [`scan`]'s business. What lives here is the
//! policy over the result — which links belong to the deck, which of two
//! spellings is the same resource, and what order the list is in.

pub mod scan;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::DeckSource;

pub use scan::{is_http, label_for_url};

/// A link, attributed to where it first appeared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeckLink {
    /// As authored, minus trailing punctuation that belonged to the sentence.
    pub url: String,
    /// Link text where there was some, otherwise the URL without its scheme.
    pub label: String,
    /// Slide index, or null for a link that came from the frontmatter.
    pub slide: Option<u32>,
}

/// Deck links, deduplicated, in slide order.
///
/// The repository comes first because it is deck-level: it belongs to the talk
/// rather than to any one slide. The deck's own canonical url is deliberately
/// absent — a resources page that links to the page it is part of is a loop,
/// not a resource.
///
/// First mention wins on both label and position. The first time a link appears
/// is where it was introduced, which is where its text is most likely to say
/// what it is.
pub fn collect_links(source: &DeckSource) -> Vec<DeckLink> {
    let mut found: Vec<DeckLink> = Vec::new();

    if let Some(repo) = source.meta.repo.as_deref().filter(|repo| is_http(repo)) {
        found.push(DeckLink { url: repo.to_string(), label: label_for_url(repo), slide: None });
    }

    for slide in source.ordered_slides() {
        // Body before notes: the audience saw the slide, the speaker read the
        // notes, and a link in both should be attributed to the one on screen.
        let body = slide.content.clone().unwrap_or_default();
        let notes = slide.notes.clone().unwrap_or_default();

        for block in std::iter::once(&body).chain(notes.iter()) {
            for link in scan::scan(block) {
                found.push(DeckLink { url: link.url, label: link.label, slide: Some(slide.index) });
            }
        }
    }

    dedupe(found)
}

fn dedupe(links: Vec<DeckLink>) -> Vec<DeckLink> {
    let mut seen: Vec<String> = Vec::new();
    let mut kept: Vec<DeckLink> = Vec::new();

    for link in links {
        let key = canonical_key(&link.url);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        kept.push(link);
    }

    kept
}

/// The identity two links share when they are the same resource.
///
/// Scheme and host case-fold, because they are case-insensitive by spec. Path,
/// query, and fragment are left exactly as written: a fragment addresses a
/// section, and collapsing two anchors of one long page into a single entry
/// loses the part that made each of them worth listing.
fn canonical_key(url: &str) -> String {
    let Some(authority) = url.find("://").map(|at| at + 3) else {
        // Something with no authority is still a string an author wrote, and an
        // exact match is the only claim about it that is safe to make.
        return url.to_string();
    };

    let end = url[authority..].find(['/', '?', '#']).map_or(url.len(), |offset| authority + offset);

    format!("{}{}", url[..end].to_ascii_lowercase(), &url[end..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DeckMetadata, DeckSlide};

    const REPO: &str = "https://github.com/ubugeeei-prod/slidx";

    fn deck(slides: Vec<DeckSlide>) -> DeckSource {
        DeckSource { slides, ..DeckSource::default() }
    }

    fn slide(index: u32, content: &str) -> DeckSlide {
        DeckSlide { index, content: Some(content.into()), ..DeckSlide::default() }
    }

    fn urls(source: &DeckSource) -> Vec<String> {
        collect_links(source).into_iter().map(|link| link.url).collect()
    }

    #[test]
    fn a_link_the_speaker_only_mentioned_in_the_notes_is_still_collected() {
        // Exactly the one somebody comes looking for afterwards.
        let slides = vec![DeckSlide {
            index: 0,
            notes: Some(vec!["Mentioned https://slidx.dev/docs here.".into()]),
            ..DeckSlide::default()
        }];

        assert_eq!(urls(&deck(slides)), ["https://slidx.dev/docs"]);
    }

    #[test]
    fn a_link_on_the_slide_is_attributed_before_the_same_slides_notes() {
        let slides = vec![DeckSlide {
            index: 0,
            content: Some("[on screen](https://slidx.dev/a)".into()),
            notes: Some(vec!["[in the notes](https://slidx.dev/b)".into()]),
            ..DeckSlide::default()
        }];

        assert_eq!(urls(&deck(slides)), ["https://slidx.dev/a", "https://slidx.dev/b"]);
    }

    #[test]
    fn links_are_listed_in_slide_order_however_the_slides_arrive() {
        let slides =
            vec![slide(1, "[b](https://slidx.dev/b)"), slide(0, "[a](https://slidx.dev/a)")];

        assert_eq!(urls(&deck(slides)), ["https://slidx.dev/a", "https://slidx.dev/b"]);
    }

    #[test]
    fn the_first_mention_of_a_link_is_the_one_that_names_it() {
        // The first time a link appears is where it was introduced, which is
        // where its text is most likely to say what it is.
        let slides = vec![
            slide(0, "[the parser docs](https://slidx.dev/docs)"),
            slide(1, "[docs again](https://slidx.dev/docs)"),
        ];
        let links = collect_links(&deck(slides));

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].label, "the parser docs");
        assert_eq!(links[0].slide, Some(0));
    }

    #[test]
    fn a_host_written_with_capitals_is_the_same_link() {
        let slides = vec![slide(0, "https://slidx.dev/docs"), slide(1, "https://SLIDX.dev/docs")];

        assert_eq!(urls(&deck(slides)).len(), 1);
    }

    #[test]
    fn two_anchors_of_one_page_are_two_links() {
        // A fragment addresses a section. Collapsing them loses the part that
        // made each worth listing.
        let slides = vec![
            slide(0, "https://slidx.dev/docs#steps"),
            slide(1, "https://slidx.dev/docs#themes"),
        ];

        assert_eq!(urls(&deck(slides)).len(), 2);
    }

    #[test]
    fn a_path_written_with_capitals_is_a_different_link() {
        // Paths are case-sensitive by spec, and two of them are two resources
        // until a server says otherwise.
        let slides = vec![slide(0, "https://slidx.dev/Docs"), slide(1, "https://slidx.dev/docs")];

        assert_eq!(urls(&deck(slides)).len(), 2);
    }

    #[test]
    fn the_repository_leads_the_list_because_it_belongs_to_the_talk() {
        let source = DeckSource {
            meta: DeckMetadata { repo: Some(REPO.into()), ..DeckMetadata::default() },
            slides: vec![slide(0, "[docs](https://slidx.dev/docs)")],
            ..DeckSource::default()
        };
        let links = collect_links(&source);

        assert_eq!(links[0].url, REPO);
        assert_eq!(links[0].slide, None);
    }

    #[test]
    fn the_repository_is_not_listed_twice_when_a_slide_links_it_too() {
        let source = DeckSource {
            meta: DeckMetadata { repo: Some(REPO.into()), ..DeckMetadata::default() },
            slides: vec![slide(0, &format!("[repo]({REPO})"))],
            ..DeckSource::default()
        };

        assert_eq!(collect_links(&source).len(), 1);
    }

    #[test]
    fn the_decks_own_url_is_not_a_resource_on_its_own_resources_page() {
        // A page of resources that links to the page it is part of is a loop.
        let source = DeckSource {
            meta: DeckMetadata {
                url: Some("https://slidx.dev/talks/zero-js".into()),
                ..DeckMetadata::default()
            },
            ..DeckSource::default()
        };

        assert!(collect_links(&source).is_empty());
    }

    #[test]
    fn a_repository_that_is_not_a_web_link_is_left_out() {
        // `git@github.com:…` is a remote, not something an attendee can open.
        let source = DeckSource {
            meta: DeckMetadata {
                repo: Some("git@github.com:ubugeeei-prod/slidx.git".into()),
                ..DeckMetadata::default()
            },
            ..DeckSource::default()
        };

        assert!(collect_links(&source).is_empty());
    }
}
