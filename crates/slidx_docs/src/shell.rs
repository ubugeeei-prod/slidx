//! One page as a complete HTML document.
//!
//! # Everything is in the file
//!
//! The brand tokens, the deck theme's code colours, the stylesheet and the mark
//! are all inlined, so a page is one document that renders with nothing beside
//! it. That is the same decision the slide shell makes, and it is here for a
//! reason a documentation site has of its own: the reader most likely to need
//! this site is on a conference network the hour before they speak, or on a
//! plane with a page they saved. A saved page that lost its stylesheet is a page
//! that lost its meaning.
//!
//! # No script
//!
//! Not one line, on any page. A documentation site for a framework whose whole
//! argument is that a slide should render before any script runs cannot be an
//! application, and everything here — navigation, the current page, both colour
//! schemes — is a link, an attribute, or a media query.

use slidx_brand::{css as brand_css, mark, WORDMARK};

use crate::nav::Section;
use crate::page::Page;
use crate::style::STYLESHEET;

/// What the site is, in the one line under the wordmark.
const TAGLINE: &str = "Markdown decks, compiled to static pages";

/// Renders one page as a whole document.
///
/// `pages` is every page on the site, because the navigation is built from it:
/// a page cannot be published without appearing in the navigation, and a
/// navigation entry cannot point at a page that was not published.
pub fn render(page: &Page, pages: &[Page]) -> String {
    // Two hashes rather than one: the skip link's `href="#main"` would close a
    // single-hash raw string.
    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<meta name="description" content="{summary}">
<style>
{brand}
{code}
{stylesheet}
</style>
</head>
<body>
<a class="slidx-docs-skip" href="#main">Skip to the page</a>
<header class="slidx-docs-header">
  <a class="slidx-docs-lockup" href="index.html">{mark}<span>{wordmark}</span></a>
  <p class="slidx-docs-tagline">{tagline}</p>
</header>
<div class="slidx-docs-body">
<nav class="slidx-docs-nav" aria-label="Sections">
{navigation}
</nav>
<main class="slidx-docs-main" id="main">
<article class="slidx-docs-prose">
{body}
</article>
</main>
</div>
<footer class="slidx-docs-footer">
  <p>slidx is pre-alpha and unreleased. <a href="https://github.com/ubugeeei-prod/slidx">The repository</a> is the whole of it.</p>
</footer>
</body>
</html>
"##,
        title = escape(&title(page)),
        summary = escape(&page.summary),
        brand = brand_css::render(),
        code = code_colours(),
        stylesheet = STYLESHEET,
        mark = mark::render_mono("currentColor"),
        wordmark = WORDMARK,
        tagline = TAGLINE,
        navigation = navigation(page, pages),
        body = page.html(),
    )
}

/// What the browser tab says.
///
/// The front page is the product's name and nothing else; every other page
/// leads with its own title, because a reader with six tabs open is looking for
/// the one that says "The night before" and not the one that says "slidx —".
fn title(page: &Page) -> String {
    if page.slug == "index" {
        format!("{WORDMARK} — {TAGLINE}")
    } else {
        format!("{} — {WORDMARK}", page.title)
    }
}

/// The deck theme's code colours.
///
/// Emitted from the default theme rather than restated, so a fenced block on
/// this site is coloured by the palette the same code gets on a slide. The
/// theme's sizes come along with it and go unused: they are quoted in `cqh`,
/// shares of a slide's height, and this stylesheet asks for none of them.
fn code_colours() -> String {
    slidx_theme::css::render(&slidx_theme::default_theme())
}

/// Sections, and the pages inside them.
///
/// A section with no pages is skipped entirely, so the navigation can never
/// offer a door that opens onto nothing — which is what would otherwise happen
/// every time a section is added before the pages that fill it.
///
/// A section holding exactly one page *is* that page, and is drawn as a single
/// link under the section's own label. The alternative reads as "Start / Start"
/// and "The night before / The night before": two lines of chrome saying one
/// thing, on a rail whose entire job is to name the four situations a reader
/// might be in.
fn navigation(current: &Page, pages: &[Page]) -> String {
    let mut html = String::from("<ol>\n");

    for section in Section::ALL {
        let mut in_section: Vec<&Page> =
            pages.iter().filter(|page| page.section == section).collect();

        if in_section.is_empty() {
            continue;
        }

        in_section.sort_by_key(|page| (page.order, page.slug.clone()));

        html.push_str("<li>\n");

        if let [only] = in_section[..] {
            html.push_str(&entry(only, current, section.label()));
            html.push('\n');
        } else {
            html.push_str(&format!("<h2>{}</h2>\n<ol>\n", escape(section.label())));
            for page in in_section {
                html.push_str(&format!("<li>{}</li>\n", entry(page, current, &page.title)));
            }
            html.push_str("</ol>\n");
        }

        html.push_str("</li>\n");
    }

    html.push_str("</ol>");
    html
}

