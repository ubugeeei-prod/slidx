//! What a crawler and a link preview get.
//!
//! A built deck starts from an unusually good position: one document per slide,
//! a real URL each, the words in the markup, and a page that renders before any
//! script runs. What was missing is everything a crawler and a social card
//! reader look for *beyond* that — which of two hostnames is the real one, where
//! the rest of the deck is, what this slide is about, and what to draw when
//! somebody pastes the link.
//!
//! # Whose URL is it
//!
//! A canonical link, a sitemap entry and an `og:url` are all absolute by
//! definition, and a build genuinely does not know the origin it will be
//! deployed to. So the origin is something someone says: `url:` in the deck's
//! frontmatter, which authors already write for the QR codes and the published
//! description, or the plugin's own option when a deployment knows better than
//! the file does.
//!
//! When nobody has said, **nothing absolute is emitted.** No canonical, no
//! `og:url`, no sitemap. A guessed origin is not a smaller version of the right
//! answer: a canonical pointing at somebody else's host tells a search engine to
//! drop this deck in favour of a page that does not exist. What still works
//! without one is everything that was never absolute — the description, the
//! prev/next pair, the card, the structured data — and those are emitted
//! relative, because a relative URL that resolves is worth more than an
//! absolute one that lies.
//!
//! This is the same rule the QR codes already follow: no URL means no code
//! rather than a code that scans to nothing.
//!
//! # One place
//!
//! Every crawler-facing decision is here, so the answer to "is this deck
//! indexable" is given once and read by the page, the sitemap and the
//! `robots.txt` together. The three used to be able to disagree only because
//! two of them did not exist.

use slidx_core::{Deck, Slide};

use crate::og::{OG_HEIGHT, OG_WIDTH};
use crate::url::{resolve, slide_path, up_to_root};

pub mod description;
pub mod jsonld;
pub mod robots;
pub mod sitemap;

pub use description::describe;

/// The tag that keeps a page out of a search index.
///
/// `noindex` alone rather than `noindex, nofollow`: the links out of a slide go
/// to the rest of the same deck, which is covered by the same tag on every one
/// of those pages, and to sources the author cited, which are somebody else's
/// pages to index.
pub const NOINDEX: &str = "<meta name=\"robots\" content=\"noindex\">";

/// [`NOINDEX`] and a newline for a deck that has not said it is public.
///
/// For the pages that are neither a slide nor private: a shared snippet is
/// meant to be found once the talk exists, and must not be findable before it
/// does — the code on it is from a talk nobody has announced.
pub fn noindex_line(deck: &Deck) -> String {
    if deck.meta.is_draft() {
        format!("{NOINDEX}\n")
    } else {
        String::new()
    }
}

/// What the pages of a deck may say about themselves.
#[derive(Debug, Clone)]
pub struct SeoOptions {
    /// Absolute URL of the deck's root, when anyone has said what it is.
    pub deck_url: Option<String>,
    /// Where the deck is mounted in the site, root-relative and slash-ended.
    ///
    /// Only `robots.txt` needs it, because that file sits at the site root and
    /// has to name the deck from there. Everything else in a page is either
    /// relative to the page or absolute from [`SeoOptions::deck_url`].
    pub deck_path: String,
    /// True when this build drew social cards, so a page may point at one.
    pub cards: bool,
    /// True when this build also wrote the speaker's own pages.
    pub presenter: bool,
}

impl Default for SeoOptions {
    fn default() -> Self {
        Self { deck_url: None, deck_path: "/".to_string(), cards: false, presenter: false }
    }
}

