//! The deck pipeline, reachable from JavaScript.
//!
//! # Why WebAssembly rather than a native addon
//!
//! A native addon is faster and costs a platform matrix: a package per
//! target triple, optional dependencies, a postinstall, and a release that can
//! ship six artifacts and break on the seventh. slidx claims installation is
//! one command on every machine, and that claim does not survive a build that
//! has no prebuilt binary for the runtime someone is using.
//!
//! Parsing a deck is microseconds either way. Decks have dozens of slides, not
//! millions of nodes, so the speed a native addon buys is never the thing
//! standing between an author and their talk.
//!
//! The decisive reason is elsewhere: the same module runs in the browser. The
//! visual editor's live preview and the production build execute *the same
//! code*, so the canvas cannot disagree with the artifact. A native addon
//! would need a second implementation to run in the editor, and a second
//! implementation is a second set of answers.
//!
//! # Boundary
//!
//! Everything crossing into JavaScript is plain data, serialised through
//! serde. No handles, no lifetimes, nothing to free — a caller cannot leak or
//! double-free something it never holds.

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod declarations;
pub mod edit;
pub mod publish;
pub mod summary;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use wasm_bindgen::prelude::*;

use slidx_core::{parse_deck, DeckParseOptions};
use slidx_lint::{LintOptions, Measurement};

/// What a caller can ask for when building a deck.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct BuildOptions {
    /// Theme name. Falls back to the deck's own `theme:`, then the default.
    pub theme: Option<String>,
    /// Separator for single-file decks.
    pub separator: Option<String>,
    /// Skip rendering and return only the model and diagnostics. The editor
    /// uses this while typing, where the outline matters and the HTML does not.
    pub parse_only: bool,
    /// Also render the presenter view for each slide.
    ///
    /// Off by default: a deck that is only being built for the web does not
    /// need it, and it doubles the rendering work.
    pub presenter: bool,
    /// Also render the print shell — one document, one page per stop.
    pub print: bool,
    /// Also draw a social card per slide, and one for the deck.
    pub og: bool,
    /// Absolute URL of the deck's root, overriding the deck's own `url:`.
    ///
    /// A canonical link, an `og:url` and a sitemap entry are absolute by
    /// definition, and a build has no way to know the origin it will be deployed
    /// to. So the origin is something someone states: usually `url:` in the
    /// frontmatter, which is where authors already write it for the QR codes,
    /// and this when the deployment knows better than the file does — a preview
    /// build of the same deck is at a different address, and the file cannot say
    /// so without being edited per environment.
    ///
    /// Absent means nothing absolute is emitted at all. A guessed origin sends a
    /// search engine to a page that does not exist.
    pub deck_url: Option<String>,
    /// Where the deck is mounted in the site, root-relative. Defaults to `/`.
    ///
    /// Only `robots.txt` needs it: that file lives at the site root and has to
    /// name the deck from there, so it is the one artefact that cannot be
    /// written relative to the deck itself.
    pub deck_path: Option<String>,
    /// Module URL the presenter view imports the runtime from.
    pub runtime_src: Option<String>,
    /// Image sizes the caller already read, keyed by the path a slide writes.
    ///
    /// There is no filesystem on this side of the boundary, so the resolution
    /// rules cannot open `./logo.png` themselves. A caller that can — the Vite
    /// plugin — reads each header, passes it through [`probe_image`], and hands
    /// the answers back here. Absent means those rules stay silent, which is
    /// the editor mid-keystroke.
    pub assets: Vec<AssetSize>,
    /// The runtime's source, inlined into the print shell.
    ///
    /// The print shell is opened over `file://` — by the PDF exporter, from a
    /// USB stick, out of an email attachment — and a browser refuses to
    /// resolve a module import from a null origin whatever the path says. So
    /// that document carries its own script rather than referencing one.
    pub print_runtime: Option<String>,
}

