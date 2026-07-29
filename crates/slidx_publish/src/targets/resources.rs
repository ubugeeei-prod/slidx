//! The page of links, so nobody has to squint at a URL on a projector.
//!
//! Every talk has the moment where a link appears on a slide and forty people
//! photograph it. The list already exists — it is scattered across the deck —
//! so the page is a collection job, not an authoring one, and collecting it by
//! hand afterwards is precisely the chore that does not get done.
//!
//! Order is the deck's order and the labels are the deck's words, because the
//! page is only useful if a reader can match an entry against the slide they
//! remember. Sorting it alphabetically would break that, so it is not sorted.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::links::{collect_links, DeckLink};
use crate::types::{reason, Composed, DeckSource};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesPage {
    /// Heading of the page.
    pub title: String,
    /// Suggested file name.
    pub path: String,
    /// Deduplicated, in slide order.
    pub links: Vec<DeckLink>,
    pub markdown: String,
}

/// Where the page is written, relative to wherever the caller is writing.
const RESOURCES_PATH: &str = "resources.md";

pub fn compose_resources(source: &DeckSource) -> Composed<ResourcesPage> {
    let links = collect_links(source);

    if links.is_empty() {
        return Composed::Blocked(vec![reason(
            "links",
            "no link appears anywhere in the deck — add `repo:` to the frontmatter, or link \
             something from a slide",
        )]);
    }

    // The only target that needs nothing from the frontmatter. A deck that
    // filled in none of it still has links in it, and this page is still worth
    // producing, so the heading falls back rather than blocking.
    let deck_title = source.meta.title.as_deref().unwrap_or_default().trim();
    let title = if deck_title.is_empty() {
        "Resources".to_string()
    } else {
        format!("Resources — {deck_title}")
    };

    let items: Vec<String> = links
        .iter()
        .map(|link| format!("- [{}]({})", escape_label(&link.label), link.url))
        .collect();

    Composed::Ready(ResourcesPage {
        markdown: format!("# {title}\n\n{}\n", items.join("\n")),
        title,
        path: RESOURCES_PATH.to_string(),
        links,
    })
}

/// Brackets in link text break the link they are in.
///
/// A label taken from a slide can contain anything the author typed, and a page
/// of resources whose third entry swallowed the fourth is worse than an escaped
/// bracket.
fn escape_label(label: &str) -> String {
    label.replace('[', "\\[").replace(']', "\\]")
}

/// One line for a printed plan.
pub fn describe_resources(page: &ResourcesPage) -> String {
    format!("write {} with {} link(s)", page.path, page.links.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DeckMetadata, DeckSlide};

    fn slide(index: u32, content: &str) -> DeckSlide {
        DeckSlide { index, content: Some(content.into()), ..DeckSlide::default() }
    }

    fn deck(title: Option<&str>, slides: Vec<DeckSlide>) -> DeckSource {
        DeckSource {
            meta: DeckMetadata { title: title.map(str::to_string), ..DeckMetadata::default() },
            slides,
            ..DeckSource::default()
        }
    }

    fn page(source: &DeckSource) -> ResourcesPage {
        compose_resources(source).value().cloned().expect("a page")
    }

    fn slides() -> Vec<DeckSlide> {
        vec![
            slide(0, "See [the docs](https://slidx.dev/docs)."),
            DeckSlide {
                index: 1,
                notes: Some(vec!["And https://slidx.dev/themes".into()]),
                ..DeckSlide::default()
            },
        ]
    }

    #[test]
    fn the_page_is_a_list_in_slide_order_under_a_heading_naming_the_deck() {
        assert_eq!(
            page(&deck(Some("Zero-JavaScript Slides"), slides())).markdown,
            "# Resources — Zero-JavaScript Slides\n\n\
             - [the docs](https://slidx.dev/docs)\n\
             - [slidx.dev/themes](https://slidx.dev/themes)\n"
        );
    }

    #[test]
    fn a_deck_with_no_frontmatter_at_all_still_gets_a_page() {
        // The one target that needs nothing from the author but the deck.
        assert_eq!(page(&deck(None, slides())).title, "Resources");
        assert_eq!(page(&deck(None, slides())).path, "resources.md");
    }

    #[test]
    fn a_bracket_in_a_label_is_escaped_rather_than_swallowing_the_next_entry() {
        let source = deck(None, vec![slide(0, "[draft [notes](https://slidx.dev/x)")]);

        assert!(
            page(&source).markdown.contains("- [draft \\[notes](https://slidx.dev/x)"),
            "{}",
            page(&source).markdown
        );
    }

    #[test]
    fn a_deck_that_links_to_nothing_is_told_how_to_fix_that() {
        let composed = compose_resources(&deck(None, vec![DeckSlide::default()]));

        assert_eq!(
            composed.reasons().iter().map(|r| r.field.as_str()).collect::<Vec<_>>(),
            ["links"]
        );
    }

    #[test]
    fn the_plan_line_counts_what_the_page_would_carry() {
        assert_eq!(
            describe_resources(&page(&deck(None, slides()))),
            "write resources.md with 2 link(s)"
        );
    }
}
