//! The page a slide is served as.
//!
//! One document per slide. Navigation is the browser's job — a built deck is
//! ordinary multi-page HTML — so a shell carries no router and no framework,
//! and a slide with no steps fetches nothing at all.
//!
//! "The browser's job" was doing a lot of work in that sentence for a long
//! time: it is only true once the document contains a link, and this one did
//! not. See [`crate::navigation`].
//!
//! Everything is inlined: theme, layout, and animation CSS all live in the
//! document. That is the offline guarantee expressed at the smallest scale —
//! a shell that references anything remote is a shell that goes blank when the
//! venue Wi-Fi does.

use std::sync::{Arc, OnceLock};

use slidx_core::{Deck, Slide, CAMERA_ATTRIBUTE, CAMERA_STATE_ATTRIBUTE, DEMO_ATTRIBUTE};
use slidx_theme::layout::Layout;
use slidx_theme::{css, transition, Theme};

use crate::layout;
use crate::markdown::MarkdownOptions;
use crate::region;
use crate::seo::{self, SeoOptions};

/// How to build a slide page.
#[derive(Debug, Clone)]
pub struct ShellOptions {
    pub theme: Theme,
    pub markdown: MarkdownOptions,
    /// Module URL a slide with steps imports the runtime from.
    pub runtime_src: String,
    /// Emitted into the page so the runtime can resolve steps. Omitted for
    /// slides with a single stop, which need no runtime at all.
    pub include_runtime: bool,
    /// What the page may say about where it lives and who may index it.
    pub seo: SeoOptions,
    /// The theme, compiled, once.
    ///
    /// Every slide in a deck inlines the same theme CSS — that is the offline
    /// guarantee and it is not negotiable — but it was *rendered* once per
    /// slide, and a deck is the one place that loop is long. Held behind an
    /// `Arc` so `ShellOptions` stays `Clone`, and filled on first use rather
    /// than in `Default` so constructing options costs nothing.
    ///
    /// Not part of the API: set it through [`ShellOptions::with_theme`], which
    /// is the only way to change the theme without leaving a stale stylesheet
    /// behind it. Public only so `..Default::default()` keeps working.
    #[doc(hidden)]
    pub theme_css: Arc<OnceLock<String>>,
}

impl ShellOptions {
    /// The theme as CSS, rendered on the first slide and reused by the rest.
    pub fn theme_css(&self) -> &str {
        self.theme_css.get_or_init(|| css::render(&self.theme))
    }

    /// Options for a different theme, with an empty cache.
    ///
    /// The setter exists because assigning `theme` directly would leave a
    /// stale stylesheet behind it, and a field nobody can write is a field
    /// nobody can get wrong.
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.theme_css = Arc::default();
        self
    }
}

impl Default for ShellOptions {
    fn default() -> Self {
        Self {
            theme: slidx_theme::default_theme(),
            markdown: MarkdownOptions::default(),
            runtime_src: "./runtime.js".to_string(),
            include_runtime: true,
            seo: SeoOptions::default(),
            theme_css: Arc::default(),
        }
    }
}

/// Renders one slide as a complete HTML document.
pub fn render_slide(deck: &Deck, slide: &Slide, options: &ShellOptions) -> String {
    let title = document_title(deck, slide);
    let slide_layout = region::layout_of(slide);
    let frame = slide_frame(
        deck,
        slide,
        options,
        &slide_layout,
        &demo_markup(slide),
        &camera_markup(slide, &slide_layout),
        &crate::navigation::links(deck, slide),
    );

    format!(
        r#"<!doctype html>
<html lang="{lang}" data-slidx-aspect="{aspect}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
{seo}<style>
{theme_css}
{shell_css}
{layout_css}
{transition_css}
</style>
</head>
<body>
{frame}{script}</body>
</html>
"#,
        // A declared `lang:` where there is one, and the slides' own characters
        // where there is not. An unset attribute is worse than a wrong one —
        // assistive technology can be corrected by a reader, silence cannot —
        // and `en` on a Japanese deck was a wrong one that also decides how
        // every line in it breaks. See `slidx_core::language`.
        lang = escape(deck.language()),
        aspect = deck.meta.aspect.as_token(),
        title = escape(&title),
        seo = seo::head(deck, slide, &options.seo),
        theme_css = options.theme_css(),
        shell_css = layout::STYLESHEET,
        layout_css = slidx_theme::layout::stylesheet(),
        transition_css = transition_css(deck, slide, &options.theme),
        // A staged slide's stage binds the keyboard and the mirror through the
        // runtime it already loads; every other slide had neither, which is
        // most of a deck. See `crate::navigation`.
        script = match stage_script(deck, slide, options) {
            staged if staged.is_empty() => crate::navigation::script(deck, slide),
            staged => staged,
        },
    )
}

