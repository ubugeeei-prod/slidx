//! The picture a link to a slide turns into.
//!
//! A deck is shared as a URL far more often than it is presented, and a URL
//! with no card is a grey rectangle in a timeline. Every slide has its own
//! address, so every slide gets its own card — the one someone links to shows
//! *that* idea rather than the title slide.
//!
//! # Why SVG rather than a rasteriser
//!
//! Rendering text to pixels needs a font stack, a shaper, and a layout engine:
//! several megabytes of dependency, and a second implementation of type
//! layout that would drift from the one the slides use. An SVG says the same
//! thing in a few kilobytes, is produced by the same theme tokens, and is
//! converted to PNG once at build time by the browser that is already there
//! for the PDF.
//!
//! The card is deliberately *not* a screenshot of the slide. A slide is
//! designed to be read from twelve metres; a card is read at 400 pixels wide
//! in a crowded feed, so it carries fewer words, larger.

use slidx_core::mark::strip_marks;
use slidx_core::{Deck, Slide};
use slidx_theme::{Scheme, Theme};

/// Facebook's and X's shared preference, and what every scraper crops to.
pub const OG_WIDTH: u32 = 1200;
pub const OG_HEIGHT: u32 = 630;

/// How to draw a card.
#[derive(Debug, Clone)]
pub struct OgOptions {
    pub theme: Theme,
    /// Shown small, under the title. The event, or the deck.
    pub eyebrow: Option<String>,
    /// Shown at the foot. The speaker, or the hashtag.
    pub footer: Option<String>,
}

impl Default for OgOptions {
    fn default() -> Self {
        Self { theme: slidx_theme::default_theme(), eyebrow: None, footer: None }
    }
}

/// Draws the card for one slide.
pub fn render_slide_card(deck: &Deck, slide: &Slide, options: &OgOptions) -> String {
    let title = slide
        .title
        .clone()
        .unwrap_or_else(|| deck.meta.title.clone().unwrap_or_else(|| "slidx".to_string()));

    card(&title, options, deck)
}

/// Draws the card for the deck as a whole.
pub fn render_deck_card(deck: &Deck, options: &OgOptions) -> String {
    let title = deck.meta.title.clone().unwrap_or_else(|| "slidx".to_string());
    card(&title, options, deck)
}

fn card(title: &str, options: &OgOptions, deck: &Deck) -> String {
    // The light scheme, always. A card is shown on someone else's page,
    // whose background slidx does not control — a dark card on a light feed
    // reads as a rendering fault rather than as a choice.
    let palette = options.theme.palette(Scheme::Light);
    let eyebrow = options.eyebrow.clone().or_else(|| deck.meta.talk.event.clone());
    let footer = options.footer.clone().or_else(|| footer_for(deck));

    let lines = wrap(&strip_marks(title), 22, 3);
    let size = title_size(&lines);

    let mut body = String::new();
    for (index, line) in lines.iter().enumerate() {
        let y = 300 + (index as i32 - (lines.len() as i32 - 1)) * (size as i32 * 6 / 5) / 2;
        body.push_str(&format!(
            "\n  <text x=\"80\" y=\"{y}\" class=\"t\" font-size=\"{size}\">{}</text>",
            escape(line)
        ));
    }

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{OG_WIDTH}" height="{OG_HEIGHT}" viewBox="0 0 {OG_WIDTH} {OG_HEIGHT}" role="img" aria-label="{alt}">
  <style>
    .t {{ fill: {text}; font-family: {font}; font-weight: 650; letter-spacing: -0.02em; }}
    .m {{ fill: {muted}; font-family: {font}; font-weight: 500; font-size: 26px; letter-spacing: 0.04em; }}
  </style>
  <rect width="{OG_WIDTH}" height="{OG_HEIGHT}" fill="{surface}"/>
  <rect x="0" y="0" width="10" height="{OG_HEIGHT}" fill="{accent}"/>{eyebrow_text}{body}{footer_text}
</svg>
"##,
        alt = escape(title),
        text = palette.text.to_hex(),
        muted = palette.muted.to_hex(),
        surface = palette.surface.to_hex(),
        accent = palette.accent.to_hex(),
        font = escape(&options.theme.font_sans),
        eyebrow_text = eyebrow
            .map(|text| format!(
                "\n  <text x=\"80\" y=\"110\" class=\"m\">{}</text>",
                escape(&text.to_uppercase())
            ))
            .unwrap_or_default(),
        body = body,
        footer_text = footer
            .map(|text| format!(
                "\n  <text x=\"80\" y=\"550\" class=\"m\">{}</text>",
                escape(&text)
            ))
            .unwrap_or_default(),
    )
}

