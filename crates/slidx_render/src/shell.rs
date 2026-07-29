//! The page a slide is served as.
//!
//! One document per slide. Navigation is the browser's job — a built deck is
//! ordinary multi-page HTML — so a shell carries no router and no framework,
//! and a slide with no steps carries no JavaScript at all.
//!
//! Everything is inlined: theme, layout, and animation CSS all live in the
//! document. That is the offline guarantee expressed at the smallest scale —
//! a shell that references anything remote is a shell that goes blank when the
//! venue Wi-Fi does.

use slidx_core::{Deck, Slide, DEMO_ATTRIBUTE};
use slidx_theme::{css, transition, Theme};

use crate::layout;
use crate::markdown::{render, MarkdownOptions};

/// How to build a slide page.
#[derive(Debug, Clone)]
pub struct ShellOptions {
    pub theme: Theme,
    pub markdown: MarkdownOptions,
    /// Emitted into the page so the runtime can resolve steps. Omitted for
    /// slides with a single stop, which need no runtime at all.
    pub include_runtime: bool,
}

impl Default for ShellOptions {
    fn default() -> Self {
        Self {
            theme: slidx_theme::default_theme(),
            markdown: MarkdownOptions::default(),
            include_runtime: true,
        }
    }
}

/// Renders one slide as a complete HTML document.
pub fn render_slide(deck: &Deck, slide: &Slide, options: &ShellOptions) -> String {
    let body = render(&crate::snippet::stage(deck, slide, &options.theme), &options.markdown);
    let (width, height) = deck.meta.aspect.dimensions();

    let title = match (&slide.title, &deck.meta.title) {
        (Some(slide_title), Some(deck_title)) if slide_title != deck_title => {
            format!("{slide_title} — {deck_title}")
        }
        (Some(slide_title), _) => slide_title.clone(),
        (None, Some(deck_title)) => deck_title.clone(),
        (None, None) => format!("Slide {}", slide.index + 1),
    };

    format!(
        r#"<!doctype html>
<html lang="{lang}" data-slidx-aspect="{aspect}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
{description}<style>
{theme_css}
{shell_css}
{transition_css}
</style>
</head>
<body>
<main class="slidx-deck">
  <article class="slidx-slide" data-slidx-layout="{slide_layout}" style="--slidx-slide-width: {width}; --slidx-slide-height: {height}">
    <div class="slidx-slide-body">
{body}
{demo}    </div>
    <footer class="slidx-slide-footer">
      <span class="slidx-slide-brand">{brand}</span>
      <span class="slidx-slide-number">{number} / {count}</span>
    </footer>
  </article>
</main>
</body>
</html>
"#,
        lang = "en",
        aspect = deck.meta.aspect.as_token(),
        title = escape(&title),
        description = deck
            .meta
            .description
            .as_ref()
            .map(|text| format!("<meta name=\"description\" content=\"{}\">\n", escape(text)))
            .unwrap_or_default(),
        theme_css = css::render(&options.theme),
        shell_css = layout::STYLESHEET,
        transition_css = transition_css(deck, slide, &options.theme),
        slide_layout = slide.layout.as_deref().unwrap_or("stack"),
        width = width,
        height = height,
        body = body,
        demo = demo_markup(slide),
        brand = escape(
            deck.meta
                .talk
                .hashtag
                .as_ref()
                .map(|tag| format!("#{tag}"))
                .or_else(|| deck.meta.author.clone())
                .unwrap_or_default()
                .as_str()
        ),
        number = slide.index + 1,
        count = deck.slides.len(),
    )
}

/// The transition this slide leaves with.
///
/// A slide's own `transition:` wins over the deck's, because the interesting
/// case is one slide that moves differently — a section break, a demo — inside
/// a deck that otherwise cuts.
///
/// An unknown token was already reported when the deck was parsed, so it falls
/// back silently here rather than reporting the same mistake twice.
fn transition_css(deck: &Deck, slide: &Slide, theme: &Theme) -> String {
    let token = slide.transition.as_deref().or(deck.meta.transition.as_deref());
    let kind = token.and_then(transition::Transition::parse).unwrap_or_default();

    transition::render(theme, kind)
}