/// Renders the real slide frame into an isolated, inert document for previews.
///
/// It shares the audience frame writer rather than reflowing Markdown a second
/// way. Live demos, cameras, navigation, and transitions stay out: a preview
/// must show the authored geometry without starting venue-only behaviour.
pub(crate) fn render_static_preview(deck: &Deck, slide: &Slide, options: &ShellOptions) -> String {
    let slide_layout = region::layout_of(slide);
    let frame = slide_frame(deck, slide, options, &slide_layout, "", "", "");

    format!(
        r#"<!doctype html>
<html lang="{lang}" data-slidx-aspect="{aspect}" data-slidx-preview>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
{theme_css}
{shell_css}
{layout_css}
</style>
</head>
<body>
{frame}</body>
</html>
"#,
        lang = escape(deck.language()),
        aspect = deck.meta.aspect.as_token(),
        title = escape(&slide.display_title()),
        theme_css = options.theme_css(),
        shell_css = layout::STYLESHEET,
        layout_css = slidx_theme::layout::stylesheet(),
    )
}

fn document_title(deck: &Deck, slide: &Slide) -> String {
    match (&slide.title, &deck.meta.title) {
        (Some(slide_title), Some(deck_title)) if slide_title != deck_title => {
            format!("{slide_title} — {deck_title}")
        }
        (Some(slide_title), _) => slide_title.clone(),
        (None, Some(deck_title)) => deck_title.clone(),
        (None, None) => format!("Slide {}", slide.index + 1),
    }
}

fn slide_frame(
    deck: &Deck,
    slide: &Slide,
    options: &ShellOptions,
    slide_layout: &Layout,
    appended: &str,
    camera: &str,
    // The footer's neighbours, or nothing for a preview — which is inert by
    // definition and would otherwise offer a link out of itself.
    navigation: &str,
) -> String {
    let body = region::body(deck, slide, slide_layout, &options.theme, &options.markdown, appended);
    let (width, height) = deck.meta.aspect.dimensions();

    format!(
        r#"<main class="slidx-deck">
  <article class="slidx-slide" data-slidx-layout="{slide_layout}" style="--slidx-slide-width: {width}; --slidx-slide-height: {height};{slide_style}">
    <div class="slidx-slide-body">
{body}{camera}    </div>
    <footer class="slidx-slide-footer">
      <span class="slidx-slide-brand">{brand}</span>
{navigation}    </footer>
  </article>
</main>
"#,
        slide_layout = slide_layout.id,
        width = width,
        height = height,
        slide_style = crate::slide_style::declarations(slide),
        body = body,
        camera = camera,
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
        navigation = if navigation.is_empty() {
            format!(
                "      <span class=\"slidx-slide-number\">{} / {}</span>\n",
                slide.index + 1,
                deck.slides.len()
            )
        } else {
            navigation.to_string()
        },
    )
}

