//! Every slide at once, as one page of links.
//!
//! A deck is a sequence and a long one is hard to hold in your head. The
//! question "where is the slide about the linter" has no answer in a format
//! whose only navigation is next and previous, and the roadmap has promised an
//! overview key since M4 against a runtime nothing on a slide imports.
//!
//! # Why it needs no script
//!
//! Because the slide is already a size container. Every length inside one is a
//! share of *the slide* rather than of the window — that is what makes a deck
//! scale as one piece to any projector — so putting a slide in a small box is
//! the whole of drawing a thumbnail. No transform, no second stylesheet for
//! small sizes, and no rendering a slide a second way.
//!
//! What is left is a grid of links between real documents, which is what the
//! rest of the deck already is.
//!
//! # Why the frames are inert
//!
//! It shares [`crate::shell::render_static_preview`]'s frame writer, so a live
//! demo does not start twelve iframes, a camera tile does not ask twelve times
//! for a webcam, and a staged slide shows the markup rather than its first
//! stop. An overview is for finding a slide, not for running one.

use slidx_core::Deck;

use crate::shell::ShellOptions;
use crate::{layout, region, seo, url};

/// The overview's own stylesheet.
///
/// Only what the grid needs. Everything inside a slide is drawn by the shell
/// stylesheet the slides already carry, because they are the same slides.
const STYLESHEET: &str = r#"
/*
 * The page around the slides is not a slide.
 *
 * Everything a slide is set with is a share of the slide — the measure, the
 * type scale, the spacing — and this page carries the same stylesheet because
 * the thumbnails are real slides. So its own chrome has to opt out explicitly,
 * in absolute units, or it inherits numbers that mean nothing outside a
 * container.
 *
 * Found by measuring: the grid was one column wide because `ol` picks up the
 * prose measure, and thirty em at the root size is 480px.
 */
.slidx-overview {
  display: grid;
  /*
   * Wide enough to recognise a slide by.
   *
   * A thumbnail is not for reading — 1920 design pixels in a 300px box puts
   * body text at three — it is for telling one slide from another by the shape
   * of its heading and where its blocks sit. Below about 320 even that goes,
   * and the grid stops answering the question it exists for.
   */
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 320px), 1fr));
  gap: 1.25rem;
  align-content: start;
  padding: 1.25rem;
  margin: 0;
  max-width: none;
  list-style: none;
}

.slidx-overview-heading {
  max-width: none;
  margin: 0;
  padding: 1.25rem 1.25rem 0;
  font-size: 1.5rem;
  line-height: 1.2;
  letter-spacing: normal;
}

/*
 * The one override the grid needs.
 *
 * A slide sizes itself to the viewport so it fills a projector. Here it fills
 * its cell instead, and everything inside follows without being told: the
 * slide is a size container, so `cqh` resolves against the cell.
 */
.slidx-overview .slidx-slide {
  width: 100%;
  height: auto;
}

.slidx-overview .slidx-deck {
  display: block;
  min-height: 0;
  padding: 0;
}

/*
 * The link is the whole thumbnail, and it says which slide it is.
 *
 * `aria-label` on the anchor rather than a caption under it: a caption would
 * be read after twelve nested headings, and the heading inside a thumbnail is
 * the slide's, not the link's.
 */
.slidx-overview-slide {
  display: block;
  text-decoration: none;
  color: inherit;
  outline-offset: 3px;
}

.slidx-overview-slide:focus-visible {
  outline: 2px solid var(--slidx-color-accent);
}

@media (hover: hover) {
  .slidx-overview-slide:hover .slidx-slide { outline: 2px solid var(--slidx-color-accent); }
}

.slidx-overview-number {
  display: block;
  padding-top: 0.4em;
  color: var(--slidx-color-muted);
  font-size: 0.8rem;
  font-variant-numeric: tabular-nums;
}
"#;

