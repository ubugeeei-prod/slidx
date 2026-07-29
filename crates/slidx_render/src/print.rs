//! One document, one page per stop.
//!
//! A handout that collapses an eight-step build into one slide is a handout
//! that shows the punchline without the setup. So the printed deck has a page
//! for every *stop*, not for every slide, and reading it follows the same
//! order the audience saw.
//!
//! The expansion happens in the browser rather than here. Stops are resolved
//! against the DOM — an anchor stages the element it follows, which is a
//! question about a rendered tree — so the print shell ships the timeline as
//! data and lets the same runtime the projector uses do the work. One
//! implementation, so the PDF cannot disagree with the screen.

use slidx_core::Deck;
use slidx_theme::{css, Theme};

use crate::markdown::{render, MarkdownOptions};
use crate::print_layout;

/// How to build the print shell.
#[derive(Debug, Clone)]
pub struct PrintOptions {
    pub theme: Theme,
    pub markdown: MarkdownOptions,
    /// Module URL of the runtime, which expands the stops.
    pub runtime_src: String,
    /// The runtime's source, inlined instead of imported.
    ///
    /// A browser refuses to resolve a module import over `file://` — it is a
    /// cross-origin request from a null origin, whatever the path says. So the
    /// document a person opens from a USB stick, an email attachment, or a PDF
    /// exporter has to carry its own script. Twelve kilobytes buys a file that
    /// works anywhere, which is the entire point of a printable fallback.
    pub inline_runtime: Option<String>,
    /// Page size, as CSS. Defaults to the deck's aspect ratio at 10in wide.
    pub page_size: Option<String>,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            theme: slidx_theme::default_theme(),
            markdown: MarkdownOptions::default(),
            runtime_src: "./runtime.js".to_string(),
            inline_runtime: None,
            page_size: None,
        }
    }
}