/// The wiring that makes a slide's stops reachable, or nothing at all.
///
/// Nothing at all is the common case and the one the front page claims: a
/// slide with one stop is finished markup, and shipping a module to it would
/// buy an author nothing and cost every audience a request on venue Wi-Fi.
///
/// A slide with steps is the other case, and it needs exactly this much. The
/// compiled timeline travels *in* the document rather than being fetched, so a
/// staged slide costs one request for the shared runtime module and nothing
/// per slide — and so the timeline cannot be a version behind the markup it
/// stages.
///
/// The script is written inline for the same reason the presenter's is: it is
/// a few lines of wiring around a runtime that is already tested, and a
/// bundled entry point would be a second thing to keep in step with it.
///
/// **A module import does not resolve over `file://`.** The request is
/// cross-origin from a null origin whatever the path says, so a staged slide
/// opened straight off a USB stick shows its first stop and stops there. That
/// case belongs to the print shell, which inlines the runtime for exactly this
/// reason; a slide is served, and serving it is what the plugin does.
fn stage_script(deck: &Deck, slide: &Slide, options: &ShellOptions) -> String {
    if !options.include_runtime || slide.timeline.frames().len() < 2 {
        return String::new();
    }

    let timeline =
        serde_json::to_string(&slide.timeline).unwrap_or_else(|_| r#"{"frames":[]}"#.to_string());

    format!(
        r#"<script type="module">
import {{ createStage, createNavigator, createMirror, loadEffects, markScriptEnabled, LAST_STEP }} from "{runtime_src}";

// Staging is gated on this attribute, so a deck whose script never arrived
// shows every element rather than a slide that is mostly invisible. Setting it
// is therefore what *switches staging on*, and it has to happen before the
// first frame is applied or the slide flashes its whole content on load.
// The runtime is a virtual module in development, so its `import.meta.url`
// names Vite's internal module id rather than the public file beside the deck.
// Resolve the stylesheet from the public runtime URL the caller supplied: the
// same address works in development and in the emitted site.
const effects = new URL("./effects.css", new URL("{runtime_src}", location.href)).href;
if (await loadEffects(document, effects)) markScriptEnabled(document);

const stage = createStage(document.querySelector(".slidx-slide"), {timeline});

// Slide one lives at the deck root and the rest live one directory down, so
// what "up" means depends on which slide is asking.
const up = {index} === 0 ? "./" : "../";
const hrefFor = (slide, step) => {{
  const path = slide === 0 ? up : `${{up}}${{slide + 1}}/`;
  return step === undefined ? path : `${{path}}?step=${{step}}`;
}};

const opening = new URLSearchParams(location.search).get("step");

const deck = createNavigator({{
  stage,
  slide: {index},
  slideCount: {count},
  step: opening === null ? undefined : opening === "last" ? LAST_STEP : Number(opening),
  hrefFor,
}});

// The projector window has its own keyboard, and a clicker sends keys to
// whichever window is focused — usually this one.
addEventListener("keydown", (event) => deck.handleKey(event));

// And the presenter view drives it from the other screen. `show` deliberately
// does not announce, so two windows cannot volley one move forever.
const mirror = createMirror();
mirror.subscribe((position) => deck.show(position));
deck.subscribe((position) => mirror.send(position));
</script>
"#,
        runtime_src = options.runtime_src,
        timeline = timeline,
        index = slide.index,
        count = deck.slides.len(),
    )
}

/// The transition this slide arrives with.
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

/// The camera's *place* on the slide. Never a camera.
///
/// This is the whole boundary the feature is built on. A published deck is a
/// static page anybody may open — from a link, a QR code, an archive years
/// later — and a page that asks for a webcam is a page people close. So what
/// the build emits is an empty tile in the region the author named, with no
/// `<video>`, no script, and nothing anywhere in the document that could reach
/// a device. Starting a stream is a second opt-in, made by the speaker at
/// presentation time, and it happens in the runtime.
///
/// The tile starts in the `idle` state, which the stylesheet draws as nothing
/// at all. That is the state every published page stays in, because the only
/// thing that writes the attribute is `enterPresentation`.
///
/// A region the layout does not have falls back to the layout's default rather
/// than being emitted as written. The mistake was already reported by
/// [`slidx_theme::layout::diagnose`], and a `grid-area` naming nothing puts the
/// tile in an implicit track — which moves every region down, so a stale
/// `camera:` would rearrange the slide rather than misplace one element.
fn camera_markup(slide: &Slide, layout: &Layout) -> String {
    let Some(camera) = &slide.camera else { return String::new() };

    let region = match layout.region(&camera.region) {
        Some(region) => region.name.as_str(),
        None => layout.fallback().name.as_str(),
    };

    format!(
        "      <figure class=\"slidx-camera\" {CAMERA_ATTRIBUTE}=\"{region}\" \
         {CAMERA_STATE_ATTRIBUTE}=\"idle\"></figure>\n"
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
    fn a_deck_is_served_in_the_language_it_declares() {
        // A translated deck served as `lang="en"` gets an English voice from
        // every screen reader and the wrong hyphenation from every browser.
        assert!(shell("---\nlang: ja\n---\n\n# はじめに\n").contains("<html lang=\"ja\""));
    }

    #[test]
    fn a_deck_that_declares_no_language_is_still_served_with_one() {
        // An empty `lang` is worse than a wrong one: it tells assistive
        // technology nothing at all rather than something it can correct for.
        assert!(shell("# One\n").contains("<html lang=\"en\""));
    }

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

    const CAMERA: &str = "---\nlayout: aside\ncamera: side\n---\n\n# Remote\n";

    #[test]
    fn a_declared_camera_puts_a_tile_in_the_region_the_author_named() {
        let html = shell(CAMERA);

        assert!(tile_in(&html).contains("data-slidx-camera=\"side\""), "wrong region:\n{html}");
    }

    #[test]
    fn a_page_with_a_camera_on_it_cannot_ask_for_one() {
        // The constraint the whole feature is shaped around. A built deck is a
        // static page anybody may open from a link, and a page that prompts for
        // a webcam is a page people close. The tile ships; the means of filling
        // it does not, and there is nothing in the document that could.
        let html = shell(CAMERA);

        for reach in ["getUserMedia", "mediaDevices", "<video"] {
            assert!(!html.contains(reach), "a published slide reaches for {reach}:\n{html}");
        }

        // Counted the way the staging test counts it: the structured data in
        // the head is a `<script>` element and is not code. `application/ld+json`
        // is the container the JSON-LD specification chose for a block of JSON,
        // and no browser executes it. Anything else here would be something that
        // runs, and something that runs is the only thing that could open a
        // camera.
        // Something that runs is the only thing that could open a camera, so
        // what runs is enumerated rather than counted at zero. The other one is
        // the navigator, which the assertions above have just been run against.
        assert_eq!(html.matches("<script").count(), 2, "something else is scripted:\n{html}");
        assert!(html.contains("<script type=\"application/ld+json\">"));
        assert!(html.contains(".slidx-slide-nav"), "and the second one is the navigator");
    }

    #[test]
    fn a_camera_tile_starts_in_the_state_a_published_page_never_leaves() {
        // `idle` is drawn as nothing at all. Without it the audience gets an
        // empty rectangle where a face was going to be, on every slide, forever
        // — which is worse than the feature being absent.
        assert!(shell(CAMERA).contains("data-slidx-camera-state=\"idle\""));
    }

    #[test]
    fn a_slide_with_no_camera_renders_no_camera_markup() {
        // Asserted on the element, not the attribute: the inlined stylesheet
        // names the attribute on every page whether or not a camera exists.
        assert!(!shell("# Ordinary\n").contains("<figure class=\"slidx-camera\""));
    }

    #[test]
    fn a_camera_naming_a_region_this_layout_lacks_lands_in_one_that_exists() {
        // A `grid-area` naming nothing puts the tile in an implicit track, which
        // pushes every region of the slide down a row. A stale `camera:` has to
        // cost a misplaced tile, not a rearranged slide.
        let html = shell("---\nlayout: split\ncamera: side\n---\n\n# One\n");
        let tile = tile_in(&html);

        assert!(tile.contains("data-slidx-camera=\"left\""), "got: {tile}");
        assert!(!tile.contains("side"), "the stale region survived: {tile}");
    }

    /// The camera tile as emitted, without the stylesheet that names every
    /// region's attribute on every page.
    fn tile_in(html: &str) -> String {
        let at = html.find("<figure class=\"slidx-camera\"").expect("no camera tile");
        let rest = &html[at..];

        rest[..rest.find('>').map_or(rest.len(), |end| end + 1)].to_string()
    }

    #[test]
    fn a_camera_tile_has_an_area_in_the_grid_it_is_a_child_of() {
        // The tile is a sibling of the regions rather than a child of one, so
        // the layout's own CSS is the only thing that can place it.
        let html = shell(CAMERA);

        assert!(html.contains(
            "[data-slidx-layout=\"aside\"] > .slidx-slide-body > [data-slidx-camera=\"side\"] { grid-area: side; }"
        ), "the tile has no area:\n{html}");
    }

    #[test]
    fn a_shell_is_a_complete_document() {
        let html = shell("# Hello\n");

        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<meta charset=\"utf-8\">"));
        assert!(html.trim_end().ends_with("</html>"));
    }

    #[test]
    fn a_static_preview_keeps_the_real_frame_and_removes_live_behaviour() {
        let source = concat!(
            "---\n",
            "layout: split\n",
            "transition: fade\n",
            "camera: left\n",
            "demo:\n",
            "  live: https://app.example.com\n",
            "  fallback: ./demo.mp4\n",
            "---\n\n",
            "# Left\n\n",
            "{.right}\n",
            "Beside it.\n\n",
            "- first <!-- step -->\n",
        );
        let deck = parse_deck(source, &DeckParseOptions::default());
        let html = render_static_preview(&deck, &deck.slides[0], &ShellOptions::default());

        assert!(html.contains("data-slidx-preview"));
        assert!(html.contains("data-slidx-layout=\"split\""));
        assert!(html.contains("data-slidx-region=\"right\""));
        assert!(html.contains("Beside it."));
        assert!(!html.contains("<script"));
        assert!(!html.contains("<figure class=\"slidx-demo\""));
        assert!(!html.contains("<figure class=\"slidx-camera\""));
        assert!(!html.contains("@view-transition"));
        assert!(!html.contains("https://app.example.com"));
    }

    /// A slide with two stops: one element whose text changes on the way.
    const STAGED: &str = "# Latency\n\nDropped to [120ms]{#latency}[38ms]{#latency}.\n";

    #[test]
    fn a_slide_with_one_stop_asks_the_network_for_nothing() {
        // This test used to say a slide with one stop carried nothing that
        // runs, and that was true because such a slide could not be advanced:
        // no key, no link, and no listener for the presenter's mirror. The
        // claim was a description of the defect.
        //
        // What survives the fix is the part that was ever worth having. A
        // finished slide still costs no request, no module graph and no
        // framework — see `crate::navigation` for the bargain and its size.
        //
        // The structured data in the head is a `<script>` element and is not
        // code: `application/ld+json` is the container the JSON-LD
        // specification chose for a block of JSON, and no browser executes it.
        let html = shell("# Hello\n\n- one\n");

        assert!(!html.contains("<script type=\"module\">"), "no module graph:\n{html}");
        assert!(!html.contains("<script src="), "no request:\n{html}");
        assert_eq!(html.matches("<script").count(), 2, "something else is scripted:\n{html}");
        assert!(html.contains("<script type=\"application/ld+json\">"));
    }

    #[test]
    fn what_a_finished_slide_pays_for_being_navigable_is_bounded() {
        // A number rather than an adjective, because "tiny" is what every
        // bundle says about itself. If this ever needs raising, the raise is
        // the review.
        let html = shell("# Hello\n\n- one\n");
        let script = html
            .split("<script>")
            .nth(1)
            .and_then(|rest| rest.split("</script>").next())
            .expect("the navigator");

        assert!(script.len() < 2400, "the navigator has grown to {} bytes", script.len());
    }

    #[test]
    fn a_slide_with_steps_loads_the_runtime() {
        // Without this the compiled timeline is unreachable: the stops exist,
        // the PDF has a page for each, the presenter view can walk them — and
        // the projector, which is the only screen the audience sees, is stuck
        // on the first one forever.
        let html = shell(STAGED);

        assert!(html.contains("<script type=\"module\">"), "no runtime on a staged slide:\n{html}");
        assert!(html.contains("createStage"));
        assert!(html.contains("./runtime.js"));
    }

    #[test]
    fn a_staged_slide_says_its_script_arrived_so_the_staging_css_takes_effect() {
        // Staging is gated on `data-slidx-js`, so that a deck whose script never
        // loaded shows everything rather than a mostly blank slide. Nothing on
        // an audience slide set it, which turned the gate inside out: every
        // element the pipeline had hidden was painted anyway. The stops still
        // advanced, the URL still tracked them, the PDF still printed one page
        // each — and the projector showed the whole slide from the first press.
        let html = shell(STAGED);

        assert!(html.contains("markScriptEnabled"), "the staging gate stays shut:\n{html}");
    }

    #[test]
    fn the_timeline_travels_with_the_document() {
        // Fetched rather than inlined would break the one case this has to
        // survive: a deck opened from a USB stick over `file://`, which is
        // where a speaker ends up when everything else has failed.
        let html = shell(STAGED);

        assert!(html.contains("\"frames\""), "the timeline is not in the page:\n{html}");
        assert!(!html.contains("fetch("));
    }

    #[test]
    fn the_runtime_url_is_the_one_the_caller_asked_for() {
        let deck = parse_deck(STAGED, &DeckParseOptions::default());
        let options =
            ShellOptions { runtime_src: "/assets/slidx.js".to_string(), ..ShellOptions::default() };

        let html = render_slide(&deck, &deck.slides[0], &options);

        assert!(html.contains("\"/assets/slidx.js\""));
        assert!(
            html.contains(
                "new URL(\"./effects.css\", new URL(\"/assets/slidx.js\", location.href))"
            ),
            "the effect stylesheet did not stay beside the caller's runtime:\n{html}"
        );
    }

    #[test]
    fn a_caller_can_refuse_the_runtime_entirely() {
        // The print shell renders every stop at once and drives them itself, so
        // it wants the markup without the wiring.
        let deck = parse_deck(STAGED, &DeckParseOptions::default());
        let options = ShellOptions { include_runtime: false, ..ShellOptions::default() };

        assert!(
            !render_slide(&deck, &deck.slides[0], &options).contains("<script type=\"module\">")
        );
    }

    #[test]
    fn the_shell_asks_the_network_for_nothing() {
        // The offline guarantee, at the smallest scale it can be checked.
        //
        // What counts is what a browser *fetches* while the slide is on screen,
        // which is the distinction the offline lint rule already turns on. A
        // canonical link, an `og:image` and a JSON-LD `@context` are addresses
        // the page states rather than requests it makes — whoever reads them is
        // a scraper on its own machine, not the browser in the room. The build
        // asserts the same thing from the other end, by counting the requests a
        // real browser makes on a real emitted page.
        let html = shell("# Hello\n\n- one\n");

        for marker in
            ["<link rel=\"stylesheet\"", "src=\"http", "//cdn", "@import url(", "url(http"]
        {
            assert!(!html.contains(marker), "shell reaches for {marker}:\n{html}");
        }
    }

    #[test]
    fn the_theme_is_compiled_once_however_many_slides_ask_for_it() {
        // Every slide inlines the same theme CSS — that is the offline
        // guarantee — but it was *rendered* per slide, and a deck is the one
        // place that loop is long. Measured at 500 slides: 37.3ms to 26.4ms,
        // with the emitted bytes identical.
        let options = ShellOptions::default();
        let first = options.theme_css().as_ptr();

        assert_eq!(options.theme_css().as_ptr(), first, "a second ask must not re-render");
    }

    #[test]
    fn changing_the_theme_does_not_leave_the_old_stylesheet_behind() {
        // The failure the cache could introduce, and the reason the field is
        // not something a caller assigns.
        let minimal = ShellOptions::default();
        let contrast = ShellOptions::default().with_theme(slidx_theme::builtin::contrast());

        assert_ne!(minimal.theme_css(), contrast.theme_css());
        assert!(contrast.theme_css().contains(&contrast.theme.light.surface.to_hex()));
    }

    #[test]
    fn the_theme_is_inlined_rather_than_linked() {
        let html = shell("# Hello\n");

        assert!(html.contains("--slidx-color-text:"));
        assert!(
            !html.contains("<link rel=\"stylesheet\""),
            "a stylesheet link is a network request"
        );
    }

    #[test]
    fn what_a_crawler_reads_is_in_the_head_of_the_page() {
        // The seam. `seo::head` composes all of it and this is the only thing
        // that puts it in a document — the same shape of gap that left the
        // compiled step pipeline unreachable for a release.
        let source = "---\ntitle: Fast Decks\ndraft: false\nurl: https://example.com/talk/\n---\n\n# One\n\nWhy decks should be pages.\n";
        let deck = parse_deck(source, &DeckParseOptions::default());
        let options = ShellOptions {
            seo: SeoOptions {
                deck_url: deck.meta.talk.url.clone(),
                cards: true,
                ..SeoOptions::default()
            },
            ..ShellOptions::default()
        };

        let html = render_slide(&deck, &deck.slides[0], &options);
        let head = html.split_once("</head>").expect("a closed head").0;

        for expected in [
            "<meta name=\"description\" content=\"Why decks should be pages.\">",
            "<link rel=\"canonical\" href=\"https://example.com/talk/\">",
            "<meta property=\"og:image\" content=\"https://example.com/talk/og-1.png\">",
            "<script type=\"application/ld+json\">",
        ] {
            assert!(head.contains(expected), "{expected} is missing from:\n{head}");
        }
    }

    #[test]
    fn a_slides_description_is_its_own_rather_than_the_decks() {
        // Forty pages sharing one description is forty search results that
        // cannot be told apart, and thirty-nine of them describe another page.
        let deck = parse_deck(
            "---\ntitle: T\ndescription: The whole talk.\n---\n\n# One\n\n---\n\n# Two\n\nWhat the second slide says.\n",
            &DeckParseOptions::default(),
        );

        let first = render_slide(&deck, &deck.slides[0], &ShellOptions::default());
        let second = render_slide(&deck, &deck.slides[1], &ShellOptions::default());

        assert!(first.contains("content=\"The whole talk.\""));
        assert!(second.contains("content=\"What the second slide says.\""));
        assert!(!second.contains("The whole talk."), "the deck's description was repeated");
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
    fn markdown_slide_styles_reach_the_slide_element_and_not_the_body() {
        let html = shell(concat!(
            "<style data-slidx>\n",
            ":root {\n",
            "  --slidx-color-surface: oklch(20% 0.02 260);\n",
            "  --slidx-layout: aside;\n",
            "}\n",
            "</style>\n",
            "\n",
            "# One\n",
        ));
        let slide = html
            .split_once("<article class=\"slidx-slide\"")
            .expect("a slide element")
            .1
            .split_once('>')
            .expect("a closed slide element")
            .0;

        assert!(slide.contains("data-slidx-layout=\"aside\""), "got: {slide}");
        assert!(slide.contains("--slidx-color-surface: oklch(20% 0.02 260);"), "got: {slide}");
        assert!(slide.contains("--slidx-layout: aside;"), "got: {slide}");
        assert!(!html.contains("<style data-slidx>"), "managed source reached the page");
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
        // Counted in the markup alone. The embedded timeline names the same
        // anchors as selectors, and a count over the whole document would be
        // measuring the wiring rather than the page.
        let deck =
            parse_deck("- one <!-- step -->\n- two <!-- step -->\n", &DeckParseOptions::default());
        let markup = render_slide(
            &deck,
            &deck.slides[0],
            &ShellOptions { include_runtime: false, ..ShellOptions::default() },
        );

        assert_eq!(markup.matches("data-slidx-step=").count(), 2);
    }

    #[test]
    fn marks_survive_into_the_page() {
        let html =
            shell("The answer is [42]{#count .accent color=accent font=mono size=heading-2}.\n");

        assert!(html.contains("data-slidx-mark=\"count\""));
        assert!(html.contains("slidx-accent"));
        assert!(html.contains("data-slidx-color=\"accent\""));
        assert!(html.contains("data-slidx-font=\"mono\""));
        assert!(html.contains("data-slidx-size=\"heading-2\""));
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
    fn code_on_a_slide_arrives_coloured_and_still_carries_no_highlighter() {
        // The whole reason highlighting happens at build time: the page an
        // audience sees is a string by then.
        let html = shell("```rust\nlet x = 1; // one\n```\n");

        assert!(html.contains("<span class=\"slidx-code-keyword\">let</span>"));
        assert!(html.contains("--slidx-color-code-comment:"));
        assert!(!html.contains("<script type=\"module\">"), "an audience slide runs nothing");
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