/// One built slide.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BuiltSlide {
    pub id: String,
    pub index: u32,
    pub title: Option<String>,
    pub notes: Vec<String>,
    /// Stops on this slide, including the resting frame. Always at least one.
    pub stop_count: u32,
    /// This slide's steps as rows and stops, for the editor's timeline.
    ///
    /// Carried in the same answer as everything else rather than fetched on its
    /// own, because a grid drawn from a second call would be a second snapshot
    /// and could describe a deck the rest of the payload no longer agrees with.
    pub steps: slidx_core::StepGrid,
    /// The frontmatter keys the author wrote, whether or not slidx knows them.
    ///
    /// The editor's inspector shows these, so a key this version has never
    /// heard of is still visible rather than quietly lost. The first slide's
    /// block is the deck's, which is what the parser already believes.
    ///
    /// Declared by hand because it is genuinely open: whatever a deck's YAML
    /// held. A generated shape would be a promise about keys slidx does not
    /// define.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    #[ts(type = "Record<string, unknown>", optional)]
    pub frontmatter: serde_json::Value,
    /// The complete HTML page. Absent when `parseOnly` was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub html: Option<String>,
    /// This slide's social card, as SVG. Absent unless `og` was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub og_svg: Option<String>,
    /// The speaker's view of this slide. Absent unless `presenter` was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub presenter_html: Option<String>,
}

/// Everything a build or a preview needs from one call.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BuildResult {
    pub title: Option<String>,
    pub description: Option<String>,
    pub slides: Vec<BuiltSlide>,
    /// Parse diagnostics and lint findings, in that order.
    pub diagnostics: Vec<Finding>,
    /// True when something in `diagnostics` should stop a build.
    pub has_blocking: bool,
    /// The whole deck as one printable document. Absent unless `print` was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub print_html: Option<String>,
    /// The deck's own social card, as SVG. Absent unless `og` was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub og_svg: Option<String>,
    /// A page per shared code fence, for the caller to write.
    ///
    /// Composed here and written by whoever asked, because this side of the
    /// boundary has no filesystem. Empty when the deck shares nothing, which
    /// is most decks.
    pub snippets: Vec<SnippetFile>,
    /// `sitemap.xml` for the deck, for the caller to write beside the slides.
    ///
    /// Absent when nobody has said where the deck is deployed: `<loc>` is
    /// defined as a full URL, so a sitemap without an origin is an invalid file
    /// rather than a relative one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sitemap: Option<String>,
    /// `robots.txt` for the site the deck is deployed into.
    ///
    /// Every directive in it is root-relative, so unlike the sitemap it is
    /// always something. Absent only when nothing was rendered at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub robots: Option<String>,
}

/// One shared snippet, as a file waiting to be written.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SnippetFile {
    /// Relative to the deck's own output root, separators already `/`.
    pub path: String,
    pub html: String,
}

/// One image, as the caller measured it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AssetSize {
    /// As a slide writes it, minus any query or fragment.
    pub path: String,
    pub width: u32,
    pub height: u32,
    /// True for a format with no resolution to run out of, which is SVG.
    pub scalable: bool,
}

/// The intrinsic size in an image's header, or nothing.
///
/// Exposed so a caller with a filesystem can use the one header parser this
/// project has rather than writing a second. `None` for a format it does not
/// read and for a truncated header of one it does — both of which are silence
/// rather than a complaint, the same as everywhere else in the linter.
#[wasm_bindgen(js_name = probeImage)]
pub fn probe_image(bytes: &[u8]) -> Result<JsValue, JsError> {
    let probed = slidx_lint::probe_image(bytes).map(|intrinsic| AssetSize {
        path: String::new(),
        width: intrinsic.width,
        height: intrinsic.height,
        scalable: intrinsic.format.is_scalable(),
    });

    serde_wasm_bindgen::to_value(&probed).map_err(|error| JsError::new(&error.to_string()))
}

/// A diagnostic, flattened for the JavaScript side.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub severity: String,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub slide_index: Option<u32>,
}

