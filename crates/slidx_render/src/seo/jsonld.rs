//! The deck, described to a machine.
//!
//! Everything here is already in the frontmatter an author wrote at proposal
//! time — the title, the event, the date, the venue, the speaker. This says it
//! again in the one vocabulary a crawler, an archive, or a bookmarking tool can
//! read without guessing, so none of them has to scrape a heading out of the
//! markup and hope.
//!
//! # Why a `<script>` in a deck that loads no module
//!
//! `application/ld+json` is not a script type any browser executes. The element
//! is a container the specification chose for a block of JSON; nothing parses
//! it as code, nothing runs, and it costs no request. The claim slidx makes is
//! about code that runs on a slide, and this is data sitting in the head.
//!
//! # What is deliberately not claimed
//!
//! No `datePublished`. The date in a deck's frontmatter is the day the *talk*
//! is given, which is routinely in the future, and publishing a future date as
//! the day the document was published would be a statement slidx cannot make on
//! the author's behalf. It goes on the event, as the event's start date, which
//! is what it is.
//!
//! `recordedAt` is the closest tie the vocabulary has between a work and the
//! occasion it exists for. `about` would be wrong — a conference is not the
//! subject matter of a talk given at it.

use serde_json::{Map, Value};
use slidx_core::{Deck, Slide};

/// What a deck of slides is, in schema.org's terms.
const DOCUMENT: &str = "PresentationDigitalDocument";

/// Where a slide page names its own facts.
#[derive(Debug, Clone, Default)]
pub struct Facts<'a> {
    /// Absolute URL of this page, when anyone has said where the deck lives.
    pub url: Option<&'a str>,
    /// Absolute URL of the deck root, for the `isPartOf` back-reference.
    pub deck_url: Option<&'a str>,
    /// Absolute URL of this slide's social card.
    pub card: Option<&'a str>,
    pub description: Option<&'a str>,
    /// What the page's `<title>` says, so the two cannot disagree.
    pub title: &'a str,
}

/// The structured-data block for one slide's page.
pub fn document(deck: &Deck, slide: &Slide, facts: &Facts<'_>) -> String {
    let mut node = Map::new();
    node.insert("@context".into(), "https://schema.org".into());
    node.insert("@type".into(), DOCUMENT.into());
    node.insert("name".into(), facts.title.into());

    insert(&mut node, "description", facts.description);
    insert(&mut node, "url", facts.url);
    insert(&mut node, "thumbnailUrl", facts.card);

    // One-based, because it is the number on the slide's own footer and the
    // number a person says out loud.
    node.insert("position".into(), (slide.index + 1).into());

    if let Some(author) = &deck.meta.author {
        node.insert("author".into(), person(author));
    }

    // Slide one *is* the deck's front page, so a part-of pointing at itself
    // would say nothing. Every other slide is a page inside something larger,
    // and that relation is the only thing telling a crawler these forty URLs
    // are one talk rather than forty.
    if slide.index > 0 {
        if let Some(deck_node) = part_of(deck, facts.deck_url) {
            node.insert("isPartOf".into(), deck_node);
        }
    }

    if let Some(event) = event(deck) {
        node.insert("recordedAt".into(), event);
    }

    let json = Value::Object(node).to_string();

    format!("<script type=\"application/ld+json\">{}</script>", escape_json(&json))
}

fn person(name: &str) -> Value {
    let mut node = Map::new();
    node.insert("@type".into(), "Person".into());
    node.insert("name".into(), name.into());

    Value::Object(node)
}

/// The deck this slide belongs to, when there is anything to say about it.
fn part_of(deck: &Deck, deck_url: Option<&str>) -> Option<Value> {
    let title = deck.meta.title.as_deref();
    if title.is_none() && deck_url.is_none() {
        return None;
    }

    let mut node = Map::new();
    node.insert("@type".into(), DOCUMENT.into());
    insert(&mut node, "name", title);
    insert(&mut node, "url", deck_url);

    Some(Value::Object(node))
}

/// The occasion the deck exists for.
fn event(deck: &Deck) -> Option<Value> {
    let talk = &deck.meta.talk;
    let name = talk.event.as_deref()?;

    let mut node = Map::new();
    node.insert("@type".into(), "Event".into());
    node.insert("name".into(), name.into());
    insert(&mut node, "startDate", talk.date.as_deref());

    if let Some(venue) = &talk.venue {
        let mut place = Map::new();
        place.insert("@type".into(), "Place".into());
        place.insert("name".into(), venue.as_str().into());
        node.insert("location".into(), Value::Object(place));
    }

    Some(Value::Object(node))
}

fn insert(node: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        node.insert(key.to_string(), value.into());
    }
}