/// One link in the navigation.
fn entry(page: &Page, current: &Page, label: &str) -> String {
    let marker = if page.slug == current.slug { r#" aria-current="page""# } else { "" };
    format!("<a href=\"{}\"{marker}>{}</a>", escape(&page.file_name()), escape(label))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(slug: &str, section: &str, order: u32, title: &str) -> Page {
        let source = format!(
            "---\ntitle: {title}\nsummary: One sentence.\nsection: {section}\norder: {order}\n---\n\n# {title}\n",
        );
        Page::parse(slug, &source).expect("a valid page")
    }

    fn site() -> Vec<Page> {
        vec![
            page("index", "start", 1, "Start"),
            page("choosing", "choosing", 1, "Choosing slidx for a talk"),
            page("tonight", "tonight", 1, "The night before"),
        ]
    }

    fn html() -> String {
        let pages = site();
        render(&pages[0], &pages)
    }

    #[test]
    fn the_page_ships_no_script_at_all() {
        // The site for a framework that argues a slide should render before any
        // script runs cannot itself need one.
        let html = html();

        assert!(!html.contains("<script"));
        assert!(!html.contains("onclick"));
        assert!(!html.contains("javascript:"));
    }

    #[test]
    fn nothing_on_the_page_is_remote() {
        // The same rule the deck output is held to, and the reason this site can
        // be read on a conference network with two bars.
        let html = html();

        for marker in ["<link", "@import", "//cdn", "src=\"http", "href=\"http"] {
            let remote = html.matches(marker).count();
            let allowed = if marker == "href=\"http" { html.matches("href=\"https://github.com/ubugeeei-prod/slidx").count() } else { 0 };
            assert_eq!(remote, allowed, "the page reaches for {marker}");
        }
    }

    #[test]
    fn the_brand_tokens_come_from_the_crate_that_generates_them() {
        // Read from `slidx_brand` rather than copied, so a colour changed in
        // Rust reaches this site by rebuilding rather than by remembering.
        let html = html();

        assert!(html.contains("--slidx-brand-signal:"));
        assert!(html.contains("--slidx-brand-space-step:"));
    }

    #[test]
    fn code_is_coloured_by_the_palette_a_slide_would_get() {
        assert!(html().contains("--slidx-color-code-keyword:"));
    }

    #[test]
    fn every_section_that_has_a_page_appears_in_the_navigation() {
        let html = html();

        for section in [Section::Start, Section::Choosing, Section::Tonight] {
            assert!(html.contains(section.label()), "no {} in the nav", section.label());
        }
    }

    #[test]
    fn a_section_with_no_pages_does_not_appear_in_the_navigation() {
        // Reference is added a page at a time. Until the first one lands, a
        // heading for it would be a door onto nothing.
        assert!(!html().contains(Section::Reference.label()));
    }

    #[test]
    fn the_page_being_read_is_marked_as_the_current_one() {
        let pages = site();
        let html = render(&pages[2], &pages);

        assert!(html.contains(r#"<a href="tonight.html" aria-current="page">"#), "got {html}");
        assert!(!html.contains(r#"<a href="index.html" aria-current="page">"#));
    }

    #[test]
    fn a_section_holding_one_page_is_drawn_as_one_link_under_its_own_label() {
        // Otherwise the rail reads "The night before / The night before", which
        // is two lines of chrome saying one thing.
        let html = html();

        assert!(html.contains(r#"<a href="tonight.html">The night before</a>"#), "got {html}");
        assert!(!html.contains("<h2>The night before</h2>"));
    }

    #[test]
    fn a_section_holding_several_pages_names_them_under_a_heading() {
        let pages = vec![
            page("index", "start", 1, "Start"),
            page("frontmatter", "reference", 1, "Frontmatter"),
            page("rules", "reference", 2, "Lint rules"),
        ];
        let html = render(&pages[0], &pages);

        assert!(html.contains("<h2>Reference</h2>"));
        assert!(html.contains(">Frontmatter</a>"));
        assert!(html.contains(">Lint rules</a>"));
    }

    #[test]
    fn pages_within_a_section_are_listed_in_their_declared_order() {
        let pages = vec![
            page("index", "start", 1, "Start"),
            page("second", "start", 2, "Second"),
            page("third", "start", 3, "Third"),
        ];
        let html = render(&pages[0], &pages);

        let position = |needle: &str| html.find(needle).expect("a nav entry");
        assert!(position("second.html") < position("third.html"));
    }

    #[test]
    fn the_front_page_is_titled_with_the_product_and_every_other_with_itself() {
        let pages = site();

        assert!(render(&pages[0], &pages).contains("<title>slidx — "));
        assert!(render(&pages[2], &pages).contains("<title>The night before — slidx</title>"));
    }

    #[test]
    fn the_summary_becomes_the_description_a_search_result_shows() {
        assert!(html().contains(r#"<meta name="description" content="One sentence.">"#));
    }

    #[test]
    fn the_mark_is_the_one_the_brand_generates() {
        // Inlined rather than linked, so a saved page keeps it, and in its
        // single-colour form so it follows the ink in both schemes.
        let html = html();

        assert!(html.contains("viewBox=\"0 0 24 24\""));
        assert!(html.contains("currentColor"));
    }

    #[test]
    fn there_is_a_way_past_the_navigation_for_a_reader_who_cannot_point() {
        let html = html();

        assert!(html.contains(r##"href="#main""##));
        assert!(html.contains(r#"id="main""#));
    }
}