fn footer_for(deck: &Deck) -> Option<String> {
    deck.meta
        .talk
        .hashtag
        .as_ref()
        .map(|tag| format!("#{tag}"))
        .or_else(|| deck.meta.author.clone())
}

/// Title size, chosen so a long title still fits the box.
///
/// Three lines at the largest size would overflow 630 pixels, so the size
/// steps down as the line count grows. A card that clips its own title is
/// worse than one set slightly smaller.
fn title_size(lines: &[String]) -> u32 {
    match lines.len() {
        0 | 1 => 84,
        2 => 72,
        _ => 60,
    }
}

/// Greedy word wrap, capped at `max_lines`.
///
/// Anything past the cap is dropped and the last line gains an ellipsis: a
/// card has room for a headline, not for a paragraph, and silently running
/// off the edge is the one outcome nobody notices until it is public.
fn wrap(text: &str, columns: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let candidate =
            if current.is_empty() { word.to_string() } else { format!("{current} {word}") };

        if candidate.chars().count() > columns && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            if lines.len() == max_lines {
                break;
            }
            current = word.to_string();
        } else {
            current = candidate;
        }
    }

    if lines.len() < max_lines && !current.is_empty() {
        lines.push(current);
    } else if lines.len() == max_lines {
        if let Some(last) = lines.last_mut() {
            last.push('…');
        }
    }

    lines
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn deck(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    fn card_for(source: &str) -> String {
        let deck = deck(source);
        render_slide_card(&deck, &deck.slides[0], &OgOptions::default())
    }

    #[test]
    fn a_card_is_the_size_every_scraper_crops_to() {
        let svg = card_for("# One\n");

        assert!(svg.contains(r#"width="1200""#));
        assert!(svg.contains(r#"height="630""#));
        assert!(svg.contains(r#"viewBox="0 0 1200 630""#));
    }

    #[test]
    fn every_slide_gets_its_own_card() {
        // A shared link should show the idea it points at, not the deck's
        // title slide.
        let deck = deck("# One\n\n---\n\n# Two\n");
        let first = render_slide_card(&deck, &deck.slides[0], &OgOptions::default());
        let second = render_slide_card(&deck, &deck.slides[1], &OgOptions::default());

        assert!(first.contains("One"));
        assert!(second.contains("Two"));
        assert_ne!(first, second);
    }

    #[test]
    fn a_card_for_a_slide_with_a_camera_on_it_has_no_hole_in_it() {
        // A social card is rendered months before the talk and looked at long
        // after it, by people who will never see the speaker's face on the
        // slide. A reserved rectangle would be an empty box in every preview.
        let svg = card_for("---\ntitle: Remote\nlayout: aside\ncamera: side\n---\n\n# Remote\n");

        assert!(!svg.contains("camera"), "the camera reached the card:\n{svg}");
    }

    #[test]
    fn a_slide_with_no_title_falls_back_to_the_decks() {
        let deck = deck("---\ntitle: My Talk\n---\n\nJust prose.\n");
        let svg = render_slide_card(&deck, &deck.slides[0], &OgOptions::default());

        assert!(svg.contains("My Talk"));
    }

    #[test]
    fn nothing_in_a_card_is_fetched() {
        // A scraper takes the image and nothing else, so a reference inside it
        // renders as a hole in someone else's timeline.
        //
        // `xmlns` is exempt and only that: it is an identifier the SVG spec
        // requires, never dereferenced by anything. Excluding it by name
        // rather than loosening the check keeps the next stray URL caught.
        let svg = card_for("# One\n").replace(r#"xmlns="http://www.w3.org/2000/svg""#, "");

        for marker in ["http://", "https://", "<image", "xlink:href", "@import"] {
            assert!(!svg.contains(marker), "card reaches for {marker}");
        }
    }

    #[test]
    fn a_long_title_wraps_rather_than_running_off_the_edge() {
        let svg = card_for("# A considerably longer title than fits on one single line\n");
        assert!(svg.matches("<text").count() >= 3);
    }

    #[test]
    fn a_title_longer_than_the_card_is_truncated_visibly() {
        // Running off the edge is the one outcome nobody notices until it is
        // public, so the cut is marked.
        let svg = card_for(
            "# One two three four five six seven eight nine ten eleven twelve thirteen fourteen\n",
        );

        assert!(svg.contains('…'));
    }

    #[test]
    fn the_type_gets_smaller_as_the_title_gets_longer() {
        // Three lines at the one-line size would overflow the box.
        let short = card_for("# Short\n");
        let long = card_for("# A considerably longer title than fits on one single line at all\n");

        assert!(short.contains("font-size=\"84\""));
        assert!(long.contains("font-size=\"60\""));
    }

    #[test]
    fn the_event_becomes_the_eyebrow() {
        let deck = deck("---\nevent: SlidxConf\n---\n\n# One\n");
        let svg = render_slide_card(&deck, &deck.slides[0], &OgOptions::default());

        assert!(svg.contains("SLIDXCONF"));
    }

    #[test]
    fn the_hashtag_becomes_the_footer() {
        let deck = deck("---\nhashtag: slidx\n---\n\n# One\n");
        let svg = render_slide_card(&deck, &deck.slides[0], &OgOptions::default());

        assert!(svg.contains("#slidx"));
    }

    #[test]
    fn the_author_stands_in_when_there_is_no_hashtag() {
        let deck = deck("---\nauthor: ubugeeei\n---\n\n# One\n");
        let svg = render_slide_card(&deck, &deck.slides[0], &OgOptions::default());

        assert!(svg.contains("ubugeeei"));
    }

    #[test]
    fn a_card_uses_the_themes_colours() {
        let svg = card_for("# One\n");
        let theme = slidx_theme::default_theme();

        let palette = theme.palette(slidx_theme::Scheme::Light);
        assert!(svg.contains(&palette.surface.to_hex()));
        assert!(svg.contains(&palette.accent.to_hex()));
    }

    #[test]
    fn markup_in_a_title_is_escaped() {
        let deck = deck("---\ntitle: \"a <script> & b\"\n---\n\nprose\n");
        let svg = render_deck_card(&deck, &OgOptions::default());

        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&lt;script&gt;"));
        assert!(svg.contains("&amp;"));
    }

    #[test]
    fn a_marked_title_shows_its_words_rather_than_its_syntax() {
        // The card is read by people, not by the compiler.
        let deck = deck("# Making [decks]{.accent} fast\n");
        let svg = render_slide_card(&deck, &deck.slides[0], &OgOptions::default());

        assert!(svg.contains("Making decks fast"));
        assert!(!svg.contains("{.accent}"));
    }

    #[test]
    fn a_card_carries_alt_text() {
        // Cards are read by screen readers wherever a platform surfaces them.
        assert!(card_for("# One\n").contains(r#"aria-label="One""#));
    }

    #[test]
    fn cjk_titles_render() {
        let deck = deck("# 日本語のタイトル\n");
        let svg = render_slide_card(&deck, &deck.slides[0], &OgOptions::default());

        assert!(svg.contains("日本語のタイトル"));
    }

    #[test]
    fn the_same_deck_always_draws_the_same_card() {
        // Cards are content-addressed by the build; a card that differed run
        // to run would invalidate every cache for nothing.
        assert_eq!(card_for("# One\n"), card_for("# One\n"));
    }
}