/// Everything a crawler and a link preview read, for one slide's page.
///
/// Returned as lines the shell drops into its `<head>`. Each is complete on its
/// own — a caller cannot get half of this and a page cannot end up with an
/// `og:image` and no canonical because two call sites disagreed.
//
// A translated deck would add `<link rel="alternate" hreflang="…">` next to the
// canonical, one per language, pointing at the same slide in each. It belongs
// here, beside the canonical it qualifies.
pub fn head(deck: &Deck, slide: &Slide, options: &SeoOptions) -> String {
    let mut lines: Vec<String> = Vec::new();

    let name = page_name(deck, slide);
    let described = describe(slide);
    let canonical = options.deck_url.as_deref().map(|url| page_url(url, slide));
    let card = card_url(slide, options);

    if let Some(text) = &described {
        lines.push(meta("description", text));
    }

    // A deck nobody has published is kept out of the index by the page itself
    // as well as by `robots.txt`, because only one of the two travels with the
    // deck to wherever it is actually deployed.
    if deck.meta.is_draft() {
        lines.push(NOINDEX.to_string());
    }

    if let Some(canonical) = &canonical {
        lines.push(format!("<link rel=\"canonical\" href=\"{}\">", escape(canonical)));
    }

    // A deck is a sequence of pages, which is a thing the markup can say
    // outright rather than leaving a reader's tooling to infer from a footer.
    let up = up_to_root(slide.index);
    if slide.index > 0 {
        lines.push(sequence("prev", &format!("{up}{}", slide_path(slide.index - 1))));
    }
    if (slide.index as usize + 1) < deck.slides.len() {
        lines.push(sequence("next", &format!("{up}{}", slide_path(slide.index + 1))));
    }

    lines.push(property("og:title", &name));
    if let Some(text) = &described {
        lines.push(property("og:description", text));
    }

    // The deck's title, when it is not already the title of this page. It reads
    // as the publication a slide belongs to, which is what a preview shows in
    // small type above the headline.
    if let Some(deck_title) = deck.meta.title.as_deref().filter(|title| *title != name) {
        lines.push(property("og:site_name", deck_title));
    }

    if let Some(canonical) = &canonical {
        lines.push(property("og:url", canonical));
    }

    if let Some(card) = &card {
        lines.push(property("og:image", card));
        lines.push(property("og:image:width", &OG_WIDTH.to_string()));
        lines.push(property("og:image:height", &OG_HEIGHT.to_string()));
        // The only tag X has no Open Graph equivalent for; it reads the rest of
        // the card from `og:*`. Without this one the image is cropped to a small
        // square beside the title rather than shown above it.
        lines.push(meta("twitter:card", "summary_large_image"));
    }

    lines.push(jsonld::document(
        deck,
        slide,
        &jsonld::Facts {
            url: canonical.as_deref(),
            deck_url: options.deck_url.as_deref(),
            // Relative only in the page, where a browser resolves it. Structured
            // data is read out of context — copied into an index, a feed, an
            // archive — so a relative card there would resolve against whatever
            // fetched it.
            card: card.as_deref().filter(|_| options.deck_url.is_some()),
            description: described.as_deref(),
            title: &name,
        },
    ));

    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// The sitemap for this deck, or nothing when there is no URL to write in it.
pub fn render_sitemap(deck: &Deck, options: &SeoOptions) -> Option<String> {
    Some(sitemap::render(deck, options.deck_url.as_deref()?))
}

/// The `robots.txt` for this deck. Always something: none of it needs an origin.
pub fn render_robots(deck: &Deck, options: &SeoOptions) -> String {
    robots::render(deck, &options.deck_path, options.deck_url.as_deref(), options.presenter)
}

/// What this page is called, in the deck's own words.
///
/// The slide's heading, else the deck's title for a slide that has none, else
/// its number. Deliberately *not* the combined `Intro — My Talk` of the
/// `<title>`: a preview shows the deck's name separately, and a headline
/// carrying it twice wastes the only line anybody reads.
fn page_name(deck: &Deck, slide: &Slide) -> String {
    slide
        .title
        .clone()
        .or_else(|| deck.meta.title.clone())
        .unwrap_or_else(|| slide.display_title())
}

/// Where this slide's page lives, absolutely.
fn page_url(deck_url: &str, slide: &Slide) -> String {
    resolve(deck_url, &slide_path(slide.index))
}

/// This slide's social card, absolute where possible and relative otherwise.
///
/// Points at the PNG. The SVG is what the build always writes and almost no
/// scraper renders it, so the file worth naming is the one the rasteriser
/// produces from it — and a build with no browser to rasterise with already says
/// so, in a warning that names the command to fix it.
fn card_url(slide: &Slide, options: &SeoOptions) -> Option<String> {
    if !options.cards {
        return None;
    }

    let file = format!("og-{}.png", slide.index + 1);

    Some(match options.deck_url.as_deref() {
        Some(deck_url) => resolve(deck_url, &file),
        None => format!("{}{file}", up_to_root(slide.index)),
    })
}

fn sequence(rel: &str, href: &str) -> String {
    format!("<link rel=\"{rel}\" href=\"{}\">", escape(href))
}

fn meta(name: &str, content: &str) -> String {
    format!("<meta name=\"{name}\" content=\"{}\">", escape(content))
}

fn property(property: &str, content: &str) -> String {
    format!("<meta property=\"{property}\" content=\"{}\">", escape(content))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    const PUBLISHED: &str = "---\ntitle: Fast Decks\ndraft: false\nurl: https://example.com/talk/\n---\n\n";

    fn published(source: &str) -> SeoOptions {
        let deck = parse_deck(source, &DeckParseOptions::default());

        SeoOptions {
            deck_url: deck.meta.talk.url.clone(),
            deck_path: "/slides/".to_string(),
            cards: true,
            presenter: true,
        }
    }

    fn head_of(source: &str, index: usize) -> String {
        let deck = parse_deck(source, &DeckParseOptions::default());
        head(&deck, &deck.slides[index], &published(source))
    }

    #[test]
    fn a_page_says_which_url_is_the_real_one() {
        // A deck deployed at a bare host and a www one otherwise has two of
        // every page and no statement about which is which.
        let html = head_of(&format!("{PUBLISHED}# One\n\n---\n\n# Two\n"), 1);

        assert!(html.contains("<link rel=\"canonical\" href=\"https://example.com/talk/2/\">"));
        assert!(html.contains("<meta property=\"og:url\" content=\"https://example.com/talk/2/\">"));
    }

    #[test]
    fn a_deck_nobody_has_given_an_address_claims_no_canonical_at_all() {
        // A guessed origin is not a worse version of the right answer. It is a
        // statement that this page belongs somewhere it does not.
        let deck = parse_deck("# One\n", &DeckParseOptions::default());
        let html = head(&deck, &deck.slides[0], &SeoOptions { cards: true, ..SeoOptions::default() });

        assert!(!html.contains("canonical"), "{html}");
        assert!(!html.contains("og:url"), "{html}");

        // Nothing anywhere in the head claims an address. The one `https://` a
        // page like this still carries is the JSON-LD `@context`, which names a
        // vocabulary rather than a page of this deck.
        assert!(!html.contains("href=\"http"), "an address was invented:\n{html}");
        assert!(!html.contains("content=\"http"), "an address was invented:\n{html}");
    }

    #[test]
    fn a_deck_with_no_address_still_points_at_its_own_card() {
        // Relative, and correct: the card is beside the deck root, and a page
        // one directory down has to climb back to it.
        let deck = parse_deck("# One\n\n---\n\n# Two\n", &DeckParseOptions::default());
        let options = SeoOptions { cards: true, ..SeoOptions::default() };

        assert!(head(&deck, &deck.slides[0], &options).contains("content=\"og-1.png\""));
        assert!(head(&deck, &deck.slides[1], &options).contains("content=\"../og-2.png\""));
    }

    #[test]
    fn each_slide_points_at_the_card_drawn_for_that_slide() {
        let html = head_of(&format!("{PUBLISHED}# One\n\n---\n\n# Two\n"), 1);

        assert!(html.contains("content=\"https://example.com/talk/og-2.png\""));
        assert!(html.contains(&format!("content=\"{OG_WIDTH}\"")));
        assert!(html.contains(&format!("content=\"{OG_HEIGHT}\"")));
        assert!(html.contains("twitter:card"));
    }

    #[test]
    fn a_build_that_drew_no_cards_points_at_none() {
        let deck = parse_deck("# One\n", &DeckParseOptions::default());
        let html = head(&deck, &deck.slides[0], &SeoOptions::default());

        assert!(!html.contains("og:image"), "{html}");
        assert!(!html.contains("twitter:card"), "{html}");
    }

    #[test]
    fn the_slides_are_linked_as_the_sequence_they_are() {
        let source = format!("{PUBLISHED}# One\n\n---\n\n# Two\n\n---\n\n# Three\n");

        let first = head_of(&source, 0);
        let middle = head_of(&source, 1);
        let last = head_of(&source, 2);

        assert!(!first.contains("rel=\"prev\""), "nothing precedes slide one:\n{first}");
        assert!(first.contains("<link rel=\"next\" href=\"2/\">"));

        assert!(middle.contains("<link rel=\"prev\" href=\"../\">"));
        assert!(middle.contains("<link rel=\"next\" href=\"../3/\">"));

        assert!(last.contains("<link rel=\"prev\" href=\"../2/\">"));
        assert!(!last.contains("rel=\"next\""), "nothing follows the last slide:\n{last}");
    }

    #[test]
    fn a_title_and_a_description_differ_from_slide_to_slide() {
        // The failure this exists to prevent: forty pages whose preview and
        // whose search result are the same two lines about the deck.
        let source = format!(
            "{PUBLISHED}# Opening\n\nWhy decks should be pages.\n\n---\n\n# Results\n\nThe rewrite paid for itself.\n"
        );

        let first = head_of(&source, 0);
        let second = head_of(&source, 1);

        assert!(first.contains("content=\"Opening\""));
        assert!(first.contains("content=\"Why decks should be pages.\""));
        assert!(second.contains("content=\"Results\""));
        assert!(second.contains("content=\"The rewrite paid for itself.\""));
    }

    #[test]
    fn the_decks_name_is_carried_beside_a_slides_own() {
        let html = head_of(&format!("{PUBLISHED}# Opening\n"), 0);

        assert!(html.contains("<meta property=\"og:site_name\" content=\"Fast Decks\">"));
    }

    #[test]
    fn a_slide_that_shares_the_decks_title_does_not_say_it_twice() {
        let html = head_of(&format!("{PUBLISHED}# Fast Decks\n"), 0);

        assert!(!html.contains("og:site_name"), "{html}");
    }

    #[test]
    fn a_draft_deck_asks_every_page_not_to_be_indexed() {
        let source = "---\nurl: https://example.com/talk/\n---\n\n# One\n\n---\n\n# Two\n";
        let deck = parse_deck(source, &DeckParseOptions::default());
        let options = published(source);

        for slide in &deck.slides {
            let html = head(&deck, slide, &options);
            assert!(html.contains(NOINDEX), "slide {} is indexable:\n{html}", slide.index);
        }
    }

    #[test]
    fn a_published_deck_asks_for_nothing_of_the_kind() {
        assert!(!head_of(&format!("{PUBLISHED}# One\n"), 0).contains("noindex"));
    }

    #[test]
    fn the_structured_data_travels_in_the_document_rather_than_being_fetched() {
        let html = head_of(&format!("{PUBLISHED}# One\n"), 0);

        assert!(html.contains("<script type=\"application/ld+json\">"));
        assert!(html.contains("PresentationDigitalDocument"));
    }

    #[test]
    fn structured_data_carries_no_relative_card_because_it_is_read_out_of_context() {
        // A page's own `og:image` is resolved by whoever loaded the page. A
        // JSON-LD block is copied into indexes and feeds, where there is no
        // page left to resolve against.
        let deck = parse_deck("# One\n", &DeckParseOptions::default());
        let html = head(&deck, &deck.slides[0], &SeoOptions { cards: true, ..SeoOptions::default() });

        assert!(html.contains("content=\"og-1.png\""), "the page still points at it:\n{html}");
        assert!(!html.contains("thumbnailUrl"), "{html}");
    }

    #[test]
    fn a_quote_in_a_description_cannot_end_the_attribute_it_is_in() {
        let source = format!("{PUBLISHED}# One\n\nThe \"fast\" path & why.\n");
        let html = head_of(&source, 0);

        assert!(html.contains("&quot;fast&quot; path &amp; why."), "{html}");
    }

    #[test]
    fn a_sitemap_exists_only_for_a_deck_with_an_address() {
        let deck = parse_deck(&format!("{PUBLISHED}# One\n"), &DeckParseOptions::default());

        assert!(render_sitemap(&deck, &published(PUBLISHED)).is_some());
        assert!(render_sitemap(&deck, &SeoOptions::default()).is_none());
    }

    #[test]
    fn a_robots_file_exists_whether_or_not_the_deck_has_an_address() {
        let deck = parse_deck(&format!("{PUBLISHED}# One\n"), &DeckParseOptions::default());

        assert!(render_robots(&deck, &published(PUBLISHED)).contains("User-agent: *"));
        assert!(render_robots(&deck, &SeoOptions::default()).contains("User-agent: *"));
    }
}