/// Parses, lints, and renders a deck in one call.
///
/// Never returns an error for bad *content*: a deck edited minutes before a
/// talk has to render something. Problems come back in `diagnostics`, and only
/// a malformed options object is an actual error.
///
/// The options reach TypeScript as `BuildDeckOptions` rather than as
/// `BuildOptions`: the struct is `#[serde(default)]`, so every field may be
/// left out, and someone who wants one option should not have to restate the
/// other seven to get it.
#[wasm_bindgen(js_name = buildDeck, unchecked_return_type = "BuildResult")]
pub fn build_deck(
    source: &str,
    #[wasm_bindgen(unchecked_optional_param_type = "BuildDeckOptions")] options: JsValue,
) -> Result<JsValue, JsError> {
    let options: BuildOptions = if options.is_undefined() || options.is_null() {
        BuildOptions::default()
    } else {
        serde_wasm_bindgen::from_value(options)
            .map_err(|error| JsError::new(&format!("invalid options: {error}")))?
    };

    to_js(&build(source, &options))
}

/// Sends a value across as JSON-shaped data.
///
/// Maps become plain objects rather than `Map` instances. The default is the
/// other way round, which is right for a `HashMap` used as a lookup and wrong
/// for the thing this boundary actually carries: an author's frontmatter, which
/// is JSON on both sides and has to survive `JSON.stringify` on the way to a
/// browser.
pub(crate) fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);

    value.serialize(&serializer).map_err(|error| JsError::new(&error.to_string()))
}

/// Reports what a browser found when it laid the built pages out.
///
/// Separate from [`build_deck`] because the pages have to exist before anything
/// can open them, so the measurement necessarily arrives after the build that
/// already linted everything else. It returns only the findings a browser could
/// produce; the rest were reported once already.
#[wasm_bindgen(js_name = lintMeasured)]
pub fn lint_measured(
    source: &str,
    measured: JsValue,
    options: JsValue,
) -> Result<JsValue, JsError> {
    let options: BuildOptions = if options.is_undefined() || options.is_null() {
        BuildOptions::default()
    } else {
        serde_wasm_bindgen::from_value(options)
            .map_err(|error| JsError::new(&format!("invalid options: {error}")))?
    };

    let measured: Vec<Measurement> = serde_wasm_bindgen::from_value(measured)
        .map_err(|error| JsError::new(&format!("invalid measurements: {error}")))?;

    let parse_options = DeckParseOptions {
        separator: options.separator.clone().unwrap_or_else(|| "---".to_string()),
        ..DeckParseOptions::default()
    };
    let deck = parse_deck(source, &parse_options);

    let findings = slidx_lint::lint_measured(&deck, &measured, &LintOptions::default());
    let findings: Vec<Finding> = findings.iter().map(finding).collect();

    serde_wasm_bindgen::to_value(&findings).map_err(|error| JsError::new(&error.to_string()))
}

/// The CSS a theme resolves to, for callers that render their own shells.
#[wasm_bindgen(js_name = themeCss)]
pub fn theme_css(name: Option<String>) -> String {
    slidx_theme::css::render(&resolve_theme(name.as_deref(), None))
}

/// Theme names built in to slidx.
#[wasm_bindgen(js_name = themeNames)]
pub fn theme_names() -> Vec<String> {
    slidx_theme::builtin::all().into_iter().map(|theme| theme.id).collect()
}

/// The version of the pipeline this module was built from.
///
/// The plugin reports it on a mismatch: a cached wasm module paired with a
/// newer plugin produces confusing output, and the confusion is worth one
/// string.
#[wasm_bindgen(js_name = version)]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// How this module reads a deck source.
///
/// One place, so a build and an edit cannot disagree about where a slide ends
/// — which is what the editor's canvas and its writes depend on.
pub(crate) fn parse_options(separator: Option<&str>) -> DeckParseOptions {
    DeckParseOptions {
        separator: separator.unwrap_or("---").to_string(),
        ..DeckParseOptions::default()
    }
}

mod build;

