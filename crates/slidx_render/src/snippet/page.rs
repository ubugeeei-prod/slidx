//! The page a QR points at.
//!
//! Not a slide. A slide is a fixed frame designed to be read across a room at
//! a distance the author cannot control; this is read on a phone, held, by one
//! person, who is going to select the code and paste it somewhere. So it
//! borrows the theme's colours and its monospace face and none of its geometry:
//! the projector type scale is quoted against the height of a slide, and there
//! is no slide here.
//!
//! # No script
//!
//! Not even a copy button. Selection is a browser feature and every phone
//! already has it; a button would be the only JavaScript in the deck's output,
//! on the one page most likely to be opened over a hotel connection with two
//! bars. The `<pre>` scrolls horizontally rather than wrapping, because a
//! wrapped line in code is a line that has lost its indentation, and the
//! indentation is often the thing being shown.

use slidx_core::Deck;
use slidx_highlight::{to_html, Language};
use slidx_theme::{css, Theme};

use super::Snippet;

/// How to build a snippet page.
#[derive(Debug, Clone)]
pub struct SnippetOptions {
    pub theme: Theme,
}

impl Default for SnippetOptions {
    fn default() -> Self {
        Self { theme: slidx_theme::default_theme() }
    }
}

/// Renders one snippet as a complete HTML document.
pub fn render_snippet(deck: &Deck, snippet: &Snippet, options: &SnippetOptions) -> String {
    let language = snippet.language.as_deref().and_then(Language::parse);
    let heading = snippet.title.clone().unwrap_or_else(|| snippet.key.clone());

    let class = snippet
        .language
        .as_ref()
        .map(|name| format!(" class=\"language-{}\"", escape(name)))
        .unwrap_or_default();

    format!(
        r#"<!doctype html>
<html lang="en" data-slidx-snippet>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
{noindex}<style>
{theme_css}
{page_css}
</style>
</head>
<body>
<main class="slidx-snippet">
  <h1>{heading}</h1>
  <p class="slidx-snippet-source">{source}</p>
  <pre><code{class}>{code}</code></pre>
</main>
</body>
</html>
"#,
        title = escape(&title(deck, &heading)),
        noindex = crate::seo::noindex_line(deck),
        theme_css = css::render(&options.theme),
        page_css = STYLESHEET,
        heading = escape(&heading),
        source = escape(&source(deck, snippet)),
        class = class,
        code = to_html(&snippet.code, language),
    )
}

/// What the browser tab says.
fn title(deck: &Deck, heading: &str) -> String {
    match &deck.meta.title {
        Some(deck_title) if deck_title != heading => format!("{heading} — {deck_title}"),
        _ => heading.to_string(),
    }
}