/// Escapes `<` so a deck can quote HTML in its own title.
///
/// The bytes inside a `<script>` are not HTML-escaped — a `&amp;` here would
/// reach a parser as those five characters — but a `</script` anywhere in the
/// JSON ends the element early and spills the rest of the deck's metadata into
/// the page as text. `<` is the JSON spelling of the same character and
/// the only one that closes that hole.
fn escape_json(json: &str) -> String {
    json.replace('<', "\\u003c")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn block(source: &str, index: usize) -> Value {
        let deck = parse_deck(source, &DeckParseOptions::default());
        let slide = &deck.slides[index];
        let facts = Facts {
            url: Some("https://example.com/talk/"),
            deck_url: Some("https://example.com/talk/"),
            title: "One",
            ..Facts::default()
        };

        parse(&document(&deck, slide, &facts))
    }

    /// The JSON inside the element, which is the thing a consumer reads.
    fn parse(element: &str) -> Value {
        let inner = element
            .trim_start_matches("<script type=\"application/ld+json\">")
            .trim_end_matches("</script>");

        serde_json::from_str(inner).expect("valid JSON-LD")
    }

    #[test]
    fn a_slide_page_declares_itself_a_presentation_document() {
        let node = block("# One\n", 0);

        assert_eq!(node["@context"], json!("https://schema.org"));
        assert_eq!(node["@type"], json!("PresentationDigitalDocument"));
        assert_eq!(node["name"], json!("One"));
        assert_eq!(node["url"], json!("https://example.com/talk/"));
    }

    #[test]
    fn the_talks_event_venue_and_date_come_from_the_frontmatter_unchanged() {
        let source = "---\nevent: SlidxConf 2026\ndate: 2026-05-14\nvenue: Tokyo\n---\n\n# One\n";
        let node = block(source, 0);

        assert_eq!(node["recordedAt"]["@type"], json!("Event"));
        assert_eq!(node["recordedAt"]["name"], json!("SlidxConf 2026"));
        assert_eq!(node["recordedAt"]["startDate"], json!("2026-05-14"));
        assert_eq!(node["recordedAt"]["location"]["name"], json!("Tokyo"));
    }

    #[test]
    fn the_date_of_a_talk_is_never_published_as_the_date_of_the_document() {
        // A conference deck's date is usually in the future, and slidx cannot
        // claim on an author's behalf that a document was published then.
        let node = block("---\ndate: 2026-05-14\n---\n\n# One\n", 0);

        assert!(node.get("datePublished").is_none(), "{node}");
    }

    #[test]
    fn a_slide_after_the_first_says_which_talk_it_is_part_of() {
        // Without it, a forty-slide deck is forty unrelated URLs.
        let node = block("---\ntitle: Fast Decks\n---\n\n# One\n\n---\n\n# Two\n", 1);

        assert_eq!(node["isPartOf"]["name"], json!("Fast Decks"));
        assert_eq!(node["isPartOf"]["url"], json!("https://example.com/talk/"));
        assert_eq!(node["position"], json!(2));
    }

    #[test]
    fn the_first_slide_is_the_deck_rather_than_a_part_of_it() {
        let node = block("---\ntitle: Fast Decks\n---\n\n# One\n", 0);

        assert!(node.get("isPartOf").is_none(), "{node}");
        assert_eq!(node["position"], json!(1));
    }

    #[test]
    fn a_deck_that_named_nobody_and_nowhere_still_produces_valid_data() {
        // Frontmatter is optional in this project, so every field here has to
        // be, and the result still has to parse.
        let deck = parse_deck("# One\n", &DeckParseOptions::default());
        let node =
            parse(&document(&deck, &deck.slides[0], &Facts { title: "One", ..Facts::default() }));

        assert_eq!(node["name"], json!("One"));
        assert!(node.get("url").is_none());
        assert!(node.get("author").is_none());
        assert!(node.get("recordedAt").is_none());
    }

    #[test]
    fn a_title_containing_a_closing_script_tag_cannot_end_the_element() {
        // Otherwise the rest of the deck's metadata spills into the page as
        // text, and the document a crawler reads is not the one that was meant.
        let deck = parse_deck("# One\n", &DeckParseOptions::default());
        let facts = Facts { title: "</script><img onerror=alert(1)>", ..Facts::default() };
        let element = document(&deck, &deck.slides[0], &facts);

        assert!(!element.contains("</script><img"), "{element}");
        assert_eq!(element.matches("</script>").count(), 1);
        assert_eq!(parse(&element)["name"], json!("</script><img onerror=alert(1)>"));
    }

    #[test]
    fn the_speaker_and_the_card_are_carried_when_the_deck_has_them() {
        let deck =
            parse_deck("---\nauthor: ubugeeei\n---\n\n# One\n", &DeckParseOptions::default());
        let facts = Facts {
            title: "One",
            card: Some("https://example.com/talk/og-1.png"),
            description: Some("A framework."),
            ..Facts::default()
        };
        let node = parse(&document(&deck, &deck.slides[0], &facts));

        assert_eq!(node["author"]["@type"], json!("Person"));
        assert_eq!(node["author"]["name"], json!("ubugeeei"));
        assert_eq!(node["thumbnailUrl"], json!("https://example.com/talk/og-1.png"));
        assert_eq!(node["description"], json!("A framework."));
    }
}
