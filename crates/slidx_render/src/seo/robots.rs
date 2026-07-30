//! What a crawler is told before it reads anything.
//!
//! This is the coarse instrument and the `noindex` on each page is the precise
//! one, which is why a draft deck gets both. They fail differently: a `robots`
//! meta travels with the page and holds wherever the deck is deployed, but only
//! after the page has been fetched; a `Disallow` stops the fetch, and lives at a
//! fixed site-wide address that says nothing about where the deck ended up.
//!
//! Whoever deploys the deck owns `/robots.txt`, so a project that already has
//! one keeps it — see the plugin, which does not overwrite an author's file.

use slidx_core::Deck;

use crate::seo::sitemap;
use crate::url::resolve;

/// Where the file goes: the site root, and nowhere else.
///
/// A crawler reads `/robots.txt` and only `/robots.txt`. A copy inside the deck
/// directory would be a file nothing ever asks for.
pub const FILE_NAME: &str = "robots.txt";

/// The `robots.txt` for a deck mounted at `deck_path`.
///
/// `deck_path` is root-relative and ends in a slash — `/slides/` for the
/// default layout, `/` for a deck that is the whole site — because a `Disallow`
/// is matched as a prefix against the path of a request.
pub fn render(deck: &Deck, deck_path: &str, deck_url: Option<&str>, presenter: bool) -> String {
    let mut lines = vec!["# Written by slidx from the deck's own frontmatter.".to_string()];

    if deck.meta.is_draft() {
        lines.push("# This deck has not said it is public, so nothing in it is offered.".into());
        lines.push("User-agent: *".into());
        lines.push(format!("Disallow: {deck_path}"));

        // No `Sitemap:` line. There is nothing being offered, and pointing at a
        // list that is deliberately empty would only invite a fetch.
        return finish(lines);
    }

    lines.push("User-agent: *".into());

    if presenter {
        // The speaker's own view of a slide carries the notes for it. Those are
        // the author's private half of the talk, and a crawler that never
        // fetches them cannot cache them either.
        lines.push("# The speaker's view of a slide carries the notes for it.".into());
        lines.push(format!("Disallow: {deck_path}presenter/"));
        // Slide one is the deck root and the rest are a directory down, so the
        // second form needs a wildcard. It is an extension rather than the
        // original specification; a crawler that does not understand it reads
        // the line as a literal path, matches nothing, and the `noindex` on the
        // page is what holds.
        lines.push(format!("Disallow: {deck_path}*/presenter/"));
    }

    if let Some(deck_url) = deck_url {
        lines.push(String::new());
        lines.push(format!("Sitemap: {}", resolve(deck_url, sitemap::FILE_NAME)));
    }

    finish(lines)
}

fn finish(mut lines: Vec<String>) -> String {
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    const PUBLIC: &str = "---\ndraft: false\n---\n\n# One\n";

    fn deck(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    #[test]
    fn a_draft_deck_is_disallowed_wherever_it_is_mounted() {
        let text = render(&deck("# One\n"), "/slides/", Some("https://example.com/talk/"), true);

        assert!(text.contains("User-agent: *"));
        assert!(text.contains("Disallow: /slides/"));
    }

    #[test]
    fn a_draft_deck_offers_no_sitemap_to_fetch() {
        let text = render(&deck("# One\n"), "/slides/", Some("https://example.com/talk/"), true);

        assert!(!text.contains("Sitemap:"), "{text}");
    }

    #[test]
    fn a_deck_that_is_the_whole_site_disallows_the_root_when_it_is_a_draft() {
        assert!(render(&deck("# One\n"), "/", None, false).contains("Disallow: /\n"));
    }

    #[test]
    fn a_published_deck_disallows_nothing_but_the_speakers_own_pages() {
        let text = render(&deck(PUBLIC), "/slides/", None, true);

        assert!(text.contains("Disallow: /slides/presenter/"));
        assert!(text.contains("Disallow: /slides/*/presenter/"));
        assert!(!text.contains("Disallow: /slides/\n"), "the deck itself is offered:\n{text}");
    }

    #[test]
    fn a_build_with_no_speakers_pages_says_nothing_about_them() {
        // A `Disallow` for a path this build never wrote would be a rule about
        // a page that does not exist.
        let text = render(&deck(PUBLIC), "/slides/", None, false);

        assert!(!text.contains("presenter"), "{text}");
    }

    #[test]
    fn a_published_deck_names_where_its_sitemap_is() {
        let text = render(&deck(PUBLIC), "/slides/", Some("https://example.com/talk/"), true);

        assert!(text.contains("Sitemap: https://example.com/talk/sitemap.xml"), "{text}");
    }

    #[test]
    fn a_deck_with_no_url_still_gets_a_usable_file() {
        // Every directive in it is root-relative, so none of them needed an
        // origin. Only the sitemap did.
        let text = render(&deck(PUBLIC), "/slides/", None, true);

        assert!(text.contains("User-agent: *"));
        assert!(!text.contains("Sitemap:"));
    }

    #[test]
    fn the_file_ends_with_a_newline_like_every_other_text_file() {
        assert!(render(&deck(PUBLIC), "/slides/", None, true).ends_with('\n'));
    }
}