/// Renders the whole deck as one page of thumbnails.
pub fn render_overview(deck: &Deck, options: &ShellOptions) -> String {
    let slides: String = deck
        .slides
        .iter()
        .map(|slide| {
            let slide_layout = region::layout_of(slide);
            let frame = crate::shell::slide_frame(deck, slide, options, &slide_layout, "", "", "");

            // Relative, like every other address in a deck, so an overview
            // works from a USB stick and out of a directory somebody moved.
            let href = format!("../{}", url::slide_path(slide.index));
            let number = slide.index + 1;
            let label = crate::shell::escape(&match &slide.title {
                Some(title) => format!("Slide {number}: {title}"),
                None => format!("Slide {number}"),
            });

            format!(
                "<li><a class=\"slidx-overview-slide\" href=\"{href}\" \
                 aria-label=\"{label}\">{frame}\
                 <span class=\"slidx-overview-number\" aria-hidden=\"true\">{number}</span>\
                 </a></li>\n",
            )
        })
        .collect();

    format!(
        r#"<!doctype html>
<html lang="{lang}" data-slidx-aspect="{aspect}" data-slidx-overview>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
{noindex}
<style>
{theme_css}
{shell_css}
{layout_css}
{overview_css}
</style>
</head>
<body>
<main>
<h1 class="slidx-overview-heading">{title}</h1>
<ol class="slidx-overview">
{slides}</ol>
</main>
</body>
</html>
"#,
        lang = crate::shell::escape(deck.language()),
        aspect = deck.meta.aspect.as_token(),
        title = crate::shell::escape(deck.meta.title.as_deref().unwrap_or("Overview")),
        // An overview is a way in, not a page to rank: it duplicates every
        // slide's words with none of their context.
        noindex = seo::NOINDEX,
        theme_css = options.theme_css(),
        shell_css = layout::STYLESHEET,
        layout_css = slidx_theme::layout::stylesheet(),
        overview_css = STYLESHEET,
        slides = slides,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn deck_of(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    fn overview(source: &str) -> String {
        render_overview(&deck_of(source), &ShellOptions::default())
    }

    #[test]
    fn every_slide_is_there_and_every_one_is_a_link() {
        let html = overview("# One\n\n---\n\n# Two\n\n---\n\n# Three\n");

        assert_eq!(html.matches(r#"<a class="slidx-overview-slide""#).count(), 3);
        assert!(html.contains(r#"href="../""#), "slide one is the deck root");
        assert!(html.contains(r#"href="../2/""#));
        assert!(html.contains(r#"href="../3/""#));
    }

    #[test]
    fn it_runs_nothing() {
        // The whole point of building it out of the slide's own container
        // queries: a thumbnail needs no script, so an overview is a page of
        // links between real documents like the rest of the deck.
        let html = overview("# One\n\n---\n\n# Two\n");

        assert!(!html.contains("<script>"), "got:\n{html}");
        assert!(!html.contains("<script type=\"module\">"));
    }

    #[test]
    fn a_thumbnail_is_named_by_its_slide() {
        // The link is the thumbnail, so the link carries the name. A caption
        // would be read after everything inside the slide.
        let html = overview("# The linter checks the room\n\n---\n\n(no heading)\n");

        assert!(html.contains(r#"aria-label="Slide 1: The linter checks the room""#));
        assert!(html.contains(r#"aria-label="Slide 2""#), "a slide with no title says its number");
    }

    #[test]
    fn nothing_venue_only_starts_twelve_times_over() {
        // It shares the static preview's frame writer, so a live demo does not
        // open twelve iframes and a camera tile does not ask twelve times for
        // a webcam.
        let html = overview("---\nlayout: aside\ncamera: side\n---\n\n# Remote\n\n---\n\n# Two\n");

        assert!(!html.contains("getUserMedia"));
        assert!(!html.contains("<video"));
        assert!(!html.contains("<iframe"));
    }

    #[test]
    fn it_is_not_offered_to_a_crawler() {
        // Every slide's words with none of their context. A way in, not a page
        // to rank.
        assert!(overview("# One\n").contains("noindex"));
    }

    #[test]
    fn the_page_around_the_slides_is_not_set_like_a_slide() {
        // The grid was one column wide because `ol` picks up the prose measure
        // and thirty em at the root size is 480px. Everything a slide is set
        // with is a share of the slide, so this page's own chrome opts out.
        let html = overview("# One\n");

        assert!(html.contains(".slidx-overview {"));
        assert!(html.contains("max-width: none;"));
    }

    #[test]
    fn a_deck_of_one_slide_still_renders() {
        let html = overview("# Only\n");

        assert_eq!(html.matches(r#"<a class="slidx-overview-slide""#).count(), 1);
        assert!(html.contains(r#"href="../""#));
    }
}
