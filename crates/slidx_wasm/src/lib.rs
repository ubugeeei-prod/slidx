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

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use slidx_core::{parse_deck, DeckParseOptions};
use slidx_lint::{lint, LintInput, LintOptions};
use slidx_render::{render_slide, ShellOptions};

/// What a caller can ask for when building a deck.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BuildOptions {
    /// Theme name. Falls back to the deck's own `theme:`, then the default.
    pub theme: Option<String>,
    /// Separator for single-file decks.
    pub separator: Option<String>,
    /// Skip rendering and return only the model and diagnostics. The editor
    /// uses this while typing, where the outline matters and the HTML does not.
    pub parse_only: bool,
}

/// One built slide.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltSlide {
    pub id: String,
    pub index: u32,
    pub title: Option<String>,
    pub notes: Vec<String>,
    /// Stops on this slide, including the resting frame. Always at least one.
    pub stop_count: u32,
    /// The complete HTML page. Absent when `parseOnly` was set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
}

/// Everything a build or a preview needs from one call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildResult {
    pub title: Option<String>,
    pub description: Option<String>,
    pub slides: Vec<BuiltSlide>,
    /// Parse diagnostics and lint findings, in that order.
    pub diagnostics: Vec<Finding>,
    /// True when something in `diagnostics` should stop a build.
    pub has_blocking: bool,
}

/// A diagnostic, flattened for the JavaScript side.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub severity: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide_index: Option<u32>,
}

/// Parses, lints, and renders a deck in one call.
///
/// Never returns an error for bad *content*: a deck edited minutes before a
/// talk has to render something. Problems come back in `diagnostics`, and only
/// a malformed options object is an actual error.
#[wasm_bindgen(js_name = buildDeck)]
pub fn build_deck(source: &str, options: JsValue) -> Result<JsValue, JsError> {
    let options: BuildOptions = if options.is_undefined() || options.is_null() {
        BuildOptions::default()
    } else {
        serde_wasm_bindgen::from_value(options)
            .map_err(|error| JsError::new(&format!("invalid options: {error}")))?
    };

    let result = build(source, &options);
    serde_wasm_bindgen::to_value(&result).map_err(|error| JsError::new(&error.to_string()))
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

fn build(source: &str, options: &BuildOptions) -> BuildResult {
    let parse_options = DeckParseOptions {
        separator: options.separator.clone().unwrap_or_else(|| "---".to_string()),
        ..DeckParseOptions::default()
    };

    let deck = parse_deck(source, &parse_options);
    let theme = resolve_theme(options.theme.as_deref(), deck.meta.theme.as_deref());
    let surfaces = theme.surfaces();

    let mut diagnostics: Vec<Finding> = deck.diagnostics.iter().map(finding).collect();
    let has_blocking = deck.diagnostics.has_blocking();

    let findings = lint(&LintInput::new(&deck, &surfaces), &LintOptions::default());
    diagnostics.extend(findings.iter().map(finding));

    let shell = ShellOptions { theme, ..ShellOptions::default() };

    let slides = deck
        .slides
        .iter()
        .map(|slide| BuiltSlide {
            id: slide.id.clone(),
            index: slide.index,
            title: slide.title.clone(),
            notes: slide.notes.clone(),
            stop_count: slide.timeline.len() as u32,
            html: (!options.parse_only).then(|| render_slide(&deck, slide, &shell)),
        })
        .collect();

    BuildResult {
        title: deck.meta.title.clone(),
        description: deck.meta.description.clone(),
        slides,
        diagnostics,
        has_blocking,
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