/// The declared demo, as markup that is complete before any script runs.
///
/// Both sides ship in the document and CSS paints one of them, so switching
/// costs one attribute write. The alternative — creating the video when the
/// demo fails — asks the network for a file at the exact moment the network is
/// the thing that died.
///
/// `muted` is not a preference. Browsers refuse to autoplay audio, so an
/// unmuted recording would sit on a first frame until someone found the play
/// button, which is the two minutes of fumbling this feature exists to delete.
/// The speaker is narrating over it live regardless.
///
/// The live side is the one place a shell is allowed to name a remote URL. A
/// demo that is not remote is not a demo, and the offline guarantee is about
/// the deck still working when that URL is gone — which is the fallback's job.
fn demo_markup(slide: &Slide) -> String {
    let Some(demo) = &slide.demo else { return String::new() };

    let live = format!(
        "      <iframe class=\"slidx-demo-live\" src=\"{}\" title=\"Live demo\"></iframe>\n",
        escape(&demo.live)
    );

    // A demo with no recording still renders its live side. The linter has
    // already reported the missing fallback; dropping the demo here would
    // punish the author on stage for a warning they read at their desk.
    let recording = match demo.fallback.as_deref().filter(|path| !path.trim().is_empty()) {
        Some(path) => format!(
            "      <video class=\"slidx-demo-fallback\" src=\"{}\"{} preload=\"auto\" muted playsinline controls></video>\n",
            escape(path),
            demo.poster
                .as_deref()
                .map(|poster| format!(" poster=\"{}\"", escape(poster)))
                .unwrap_or_default(),
        ),
        None => String::new(),
    };

    format!(
        "      <figure class=\"slidx-demo\" {DEMO_ATTRIBUTE}=\"live\">\n{live}{recording}      </figure>\n"
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn shell(source: &str) -> String {
        let deck = parse_deck(source, &DeckParseOptions::default());
        render_slide(&deck, &deck.slides[0], &ShellOptions::default())
    }

    const DEMO: &str =
        "---\ndemo:\n  live: https://app.example.com\n  fallback: ./checkout.mp4\n---\n\n# Live\n";

    #[test]
    fn a_declared_demo_puts_both_sides_in_the_document() {
        // Both sides ship in the markup so that switching is an attribute
        // write rather than a fetch. A fallback that has to be loaded when the
        // demo dies is not a fallback.
        let html = shell(DEMO);

        assert!(html.contains("src=\"https://app.example.com\""));
        assert!(html.contains("src=\"./checkout.mp4\""));
    }

    #[test]
    fn a_deck_with_no_script_still_shows_the_live_demo() {
        assert!(shell(DEMO).contains("data-slidx-demo=\"live\""));
    }

    #[test]
    fn the_recording_is_told_to_load_before_it_is_needed() {
        assert!(shell(DEMO).contains("preload=\"auto\""));
    }

    #[test]
    fn a_slide_with_no_demo_renders_no_demo_markup() {
        // Asserted on the element, not the attribute: the inlined stylesheet
        // names the attribute on every page whether or not a demo exists.
        assert!(!shell("# Ordinary\n").contains("<figure class=\"slidx-demo\""));
    }

    #[test]
    fn a_demo_with_no_recording_still_renders_its_live_side() {
        // The linter has already said so. Dropping the demo entirely would
        // punish the author on stage for a warning they saw at their desk.
        let html = shell("---\ndemo: https://app.example.com\n---\n\n# Live\n");

        assert!(html.contains("src=\"https://app.example.com\""));
        assert!(!html.contains("<video"));
    }

    #[test]
    fn a_demo_url_containing_markup_is_escaped() {
        let html = shell("---\ndemo:\n  live: \"https://x.test/?a=1&b=2\"\n---\n\n# Live\n");

        assert!(html.contains("a=1&amp;b=2"), "got: {html}");
    }

    #[test]
    fn a_poster_frame_is_used_when_the_deck_declares_one() {
        let source = "---\ndemo:\n  live: https://app.example.com\n  fallback: ./c.mp4\n  poster: ./c.png\n---\n\n# Live\n";
        assert!(shell(source).contains("poster=\"./c.png\""));
    }

    #[test]
    fn a_shell_is_a_complete_document() {
        let html = shell("# Hello\n");

        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<meta charset=\"utf-8\">"));
        assert!(html.trim_end().ends_with("</html>"));
    }

    #[test]
    fn nothing_in_the_shell_is_remote() {
        // The offline guarantee, at the smallest scale it can be checked.
        let html = shell("# Hello\n\n- one\n");

        for marker in ["http://", "https://", "//cdn", "@import url("] {
            assert!(!html.contains(marker), "shell reaches for {marker}:\n{html}");
        }
    }

    #[test]
    fn the_theme_is_inlined_rather_than_linked() {
        let html = shell("# Hello\n");

        assert!(html.contains("--slidx-color-text:"));
        assert!(!html.contains("<link"), "a stylesheet link is a network request");
    }

    #[test]
    fn the_title_combines_the_slide_and_the_deck() {
        let deck =
            parse_deck("---\ntitle: My Talk\n---\n\n# Intro\n", &DeckParseOptions::default());
        let html = render_slide(&deck, &deck.slides[0], &ShellOptions::default());

        assert!(html.contains("<title>Intro — My Talk</title>"));
    }

    #[test]
    fn a_slide_without_a_title_still_names_the_page() {
        assert!(shell("Just prose.\n").contains("<title>Slide 1</title>"));
    }

    #[test]
    fn the_deck_title_is_not_repeated_when_the_slide_shares_it() {
        let deck =
            parse_deck("---\ntitle: My Talk\n---\n\n# My Talk\n", &DeckParseOptions::default());
        let html = render_slide(&deck, &deck.slides[0], &ShellOptions::default());

        assert!(html.contains("<title>My Talk</title>"));
    }

    #[test]
    fn the_slide_carries_its_aspect_ratio_for_layout() {
        let deck = parse_deck("---\naspect: \"4:3\"\n---\n\n# One\n", &DeckParseOptions::default());
        let html = render_slide(&deck, &deck.slides[0], &ShellOptions::default());

        assert!(html.contains("data-slidx-aspect=\"4:3\""));
        assert!(html.contains("--slidx-slide-width: 1440"));
    }

    #[test]
    fn the_layout_frontmatter_reaches_the_markup() {
        let deck = parse_deck("---\nlayout: split\n---\n\n# One\n", &DeckParseOptions::default());
        let html = render_slide(&deck, &deck.slides[0], &ShellOptions::default());

        assert!(html.contains("data-slidx-layout=\"split\""));
    }

    #[test]
    fn slides_are_numbered_for_the_audience() {
        let deck = parse_deck("# One\n\n---\n\n# Two\n", &DeckParseOptions::default());
        let html = render_slide(&deck, &deck.slides[1], &ShellOptions::default());

        assert!(html.contains("2 / 2"));
    }

    #[test]
    fn a_hashtag_is_shown_so_a_screenshot_carries_its_source() {
        // Any single slide can be the one that gets photographed and shared.
        let deck =
            parse_deck("---\nhashtag: slidxconf\n---\n\n# One\n", &DeckParseOptions::default());
        let html = render_slide(&deck, &deck.slides[0], &ShellOptions::default());

        assert!(html.contains("#slidxconf"));
    }

    #[test]
    fn step_anchors_survive_into_the_page() {
        let html = shell("- one <!-- step -->\n- two <!-- step -->\n");
        assert_eq!(html.matches("data-slidx-step=").count(), 2);
    }

    #[test]
    fn marks_survive_into_the_page() {
        let html = shell("The answer is [42]{#count .accent}.\n");

        assert!(html.contains("data-slidx-mark=\"count\""));
        assert!(html.contains("slidx-accent"));
    }

    #[test]
    fn the_body_is_emitted_verbatim() {
        // Pretty-printing the body would indent the inside of a `<pre>`, where
        // whitespace is content: every code block on every slide would gain a
        // phantom indent. Readable output is not worth corrupting code.
        let html = shell("```rust\nfn main() {\n    let x = 1;\n}\n```\n");

        let block = html.split_once("<code class=\"language-rust\">").expect("a code block").1;
        let block = block.split_once("</code>").expect("a closed block").0;

        assert!(block.contains("\n    "), "the indent was reflowed:\n{block}");
        assert!(block.ends_with('\n'), "the trailing newline was trimmed:\n{block}");
    }

    #[test]
    fn code_on_a_slide_arrives_coloured_and_still_carries_no_script() {
        // The whole reason highlighting happens at build time: the page an
        // audience sees is a string by then.
        let html = shell("```rust\nlet x = 1; // one\n```\n");

        assert!(html.contains("<span class=\"slidx-code-keyword\">let</span>"));
        assert!(html.contains("--slidx-color-code-comment:"));
        assert!(!html.contains("<script"), "an audience slide ships no JavaScript");
    }

    #[test]
    fn a_deck_transition_reaches_the_page() {
        let deck =
            parse_deck("---\ntransition: fade\n---\n\n# One\n", &DeckParseOptions::default());
        let html = render_slide(&deck, &deck.slides[0], &ShellOptions::default());

        assert!(html.contains("@view-transition"));
    }

    #[test]
    fn a_slide_transition_overrides_the_decks() {
        // The interesting case is one slide that moves differently — a
        // section break, a demo — inside a deck that otherwise cuts.
        let deck = parse_deck(
            "---\ntransition: none\n---\n\n# One\n\n---\ntransition: fade\n---\n\n# Two\n",
            &DeckParseOptions::default(),
        );

        let first = render_slide(&deck, &deck.slides[0], &ShellOptions::default());
        let second = render_slide(&deck, &deck.slides[1], &ShellOptions::default());

        assert!(!first.contains("@view-transition"));
        assert!(second.contains("@view-transition"));
    }

    #[test]
    fn a_deck_that_asks_for_no_transition_carries_no_transition_css() {
        // The opt-in itself costs the browser two page snapshots, so `none`
        // has to mean absent rather than a zero-duration animation.
        assert!(!shell("# One\n").contains("@view-transition"));
    }

    #[test]
    fn a_title_containing_markup_is_escaped() {
        let deck = parse_deck(
            "---\ntitle: \"a <script> & b\"\n---\n\n# One\n",
            &DeckParseOptions::default(),
        );
        let html = render_slide(&deck, &deck.slides[0], &ShellOptions::default());

        assert!(!html.contains("<title>a <script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
