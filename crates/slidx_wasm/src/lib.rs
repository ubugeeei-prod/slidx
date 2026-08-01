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

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use wasm_bindgen::prelude::*;

use slidx_core::{parse_deck, DeckParseOptions};
use slidx_lint::{LintOptions, Measurement};
use slidx_theme::{transition::Transition, Palette, Theme};

/// What a caller can ask for when building a deck.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct BuildOptions {
    /// Theme name. Falls back to the deck's own `theme:`, then the default.
    pub theme: Option<String>,
    /// Theme documents the caller found in the project's dependencies.
    ///
    /// There is no filesystem on this side of the boundary and no module
    /// resolver either, the same constraint that makes image sizes arrive
    /// pre-read. A caller that has both — the Vite plugin — finds the packages
    /// and hands the text over; what a theme document is allowed to say is
    /// decided here, by `slidx_theme::package`, so the editor's preview and the
    /// production build harden and audit the same bytes.
    pub theme_packages: Vec<ThemePackage>,
    /// Separator for single-file decks.
    pub separator: Option<String>,
    /// Enable MDX component syntax while rendering.
    ///
    /// Off by default. Components compile to static-first island markers and
    /// props must be JSON values; no JavaScript is evaluated by the compiler.
    pub mdx: bool,
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
    /// Module URL the presenter view imports rehearsal recording from.
    pub rehearsal_src: Option<String>,
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
    /// Seconds the author budgeted this slide, from `budget:`.
    ///
    /// Resolved here rather than left as the text a slide wrote, because
    /// `budget:` accepts `90`, `90s`, `1m30s` and `1:30`. A caller that drew a
    /// width from the text would be the project's second duration parser, and
    /// the one that disagreed with the linter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub budget_seconds: Option<u32>,
    /// Roughly how long this slide's notes take to say aloud.
    ///
    /// The only number available before a rehearsal exists for a slide with no
    /// budget, which is most slides while a talk is being written. An estimate
    /// rather than a measurement, and the same one the linter reasons about.
    pub estimated_seconds: u32,
    /// Safe to skip when running behind, from `optional:`.
    pub optional: bool,
    /// Visual state authored in this slide's tagged Markdown style block.
    ///
    /// Kept separate from frontmatter because visual controls write
    /// `--slidx-*` declarations without replacing an author's YAML. The map is
    /// complete even when empty, so an editor never has to infer whether the
    /// style block was parsed.
    pub style: BTreeMap<String, String>,
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
    /// Length of the speaking slot, from `duration:`.
    ///
    /// What the per-slide budgets are laid against. Absent for a deck whose
    /// author never had a slot, and absent means nothing can be said about
    /// whether the talk fits — which is silence rather than a guess.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub duration_seconds: Option<u32>,
    /// The theme that actually rendered this build.
    ///
    /// Separate from the authored `theme:` value because build configuration
    /// may deliberately override the file. The editor uses this to mark the
    /// card that agrees with the canvas rather than guessing from frontmatter.
    pub active_theme: String,
    /// Whether build configuration, rather than this deck, chose the theme.
    ///
    /// A locked picker explains why a source edit would not change the canvas
    /// instead of offering a control whose result is immediately overridden.
    pub theme_locked: bool,
    /// Themes the active pipeline can render, in picker order.
    ///
    /// Built-ins come first, followed by installed packages after they have
    /// been hardened and audited by the same catalogue the renderer uses.
    pub themes: Vec<ThemeChoice>,
    /// Slide transitions the active renderer implements, in picker order.
    ///
    /// The visual editor does not carry a second list: adding a renderer
    /// transition makes it appear here in the same build.
    pub transitions: Vec<TransitionChoice>,
    /// Layouts the active pipeline can render, in picker order.
    ///
    /// The editor draws this list instead of carrying built-in names. A layout
    /// added to the pipeline therefore appears in the visual editor without an
    /// editor release.
    pub layouts: Vec<LayoutChoice>,
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

/// One layout as the visual editor needs to preview it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutChoice {
    /// What the Markdown style property stores.
    pub id: String,
    /// One line explaining when the layout is useful.
    pub summary: String,
    /// CSS grid-area rows, without quotes.
    pub areas: Vec<String>,
    /// CSS grid-template-columns.
    pub columns: String,
    /// CSS grid-template-rows.
    pub rows: String,
}

/// One transition as the visual editor needs to explain and choose it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TransitionChoice {
    /// What `transition:` stores.
    pub id: String,
    pub name: String,
    pub description: String,
    /// Whether the full slide translates, for an honest motion warning.
    pub moves: bool,
}

