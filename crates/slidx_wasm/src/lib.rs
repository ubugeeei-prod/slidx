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

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use wasm_bindgen::prelude::*;

use slidx_core::{parse_deck, DeckParseOptions};
use slidx_lint::{lint, LintInput, LintOptions, Measurement};
use slidx_render::{
    render_deck_card, render_presenter, render_print, render_slide, OgOptions, PresenterOptions,
    PrintOptions, ShellOptions,
};

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
    /// Module URL the presenter view imports the runtime from.
    pub runtime_src: Option<String>,
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

fn build(source: &str, options: &BuildOptions) -> BuildResult {
    let deck = parse_deck(source, &parse_options(options.separator.as_deref()));
    let theme = resolve_theme(options.theme.as_deref(), deck.meta.theme.as_deref());
    let surfaces = theme.surfaces();

    let mut diagnostics: Vec<Finding> = deck.diagnostics.iter().map(finding).collect();
    let has_blocking = deck.diagnostics.has_blocking();

    // The theme's padding is the safe area the shell enforces, and resolving
    // the theme is the only place that number exists. Without it the linter
    // cannot say whether a venue's caption strip reaches into content.
    let padding = theme.spacing.padding_px / theme.reference_height_px();
    let findings =
        lint(&LintInput::new(&deck, &surfaces).with_padding(padding), &LintOptions::default());
    diagnostics.extend(findings.iter().map(finding));

    let runtime_src = options.runtime_src.clone().unwrap_or_else(|| "./runtime.js".to_string());
    let shell = ShellOptions { theme: theme.clone(), ..ShellOptions::default() };
    let print_theme = theme.clone();
    let og_theme = theme.clone();
    let presenter =
        PresenterOptions { theme, runtime_src: runtime_src.clone(), ..PresenterOptions::default() };

    let render = !options.parse_only;
    let og = OgOptions { theme: og_theme, ..OgOptions::default() };

    let slides = deck
        .slides
        .iter()
        .map(|slide| BuiltSlide {
            id: slide.id.clone(),
            index: slide.index,
            title: slide.title.clone(),
            notes: slide.notes.clone(),
            stop_count: slide.timeline.len() as u32,
            frontmatter: slide.frontmatter.clone(),
            html: render.then(|| render_slide(&deck, slide, &shell)),
            og_svg: (render && options.og)
                .then(|| slidx_render::render_slide_card(&deck, slide, &og)),
            presenter_html: (render && options.presenter)
                .then(|| render_presenter(&deck, slide, &presenter)),
        })
        .collect();

    let print_html = (render && options.print).then(|| {
        render_print(
            &deck,
            &PrintOptions {
                theme: print_theme,
                inline_runtime: options.print_runtime.clone(),
                ..PrintOptions::default()
            },
        )
    });

    BuildResult {
        og_svg: (render && options.og).then(|| render_deck_card(&deck, &og)),
        title: deck.meta.title.clone(),
        description: deck.meta.description.clone(),
        slides,
        diagnostics,
        has_blocking,
        print_html,
    }
}

/// An explicit theme wins over the deck's own, which wins over the default.
fn resolve_theme(requested: Option<&str>, from_deck: Option<&str>) -> slidx_theme::Theme {
    requested
        .or(from_deck)
        .and_then(slidx_theme::resolve)
        .unwrap_or_else(slidx_theme::default_theme)
}

fn finding(diagnostic: &slidx_core::Diagnostic) -> Finding {
    Finding {
        severity: diagnostic.severity.as_token().to_string(),
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        help: diagnostic.help.clone(),
        slide_index: diagnostic.span.slide_index,
    }
}

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
        assert!(presenter.contains("out loud"));
        assert!(!result.slides[0].html.as_ref().unwrap().contains("out loud"));
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
    fn notes_are_carried_but_never_rendered_into_the_page() {
        let result =
            build("# One\n\n<!-- notes: say this out loud -->\n", &BuildOptions::default());

        assert_eq!(result.slides[0].notes, vec!["say this out loud".to_string()]);
        assert!(!result.slides[0].html.as_ref().unwrap().contains("say this out loud"));
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