/// Where this code came from, in one line.
///
/// A snippet page is shared onwards — pasted into chat, bookmarked, found again
/// six months later — and a page of code with no provenance is a page nobody
/// can trace back to the talk it explains.
fn source(deck: &Deck, snippet: &Snippet) -> String {
    let talk = deck.meta.title.clone().unwrap_or_else(|| "this deck".to_string());
    let slide = snippet.first_slide() + 1;

    match &deck.meta.author {
        Some(author) => format!("From {talk}, slide {slide} — {author}"),
        None => format!("From {talk}, slide {slide}"),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The snippet page's stylesheet.
///
/// Colours and faces come from the theme; every size is stated here in `rem`
/// and `ch`, because the theme's sizes are shares of a slide's height and a
/// document has no slide to be a share of.
pub const STYLESHEET: &str = r#"
*, *::before, *::after { box-sizing: border-box; }

html, body {
  margin: 0;
  background: var(--slidx-color-canvas);
  color: var(--slidx-color-text);
  font-family: var(--slidx-font-sans);
  -webkit-text-size-adjust: 100%;
}

/*
 * Held in one hand, at arm's length.
 *
 * The measure is set in `ch` against the mono face, so the column is as wide
 * as the code needs rather than as wide as prose wants.
 */
.slidx-snippet {
  max-width: 84ch;
  margin: 0 auto;
  padding: clamp(1rem, 4vw, 3rem) clamp(0.75rem, 4vw, 3rem);
  background: var(--slidx-color-surface);
  min-height: 100vh;
}

.slidx-snippet h1 {
  margin: 0 0 0.25rem;
  color: var(--slidx-color-heading);
  font-size: 1.5rem;
  line-height: 1.2;
  letter-spacing: -0.015em;
}

.slidx-snippet-source {
  margin: 0 0 1.5rem;
  color: var(--slidx-color-muted);
  font-size: 0.875rem;
}

/*
 * Scrolls rather than wraps.
 *
 * A wrapped line of code has lost its indentation, and the indentation is
 * often the thing the snippet exists to show.
 */
.slidx-snippet pre {
  margin: 0;
  padding: 1rem;
  overflow-x: auto;
  background: var(--slidx-color-code-surface);
  color: var(--slidx-color-code-text);
  border-radius: var(--slidx-radius);
  font-family: var(--slidx-font-mono);
  font-size: 0.9375rem;
  line-height: 1.55;
  tab-size: 2;
}

.slidx-snippet code { font-family: inherit; }

.slidx-code-comment { color: var(--slidx-color-code-comment); }
.slidx-code-string { color: var(--slidx-color-code-string); }
.slidx-code-number { color: var(--slidx-color-code-number); }
.slidx-code-keyword { color: var(--slidx-color-code-keyword); }
.slidx-code-type { color: var(--slidx-color-code-type); }
.slidx-code-punctuation { color: var(--slidx-color-code-punctuation); }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippet::{collect, render_snippets};
    use slidx_core::{parse_deck, DeckParseOptions};

    fn page(source: &str) -> String {
        let deck = parse_deck(source, &DeckParseOptions::default());
        render_snippet(&deck, &collect(&deck)[0], &SnippetOptions::default())
    }

    fn rust_page() -> String {
        page(concat!(
            "---\ntitle: Fast Decks\nauthor: ubugeeei\n---\n\n",
            "# Retrying\n\n```rust {#retry .share}\n",
            "/// Retries with a ceiling.\nasync fn retry(limit: u8) -> Result<(), Error> {\n",
            "    Ok(())\n}\n```\n",
        ))
    }

    #[test]
    fn the_page_carries_the_whole_snippet_rather_than_what_fitted_on_the_slide() {
        // Character for character, including the indentation — the page exists
        // so the code can be pasted, and pasted code that needs re-indenting is
        // a screenshot with extra steps.
        let source = "/// Retries with a ceiling.\nasync fn retry(limit: u8) -> Result<(), Error> {\n    Ok(())\n}\n";

        assert_eq!(code_text(&rust_page()), source);
    }

    #[test]
    fn the_code_arrives_highlighted_by_the_same_scanner_the_slide_used() {
        let html = rust_page();

        assert!(html.contains("<span class=\"slidx-code-keyword\">async</span>"));
        assert!(html.contains("class=\"slidx-code-comment\""));
        assert!(html.contains("--slidx-color-code-comment:"));
    }

    #[test]
    fn the_page_ships_no_script_at_all() {
        // Not even a copy button. Selection is a browser feature every phone
        // already has, and this is the page most likely to be opened over a
        // hotel connection with two bars.
        let html = rust_page();

        assert!(!html.contains("<script"));
        assert!(!html.contains("onclick"));
    }

    #[test]
    fn nothing_on_the_page_is_remote() {
        // The reason the snippet lives in the deck's own output in the first
        // place, applied to the page itself.
        let html = rust_page();

        for marker in ["http://", "https://", "//cdn", "@import url(", "<link"] {
            assert!(!html.contains(marker), "the page reaches for {marker}");
        }
    }

    #[test]
    fn the_code_is_selectable_text_and_not_an_image() {
        // The whole point of a page over a screenshot: it can be copied.
        let html = rust_page();

        assert!(html.contains("<pre><code class=\"language-rust\">"));
        assert!(!html.contains("user-select: none"));
        assert!(!STYLESHEET.contains("user-select"));
    }

    #[test]
    fn the_page_says_which_talk_and_which_slide_it_came_from() {
        // A snippet page is pasted into chat and found again six months later.
        // Code with no provenance is code nobody can trace back to the talk.
        let html = rust_page();

        assert!(html.contains("From Fast Decks, slide 1 — ubugeeei"));
        assert!(html.contains("<title>Retrying — Fast Decks</title>"));
    }

    #[test]
    fn a_deck_with_no_title_still_names_the_page() {
        let html = page("```rust {#retry .share}\nfn f() {}\n```\n");

        assert!(html.contains("<title>retry</title>"));
        assert!(html.contains("this deck"));
    }

    #[test]
    fn markup_inside_the_snippet_cannot_escape_the_page() {
        let html = page("```html {#markup .share}\n<script>alert(1)</script>\n```\n");

        assert!(!html.contains("<script"), "the snippet closed the element it is drawn in");
        assert!(html.contains("&lt;"), "the angle bracket was not escaped");
        assert!(html.contains("alert"), "the snippet itself is still shown");
    }

    #[test]
    fn every_colour_on_the_page_comes_from_a_theme_token() {
        // The same rule the slide stylesheet is held to. A snippet page that
        // hard-coded a background would be the one surface in the deck a theme
        // could not change and the audit could not see.
        assert!(!STYLESHEET.contains('#'), "the page stylesheet names a colour literally");
        assert_eq!(STYLESHEET.matches('{').count(), STYLESHEET.matches('}').count());
    }

    #[test]
    fn every_shared_block_in_a_deck_becomes_its_own_page() {
        let deck = parse_deck(
            concat!(
                "# One\n\n```rust {#a .share}\nfn a() {}\n```\n\n",
                "---\n\n# Two\n\n```py {#b .share}\ndef b(): pass\n```\n",
            ),
            &DeckParseOptions::default(),
        );

        let pages = render_snippets(&deck, &SnippetOptions::default());
        let paths: Vec<&str> = pages.iter().map(|page| page.path.as_str()).collect();

        assert_eq!(paths, vec!["snippets/a.html", "snippets/b.html"]);
    }

    #[test]
    fn a_page_renders_the_same_way_every_build() {
        let first = rust_page();
        assert_eq!(first, rust_page());
    }

    /// The text of the page's code block, with the highlighting taken back off.
    fn code_text(html: &str) -> String {
        let body = html.split_once("<pre><code").expect("a code block").1;
        let body = body.split_once('>').expect("an opened element").1;
        let body = body.split_once("</code></pre>").expect("a closed block").0;

        crate::highlight::unescape(&strip_tags(body))
    }

    fn strip_tags(html: &str) -> String {
        let mut out = String::new();
        let mut rest = html;

        while let Some(open) = rest.find('<') {
            out.push_str(&rest[..open]);
            rest = &rest[open + rest[open..].find('>').expect("a closed element") + 1..];
        }

        out.push_str(rest);
        out
    }
}