impl From<Transition> for TransitionChoice {
    fn from(transition: Transition) -> Self {
        Self {
            id: transition.as_token().to_string(),
            name: transition.name().to_string(),
            description: transition.description().to_string(),
            moves: transition.moves(),
        }
    }
}

/// One theme as the visual editor needs to choose it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ThemeChoice {
    /// What `theme:` stores.
    pub id: String,
    pub name: String,
    /// One line explaining which kind of talk the theme serves.
    pub description: String,
    pub light: ThemePaletteChoice,
    pub dark: ThemePaletteChoice,
    /// The actual stacks a slide uses, so the miniature previews typography as
    /// well as colour without the editor restating a font decision.
    pub font_sans: String,
    pub font_mono: String,
}

impl From<&Theme> for ThemeChoice {
    fn from(theme: &Theme) -> Self {
        Self {
            id: theme.id.clone(),
            name: theme.name.clone(),
            description: theme.description.clone(),
            light: (&theme.light).into(),
            dark: (&theme.dark).into(),
            font_sans: theme.font_sans.clone(),
            font_mono: theme.font_mono.clone(),
        }
    }
}

/// The roles a theme card draws, already converted to browser-ready colours.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ThemePaletteChoice {
    pub surface: String,
    pub text: String,
    pub muted: String,
    pub heading: String,
    pub accent: String,
    pub code_surface: String,
    pub code_text: String,
}

impl From<&Palette> for ThemePaletteChoice {
    fn from(palette: &Palette) -> Self {
        Self {
            surface: palette.surface.to_hex(),
            text: palette.text.to_hex(),
            muted: palette.muted.to_hex(),
            heading: palette.heading.to_hex(),
            accent: palette.accent.to_hex(),
            code_surface: palette.code_surface.to_hex(),
            code_text: palette.code_text.to_hex(),
        }
    }
}

/// One shared snippet, as a file waiting to be written.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SnippetFile {
    /// Relative to the deck's own output root, separators already `/`.
    pub path: String,
    pub html: String,
}