/// Renders the whole deck as one printable document.
pub fn render_print(deck: &Deck, options: &PrintOptions) -> String {
    let (width, height) = deck.meta.aspect.dimensions();

    let sections: String = deck
        .slides
        .iter()
        .map(|slide| {
            format!(
                // The size custom properties go on the *page*, because that is
                // the element whose `aspect-ratio` uses them. Putting them on
                // the slide inside leaves the rule resolving to `auto` and the
                // page collapses to the height of its text.
                r#"  <section class="slidx-page" data-slidx-slide="{index}" data-slidx-stops="{stops}" style="--slidx-slide-width: {width}; --slidx-slide-height: {height}">
    <article class="slidx-slide" data-slidx-layout="{layout}">
      <div class="slidx-slide-body">
{body}
      </div>
      <footer class="slidx-slide-footer">
        <span class="slidx-slide-brand">{brand}</span>
        <span class="slidx-slide-number">{number}</span>
      </footer>
    </article>
  </section>"#,
                index = slide.index,
                stops = slide.timeline.len(),
                layout = slide.layout.as_deref().unwrap_or("stack"),
                width = width,
                height = height,
                body = render(&slide.content, &options.markdown),
                brand = escape(&brand(deck)),
                number = slide.index + 1,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<!doctype html>
<html lang="en" data-slidx-print>
<head>
<meta charset="utf-8">
<title>{title}</title>
<style>
{theme_css}
{print_css}
@page {{ size: {page_size}; margin: 0; }}
</style>
</head>
<body>
<main class="slidx-print">
{sections}
</main>
<script type="module">
{script}
</script>
</body>
</html>
"#,
        title = escape(deck.meta.title.as_deref().unwrap_or("slidx")),
        theme_css = css::render(&options.theme),
        print_css = print_layout::STYLESHEET,
        page_size = options
            .page_size
            .clone()
            // Ten inches wide, and whatever the deck's ratio makes that tall. A
            // page shaped like the slide is what removes scaling entirely.
            .unwrap_or_else(|| format!(
                "10in {}in",
                inches(10.0 * f64::from(height) / f64::from(width))
            )),
        sections = sections,
        script = script(deck, options),
    )
}

/// A length in inches, without trailing zeros.
///
/// `5.6250in` and `5.625in` mean the same thing to a browser and different
/// things to a person reading a diff.
fn inches(value: f64) -> String {
    format!("{value:.4}").trim_end_matches('0').trim_end_matches('.').to_string()
}

fn brand(deck: &Deck) -> String {
    deck.meta
        .talk
        .hashtag
        .as_ref()
        .map(|tag| format!("#{tag}"))
        .or_else(|| deck.meta.author.clone())
        .unwrap_or_default()
}

/// Expands each slide into one page per stop.
///
/// Every page is a clone with one frame applied, which is only correct because
/// frames are complete snapshots: page *n* does not depend on page *n-1*
/// having been rendered first.
fn script(deck: &Deck, options: &PrintOptions) -> String {
    let timelines =
        serde_json::to_string(&deck.slides.iter().map(|slide| &slide.timeline).collect::<Vec<_>>())
            .unwrap_or_else(|_| "[]".to_string());

    // Inlined source lands in the same module scope, so its exports are
    // already in scope and there is nothing to import.
    let preamble = match &options.inline_runtime {
        Some(source) => source.clone(),
        None => {
            format!("import {{ createStage, markScriptEnabled }} from \"{}\";", options.runtime_src)
        }
    };

    format!(
        r#"{preamble}

markScriptEnabled(document);

const timelines = {timelines};

for (const page of [...document.querySelectorAll(".slidx-page")]) {{
  const index = Number(page.dataset.slidxSlide);
  const timeline = timelines[index];
  const stops = timeline?.frames?.length ?? 1;

  // One stop is one page already; cloning would only duplicate it.
  if (stops < 2) {{
    createStage(page.querySelector(".slidx-slide"), timeline ?? {{ frames: [] }}).apply(0);
    continue;
  }}

  const pages = [page];
  for (let stop = 1; stop < stops; stop += 1) {{
    const clone = page.cloneNode(true);
    page.parentNode.insertBefore(clone, pages[stop - 1].nextSibling);
    pages.push(clone);
  }}

  // Staged after all the clones exist: binding resolves anchors by mutating
  // the tree, so cloning an already-staged page would copy the resolution
  // rather than perform it.
  pages.forEach((element, stop) => {{
    createStage(element.querySelector(".slidx-slide"), timeline).apply(stop);
  }});
}}

document.documentElement.dataset.slidxPrintReady = "";
"#,
        preamble = preamble,
        timelines = timelines,
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn print(source: &str) -> String {
        let deck = parse_deck(source, &DeckParseOptions::default());
        render_print(&deck, &PrintOptions::default())
    }

    #[test]
    fn every_slide_gets_a_page() {
        let html = print("# One\n\n---\n\n# Two\n");
        assert_eq!(html.matches("class=\"slidx-page\"").count(), 2);
    }

    #[test]
    fn a_page_declares_how_many_stops_it_expands_to() {
        // The expansion happens in the browser, so the count has to travel
        // with the markup rather than being recomputed there.
        let html = print("- a <!-- step -->\n- b <!-- step -->\n");
        assert!(html.contains("data-slidx-stops=\"3\""));
    }

    #[test]
    fn the_timeline_travels_with_the_document() {
        let html = print("- a <!-- step -->\n");
        assert!(html.contains("const timelines = ["));
        assert!(html.contains("\"frames\""));
    }

    #[test]
    fn the_page_size_follows_the_decks_aspect_ratio() {
        assert!(print("# One\n").contains("size: 10in 5.625in"));

        let deck = parse_deck("---\naspect: \"4:3\"\n---\n\n# One\n", &DeckParseOptions::default());
        assert!(render_print(&deck, &PrintOptions::default()).contains("size: 10in 7.5in"));
    }

    #[test]
    fn the_size_properties_sit_on_the_element_that_uses_them() {
        // `aspect-ratio` is set on `.slidx-page`. Declaring the properties on
        // the slide inside leaves it resolving to `auto`, and every page
        // collapses to the height of its own text — which looks like the
        // deck failing to render rather than like a CSS mistake.
        let html = print("# One\n");
        let page = &html[html.find("slidx-page").unwrap()..];

        assert!(page[..300].contains("--slidx-slide-width"));
    }

    #[test]
    fn margins_are_zero_so_a_slide_fills_its_page() {
        assert!(print("# One\n").contains("margin: 0"));
    }

    #[test]
    fn pages_are_numbered_by_slide_rather_than_by_stop() {
        // A handout reader wants "slide 3", not "page 7 of a build".
        let html = print("# One\n\n---\n\n# Two\n");
        assert!(html.contains(">1</span>"));
        assert!(html.contains(">2</span>"));
    }

    #[test]
    fn an_inlined_runtime_replaces_the_import() {
        // A module import fails over `file://` whatever the path says: it is
        // a cross-origin request from a null origin. The document a speaker
        // opens from a USB stick has to carry its own script.
        let deck = parse_deck("# One\n", &DeckParseOptions::default());
        let html = render_print(
            &deck,
            &PrintOptions {
                inline_runtime: Some("function createStage() {}".to_string()),
                ..PrintOptions::default()
            },
        );

        assert!(html.contains("function createStage() {}"));
        assert!(!html.contains("import {"));
    }

    #[test]
    fn nothing_in_the_print_shell_is_remote() {
        let html = print("# One\n");
        for marker in ["http://", "https://", "//cdn"] {
            assert!(!html.contains(marker), "print shell reaches for {marker}");
        }
    }

    #[test]
    fn it_signals_when_the_expansion_has_finished() {
        // A PDF exporter that prints before the clones exist gets one page per
        // slide and no build at all.
        assert!(print("# One\n").contains("slidxPrintReady"));
    }

    #[test]
    fn a_deck_title_containing_markup_is_escaped() {
        let deck = parse_deck(
            "---\ntitle: \"a <script> b\"\n---\n\n# One\n",
            &DeckParseOptions::default(),
        );
        assert!(render_print(&deck, &PrintOptions::default()).contains("&lt;script&gt;"));
    }
}
