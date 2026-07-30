//! The list of a deck's pages, written by the thing that emitted them.
//!
//! A sitemap is the one SEO artefact an author should never maintain by hand:
//! the build knows every URL it wrote, and a hand-kept list is a list that is
//! wrong the first time a slide is inserted in the middle.
//!
//! Only the audience pages are listed. The presenter view carries the speaker's
//! notes, the print shell is the whole deck again on one page, and a snippet
//! page is reached from a slide by a QR — none of them is a page a search
//! result should land somebody on.
//!
//! Absolute URLs are not a style choice here: `<loc>` is defined as a full URL,
//! and a relative one makes the file invalid rather than lenient. That is why
//! there is no sitemap at all for a deck nobody has told slidx the address of.

use slidx_core::Deck;

use crate::url::{resolve, slide_path};

/// Where the file goes, relative to the deck's own output root.
///
/// Inside the deck rather than at the site root, because a sitemap may only
/// list URLs at or below its own directory — and a deck is usually one part of
/// somebody's site rather than the whole of it.
pub const FILE_NAME: &str = "sitemap.xml";

/// The sitemap for a deck deployed at `deck_url`.
///
/// A draft deck gets an empty list rather than no file. A deck that was public
/// last week is already in a crawler's queue, and an empty `urlset` retracts
/// the pages it listed; a missing file only looks like a deploy that half
/// finished, which a crawler will retry.
pub fn render(deck: &Deck, deck_url: &str) -> String {
    let mut body = String::new();

    if !deck.meta.is_draft() {
        for slide in &deck.slides {
            let location = resolve(deck_url, &slide_path(slide.index));
            body.push_str(&format!("  <url><loc>{}</loc></url>\n", escape(&location)));
        }
    }

    // No `<lastmod>`. The build has no idea when a slide last changed — it
    // would have to write today's date, which tells a crawler that every page
    // of the deck changed every time the deck was rebuilt.
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n\
         {body}</urlset>\n"
    )
}

/// XML escaping, which is not the same set as HTML's.
///
/// A URL with a query in it is the case that matters: an unescaped `&` makes
/// the whole file unparseable, so a crawler drops every page rather than the
/// one that had the ampersand.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    const PUBLIC: &str = "---\ndraft: false\n---\n\n";

    fn sitemap(source: &str) -> String {
        let deck = parse_deck(source, &DeckParseOptions::default());
        render(&deck, "https://example.com/talk/")
    }

    #[test]
    fn every_slide_is_listed_at_the_url_it_was_written_to() {
        let xml = sitemap(&format!("{PUBLIC}# One\n\n---\n\n# Two\n\n---\n\n# Three\n"));

        assert!(xml.contains("<loc>https://example.com/talk/</loc>"));
        assert!(xml.contains("<loc>https://example.com/talk/2/</loc>"));
        assert!(xml.contains("<loc>https://example.com/talk/3/</loc>"));
        assert_eq!(xml.matches("<url>").count(), 3);
    }

    #[test]
    fn a_draft_deck_lists_nothing_at_all() {
        let xml = sitemap("# One\n\n---\n\n# Two\n");

        assert!(!xml.contains("<url>"), "{xml}");
        assert!(xml.contains("<urlset"), "the file still exists, and retracts:\n{xml}");
    }

    #[test]
    fn the_file_is_well_formed_xml_with_the_namespace_a_crawler_looks_for() {
        let xml = sitemap(&format!("{PUBLIC}# One\n"));

        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\""));
        assert!(xml.trim_end().ends_with("</urlset>"));
    }

    #[test]
    fn no_page_claims_a_date_the_build_would_have_had_to_invent() {
        assert!(!sitemap(&format!("{PUBLIC}# One\n")).contains("lastmod"));
    }

    #[test]
    fn an_ampersand_in_a_deck_url_is_escaped_rather_than_breaking_the_file() {
        let deck = parse_deck(&format!("{PUBLIC}# One\n"), &DeckParseOptions::default());
        let xml = render(&deck, "https://example.com/?deck=a&b=1");

        assert!(xml.contains("deck=a&amp;b=1"), "{xml}");
    }
}