use build::{build, finding, resolve_theme};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deck_builds_into_one_page_per_slide() {
        let result = build("# One\n\n---\n\n# Two\n", &BuildOptions::default());

        assert_eq!(result.slides.len(), 2);
        assert!(result.slides[0].html.as_ref().unwrap().starts_with("<!doctype html>"));
    }

    #[test]
    fn a_blocking_lint_finding_counts_as_blocking() {
        // It was read off the parse diagnostics alone, before the linter ran,
        // so a deck that breaks the zero-network guarantee reported that
        // nothing blocked it. Nothing consumed the field, which is the only
        // reason it never cost anyone a build that should have failed.
        let result =
            build("# One\n\n![a logo](https://example.com/logo.png)\n", &BuildOptions::default());

        assert!(
            result.diagnostics.iter().any(|finding| finding.severity == "error"),
            "expected the remote asset to be reported: {:?}",
            result.diagnostics
        );
        assert!(result.has_blocking);
    }

    #[test]
    fn a_deck_with_nothing_wrong_blocks_nothing() {
        assert!(!build("# One\n\n- one\n- two\n", &BuildOptions::default()).has_blocking);
    }

    #[test]
    fn the_print_shell_carries_its_own_runtime() {
        // A module import fails over `file://` whatever the path says — it is
        // a cross-origin request from a null origin — and the page then never
        // finishes expanding, silently.
        let options = BuildOptions {
            print: true,
            print_runtime: Some("const marker = 1;".to_string()),
            ..BuildOptions::default()
        };
        let html = build("# One\n", &options).print_html.unwrap();

        assert!(html.contains("const marker = 1;"));
        assert!(!html.contains("import {"));
    }

    #[test]
    fn social_cards_are_opt_in_and_cover_every_slide() {
        let options = BuildOptions { og: true, ..BuildOptions::default() };
        let result = build("# One\n\n---\n\n# Two\n", &options);

        assert!(result.og_svg.is_some(), "the deck gets its own card");
        assert!(result.slides.iter().all(|slide| slide.og_svg.is_some()));
        assert!(build("# One\n", &BuildOptions::default()).slides[0].og_svg.is_none());
    }

    #[test]
    fn the_print_shell_is_opt_in() {
        assert!(build("# One\n", &BuildOptions::default()).print_html.is_none());

        let options = BuildOptions { print: true, ..BuildOptions::default() };
        assert!(build("# One\n", &options).print_html.is_some());
    }

    #[test]
    fn the_print_shell_covers_the_whole_deck_in_one_document() {
        // One document rather than one per slide: a handout is a thing you
        // print once, and a browser prints one document at a time.
        let options = BuildOptions { print: true, ..BuildOptions::default() };
        let html = build("# One\n\n---\n\n# Two\n", &options).print_html.unwrap();

        assert_eq!(html.matches("class=\"slidx-page\"").count(), 2);
    }

    #[test]
    fn the_presenter_view_is_opt_in() {
        // It doubles the rendering work, and a deck built only for the web
        // never opens it.
        assert!(build("# One\n", &BuildOptions::default()).slides[0].presenter_html.is_none());

        let options = BuildOptions { presenter: true, ..BuildOptions::default() };
        assert!(build("# One\n", &options).slides[0].presenter_html.is_some());
    }

    #[test]
    fn the_presenter_view_carries_the_notes() {
        let options = BuildOptions { presenter: true, ..BuildOptions::default() };
        let result = build("# One\n\n<!-- notes: out loud -->\n", &options);

        let presenter = result.slides[0].presenter_html.as_ref().unwrap();
        let slide = result.slides[0].html.as_ref().unwrap();
        let body = slide.split_once("<body>").expect("a body").1;

        assert!(presenter.contains("out loud"));
        assert!(!body.contains("out loud"), "the notes reached the slide:\n{body}");
    }

    #[test]
    fn a_slide_carries_the_keys_the_author_wrote_even_the_unknown_ones() {
        // The inspector shows these. A tool that hides a key it does not
        // understand is a tool that eventually loses it.
        let result =
            build("---\ntitle: T\nsponsor: Someone\n---\n\n# One\n", &BuildOptions::default());

        assert_eq!(result.slides[0].frontmatter["sponsor"], "Someone");
        assert_eq!(result.slides[0].frontmatter["title"], "T");
    }

    #[test]
    fn parse_only_skips_the_html() {
        // The editor calls this on every keystroke; rendering four pages to
        // update an outline is work nobody asked for.
        let options = BuildOptions { parse_only: true, ..BuildOptions::default() };
        let result = build("# One\n", &options);

        assert!(result.slides[0].html.is_none());
        assert_eq!(result.slides[0].title.as_deref(), Some("One"));
    }

    #[test]
    fn broken_content_still_produces_a_deck() {
        // A deck edited minutes before a talk has to render something.
        let result = build("---\nnot: [valid\n---\n\n# Still here\n", &BuildOptions::default());

        assert_eq!(result.slides.len(), 1);
        assert!(result.slides[0].html.is_some());
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn lint_findings_come_back_with_the_parse_diagnostics() {
        let result =
            build("---\nduration: 1m\nbudget: 600s\n---\n\n# One\n", &BuildOptions::default());

        assert!(result.diagnostics.iter().any(|finding| finding.code.starts_with("budget/")));
    }

    #[test]
    fn dialect_findings_come_back_from_a_build_too() {
        // The build is the last chance anything has to say this: a `steps:`
        // entry addressing a mark that is not there compiles, ships, and then
        // does nothing when the presenter clicks.
        let result = build(
            "---\nsteps:\n  - reveal: \"#reuslt\"\n---\n\nThe [result]{#result}.\n",
            &BuildOptions::default(),
        );

        assert!(
            result.diagnostics.iter().any(|finding| finding.code == "dialect/unknown-target"),
            "{:?}",
            result.diagnostics
        );
        assert!(!result.has_blocking, "a step that will not play still ships a deck");
    }

    #[test]
    fn the_theme_padding_is_what_a_declared_caption_strip_is_checked_against() {
        // The safe area is not a second number: it is the padding the shell
        // already enforces. Resolving the theme is the only place that number
        // exists, so this is where the linter is told about it.
        let result =
            build("---\nsafeArea:\n  bottom: 15%\n---\n\n# One\n", &BuildOptions::default());

        assert!(
            result.diagnostics.iter().any(|finding| finding.code == "overflow/caption-strip"),
            "no caption-strip finding: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn a_deck_with_no_room_declared_is_not_told_about_its_padding() {
        let result = build("# One\n", &BuildOptions::default());
        assert!(result.diagnostics.iter().all(|finding| !finding.code.starts_with("overflow/")));
    }

    #[test]
    fn a_diagnostic_names_the_slide_it_is_about() {
        let result = build(
            "# One\n\n---\n\n---\nsteps:\n  - reveal: \"\"\n---\n\n# Two\n",
            &BuildOptions::default(),
        );

        let reported = result.diagnostics.iter().find(|finding| finding.slide_index == Some(1));
        assert!(reported.is_some(), "no diagnostic pointed at slide 2: {:?}", result.diagnostics);
    }

    #[test]
    fn the_requested_theme_overrides_the_decks_own() {
        let source = "---\ntheme: minimal\n---\n\n# One\n";
        let options = BuildOptions { theme: Some("terminal".into()), ..BuildOptions::default() };

        let with_override = build(source, &options);
        let without = build(source, &BuildOptions::default());

        assert_ne!(with_override.slides[0].html, without.slides[0].html);
    }

    #[test]
    fn an_unknown_theme_falls_back_rather_than_failing() {
        let options = BuildOptions { theme: Some("nope".into()), ..BuildOptions::default() };
        assert!(build("# One\n", &options).slides[0].html.is_some());
    }

    #[test]
    fn stop_counts_reach_the_caller() {
        // The plugin needs these to emit one PDF page per stop.
        let result = build("- a <!-- step -->\n- b <!-- step -->\n", &BuildOptions::default());
        assert_eq!(result.slides[0].stop_count, 3);
    }

    #[test]
    fn the_timeline_grid_reaches_the_caller_without_a_second_call() {
        // The editor draws rows and columns from this. A separate entry point
        // would give it a snapshot the rest of the answer could disagree with.
        let source = "---\nautoSteps: list\n---\n\n- one\n- two\n";
        let result = build(source, &BuildOptions { parse_only: true, ..BuildOptions::default() });
        let grid = &result.slides[0].steps;

        assert_eq!(grid.rows.len(), 2);
        assert_eq!(grid.actions.len(), 2);
        assert!(!grid.declared, "generated stops have no line to edit");
        assert_eq!(grid.auto, Some(slidx_core::AutoSteps::List));
    }

    #[test]
    fn notes_are_carried_but_never_shown_to_the_audience() {
        // Never on the slide, which is the screen the room is looking at. They
        // do describe the page in its head, where the deck's description comes
        // from — the author's prose about a slide is what a description wants,
        // and `slidx publish` already builds a blog draft out of the same words.
        let result =
            build("# One\n\n<!-- notes: say this out loud -->\n", &BuildOptions::default());
        let html = result.slides[0].html.as_ref().unwrap();
        let body = html.split_once("<body>").expect("a body").1;

        assert_eq!(result.slides[0].notes, vec!["say this out loud".to_string()]);
        assert!(!body.contains("say this out loud"), "the notes are on the slide:\n{body}");
        assert!(html.contains("<meta name=\"description\" content=\"say this out loud\">"));
    }

    #[test]
    fn a_built_deck_carries_the_files_a_crawler_asks_for() {
        // Composed here and written by the caller, like the snippet pages. The
        // one thing that cannot be checked on this side is whether anybody
        // writes them, which the plugin's own build test asserts.
        let source = "---\ndraft: false\nurl: https://example.com/talk/\n---\n\n# One\n\n---\n\n# Two\n";
        let result = build(source, &BuildOptions::default());

        let sitemap = result.sitemap.expect("a sitemap");
        assert!(sitemap.contains("<loc>https://example.com/talk/2/</loc>"), "{sitemap}");
        assert!(result.robots.expect("a robots.txt").contains("Sitemap: "));
    }

    #[test]
    fn a_deck_with_no_address_gets_no_sitemap_and_no_canonical() {
        // Both are absolute by definition. A guessed origin would point a search
        // engine at a page that does not exist.
        let result = build("---\ndraft: false\n---\n\n# One\n", &BuildOptions::default());

        assert!(result.sitemap.is_none());
        assert!(!result.slides[0].html.as_ref().unwrap().contains("canonical"));
        assert!(result.robots.is_some(), "robots.txt needs no origin");
    }

    #[test]
    fn the_caller_can_name_an_address_the_deck_file_does_not_know() {
        // A preview deployment of the same deck is at a different origin, and
        // the file cannot say so without being edited per environment.
        let options = BuildOptions {
            deck_url: Some("https://preview.example.com/pr-12/".into()),
            ..BuildOptions::default()
        };
        let source = "---\ndraft: false\nurl: https://example.com/talk/\n---\n\n# One\n";
        let result = build(source, &options);

        assert!(result.sitemap.unwrap().contains("https://preview.example.com/pr-12/"));
        assert!(result.slides[0]
            .html
            .as_ref()
            .unwrap()
            .contains("href=\"https://preview.example.com/pr-12/\""));
    }

    #[test]
    fn a_deck_that_never_said_it_was_public_is_kept_out_of_every_index() {
        // The judgement this feature turns on: a talk that leaks before the
        // conference announces it cannot be un-leaked.
        let options = BuildOptions { presenter: true, print: true, ..BuildOptions::default() };
        let result = build("---\nurl: https://example.com/talk/\n---\n\n# One\n", &options);

        assert!(result.slides[0].html.as_ref().unwrap().contains("content=\"noindex\""));
        assert!(!result.sitemap.unwrap().contains("<url>"));
        assert!(result.robots.unwrap().contains("Disallow: /"));
    }

    #[test]
    fn the_speakers_own_page_is_never_indexable_however_public_the_deck_is() {
        // It carries the notes. A published deck is one URL away from it.
        let options = BuildOptions { presenter: true, print: true, ..BuildOptions::default() };
        let result = build("---\ndraft: false\n---\n\n# One\n", &options);

        assert!(result.slides[0].presenter_html.as_ref().unwrap().contains("content=\"noindex\""));
        assert!(result.print_html.as_ref().unwrap().contains("content=\"noindex\""));
        assert!(!result.slides[0].html.as_ref().unwrap().contains("noindex"));
    }

    #[test]
    fn every_built_in_theme_is_named() {
        assert!(theme_names().len() >= 4);
        assert!(theme_names().contains(&"minimal".to_string()));
    }

    #[test]
    fn the_version_matches_the_crate() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