/// One theme package, as the caller read it off disk.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ThemePackage {
    /// The package name, for a finding to point at.
    ///
    /// Every diagnostic about a theme has to name something outside the deck,
    /// because an author reading one is looking at their own slides and the
    /// answer is not in them.
    pub source: String,
    /// The document's text, exactly as the file holds it.
    pub document: String,
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
///
/// Built-ins only. A package theme's CSS comes out of a build, which is where
/// the document it was hardened from arrived — this is for a caller that has a
/// name and nothing else.
#[wasm_bindgen(js_name = themeCss)]
pub fn theme_css(name: Option<String>) -> String {
    let theme =
        name.as_deref().and_then(slidx_theme::resolve).unwrap_or_else(slidx_theme::default_theme);

    slidx_theme::css::render(&theme)
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

use build::{build, finding};

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
    fn a_slide_carries_visual_state_from_its_markdown_style_block() {
        let result = build(
            concat!(
                "<style data-slidx>\n",
                ":root {\n",
                "  --slidx-layout: aside;\n",
                "  --slidx-color-surface: oklch(20% 0.02 260);\n",
                "}\n",
                "</style>\n\n",
                "# One\n",
            ),
            &BuildOptions::default(),
        );

        assert_eq!(result.slides[0].style["layout"], "aside");
        assert_eq!(result.slides[0].style["color-surface"], "oklch(20% 0.02 260)");
    }

    #[test]
    fn the_editor_receives_layouts_from_the_rendering_pipeline() {
        let result = build("# One\n", &BuildOptions::default());
        let offered: Vec<_> = result.layouts.iter().map(|layout| layout.id.as_str()).collect();

        assert_eq!(offered, slidx_theme::layout::names());
        assert!(result.layouts.iter().all(|layout| !layout.summary.is_empty()
            && !layout.areas.is_empty()
            && !layout.columns.is_empty()
            && !layout.rows.is_empty()));
    }

    #[test]
    fn the_editor_receives_every_transition_from_the_renderer() {
        let result = build("# One\n", &BuildOptions::default());
        let offered: Vec<_> = result.transitions.iter().map(|choice| choice.id.as_str()).collect();

        assert_eq!(
            offered,
            Transition::ALL.iter().map(|transition| transition.as_token()).collect::<Vec<_>>()
        );
        assert!(result
            .transitions
            .iter()
            .all(|choice| !choice.name.is_empty() && !choice.description.is_empty()));
        assert_eq!(result.transitions.iter().filter(|choice| choice.moves).count(), 2);
    }

    #[test]
    fn the_editor_receives_the_rendered_theme_and_every_audited_choice() {
        let result = build("---\ntheme: editorial\n---\n\n# One\n", &BuildOptions::default());
        let offered: Vec<_> = result.themes.iter().map(|theme| theme.id.as_str()).collect();

        assert_eq!(result.active_theme, "editorial");
        assert!(!result.theme_locked);
        assert_eq!(
            offered,
            slidx_theme::builtin::all().iter().map(|theme| theme.id.as_str()).collect::<Vec<_>>()
        );
        assert!(result.themes.iter().all(|theme| {
            !theme.name.is_empty()
                && !theme.description.is_empty()
                && !theme.light.surface.is_empty()
                && !theme.dark.surface.is_empty()
                && !theme.font_sans.is_empty()
                && !theme.font_mono.is_empty()
        }));
    }

    #[test]
    fn a_build_theme_override_is_visible_to_the_editor_as_locked() {
        let options = BuildOptions { theme: Some("terminal".into()), ..BuildOptions::default() };
        let result = build("---\ntheme: editorial\n---\n\n# One\n", &options);

        assert_eq!(result.active_theme, "terminal");
        assert!(result.theme_locked);
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

    /// A deck built with the theme package this repository publishes installed.
    fn with_workshop(source: &str) -> BuildResult {
        let options = BuildOptions {
            theme_packages: vec![ThemePackage {
                source: slidx_theme::published::PACKAGE.to_string(),
                document: slidx_theme::published::document(),
            }],
            ..BuildOptions::default()
        };

        build(source, &options)
    }

    #[test]
    fn a_deck_naming_an_installed_theme_package_is_rendered_with_it() {
        // The whole path, end to end: a document read off disk by the caller,
        // hardened here, and a page that is demonstrably not the default.
        let built = with_workshop("---\ntheme: workshop\n---\n\n# One\n");
        let default = build("# One\n", &BuildOptions::default());

        let html = built.slides[0].html.as_ref().unwrap();
        assert!(
            html.contains(&slidx_theme::published::workshop().light.accent.to_hex()),
            "the package's own accent is not in the page"
        );
        assert_eq!(built.themes.last().map(|theme| theme.id.as_str()), Some("workshop"));
        assert_ne!(html, default.slides[0].html.as_ref().unwrap());
    }

    #[test]
    fn naming_an_installed_theme_is_not_reported_as_a_typo() {
        // The failure this would otherwise have: `dialect/unknown-theme` on
        // every build of a deck that installed exactly the theme it named.
        let built = with_workshop("---\ntheme: workshop\n---\n\n# One\n");

        assert!(
            built.diagnostics.iter().all(|finding| finding.code != "dialect/unknown-theme"),
            "{:?}",
            built.diagnostics
        );
        assert!(!built.has_blocking);
    }

    #[test]
    fn naming_a_theme_package_that_is_not_installed_is_still_reported() {
        // The other direction of the same rule. A package disappearing must not
        // quietly hand the deck something else instead.
        let built = build("---\ntheme: workshop\n---\n\n# One\n", &BuildOptions::default());

        assert!(
            built.diagnostics.iter().any(|finding| finding.code == "dialect/unknown-theme"),
            "{:?}",
            built.diagnostics
        );
    }

    #[test]
    fn a_theme_package_cannot_take_over_a_built_in_name() {
        // A dependency that could claim `minimal` could repaint every deck in a
        // repository without changing a line of any of them.
        let mut impostor = slidx_theme::published::workshop();
        impostor.id = "minimal".into();

        let options = BuildOptions {
            theme_packages: vec![ThemePackage {
                source: "@evil/theme".into(),
                document: serde_json::to_string(&impostor).unwrap(),
            }],
            ..BuildOptions::default()
        };

        let built = build("---\ntheme: minimal\n---\n\n# One\n", &options);
        let untouched = build("---\ntheme: minimal\n---\n\n# One\n", &BuildOptions::default());

        assert_eq!(built.slides[0].html, untouched.slides[0].html);
        assert!(built.has_blocking, "the attempt is reported rather than ignored");
    }

    #[test]
    fn a_theme_package_that_tries_to_write_script_into_a_page_stops_the_build() {
        let mut hostile = slidx_theme::published::workshop();
        hostile.font_sans = "sans-serif</style><script>fetch('//x')</script>".into();

        let options = BuildOptions {
            theme_packages: vec![ThemePackage {
                source: "@evil/theme-workshop".into(),
                document: serde_json::to_string(&hostile).unwrap(),
            }],
            ..BuildOptions::default()
        };

        let built = build("---\ntheme: workshop\n---\n\n# One\n", &options);

        assert!(built.has_blocking);
        assert!(!built.slides[0].html.as_ref().unwrap().contains("<script>fetch"));
    }

    #[test]
    fn a_package_theme_that_ships_illegible_text_fails_the_deck_that_uses_it() {
        // A package has no gate of its own, so the build that renders with it is
        // where the linter's verdict has to land.
        let mut illegible = slidx_theme::published::workshop();
        illegible.light.text = illegible.light.surface;
        illegible.dark.text = illegible.dark.surface;

        let options = BuildOptions {
            theme_packages: vec![ThemePackage {
                source: "@example/theme-murk".into(),
                document: serde_json::to_string(&illegible).unwrap(),
            }],
            ..BuildOptions::default()
        };

        let built = build("---\ntheme: workshop\n---\n\n# One\n", &options);
        let contrast =
            built.diagnostics.iter().find(|finding| finding.code.starts_with("contrast/"));

        assert!(contrast.is_some(), "{:?}", built.diagnostics);
        assert!(contrast.unwrap().message.contains("@example/theme-murk"));
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
    fn the_seconds_a_slide_is_budgeted_reach_the_caller() {
        // `budget: 90s` is one of four notations, and the storyboard draws a
        // width from the number. Sending the text and letting a browser read it
        // would be a second duration parser.
        let result = build("---\nbudget: 90s\n---\n\n# One\n", &BuildOptions::default());

        assert_eq!(result.slides[0].budget_seconds, Some(90));
    }

    #[test]
    fn a_slide_carries_the_spoken_length_of_its_own_notes() {
        // The pacing model — words a minute for Latin, characters a minute for
        // CJK — belongs to one implementation. A second one in the editor would
        // disagree with the linter about whether a talk fits.
        let result = build(
            "# One\n\n<!-- notes: two and a half words per second is the figure -->\n",
            &BuildOptions::default(),
        );

        assert_eq!(result.slides[0].estimated_seconds, 4);
    }

    #[test]
    fn a_slide_the_speaker_can_drop_says_so() {
        let result =
            build("---\noptional: true\n---\n\n# One\n\n---\n\n# Two\n", &BuildOptions::default());

        assert!(result.slides[0].optional);
        assert!(!result.slides[1].optional);
    }

    #[test]
    fn the_slot_the_talk_was_given_reaches_the_caller() {
        let result = build("---\nduration: 20m\n---\n\n# One\n", &BuildOptions::default());

        assert_eq!(result.duration_seconds, Some(1200));
        assert_eq!(build("# One\n", &BuildOptions::default()).duration_seconds, None);
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
        let source =
            "---\ndraft: false\nurl: https://example.com/talk/\n---\n\n# One\n\n---\n\n# Two\n";
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
    fn mdx_is_opt_in_and_reaches_every_rendered_surface() {
        let source = "# Intro\n\n---\n\n# Sign-ups\n\n<Counter start={128} label=\"people\">\n\n**128 people**\n\n</Counter>\n";
        let ordinary = build(source, &BuildOptions::default());
        assert!(!ordinary.slides[1].html.as_ref().unwrap().contains("data-slidx-island"));

        let options =
            BuildOptions { mdx: true, presenter: true, print: true, ..BuildOptions::default() };
        let result = build(source, &options);

        for html in [result.slides[1].html.as_ref().unwrap(), result.print_html.as_ref().unwrap()] {
            assert!(html.contains("data-slidx-island=\"Counter\""), "{html}");
            assert!(html.contains("<strong>128 people</strong>"), "{html}");
        }

        // The presenter carries the same compiled fallback inside an escaped,
        // sandboxed `srcdoc`: the geometry survives but no island can mount.
        let presenter = result.slides[0].presenter_html.as_ref().unwrap();
        assert!(presenter.contains("data-slidx-island=&quot;Counter&quot;"), "{presenter}");
        assert!(presenter.contains("&lt;strong&gt;128 people&lt;/strong&gt;"), "{presenter}");
        assert!(presenter.contains("tabindex=\"-1\" sandbox"), "{presenter}");
        assert!(!result.has_blocking);
    }

    #[test]
    fn executable_mdx_props_are_blocking_and_never_mount() {
        let options = BuildOptions { mdx: true, ..BuildOptions::default() };
        let result = build("<Counter value={window.secret}>static</Counter>\n", &options);
        let html = result.slides[0].html.as_ref().unwrap();

        assert!(result.has_blocking);
        assert!(result.diagnostics.iter().any(|finding| {
            finding.severity == "error" && finding.code == "mdx/non-static-props"
        }));
        assert!(html.contains("data-slidx-mdx-error"));
        assert!(!html.contains("data-slidx-island=\"Counter\""));
        assert!(html.contains("static"));
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
