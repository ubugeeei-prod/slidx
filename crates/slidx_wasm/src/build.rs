//! Turning a deck into everything one call can produce.
//!
//! Split from the boundary it is reached through because they answer different
//! questions. [`crate`] is *what crosses* — the shapes serde writes and `ts-rs`
//! describes, which every JavaScript caller compiles against. This is *how a
//! deck becomes them*, which is where the ordering, the flags, and the reasons
//! one thing is rendered and another is not actually live.

use std::collections::BTreeMap;

use slidx_core::parse_deck;
use slidx_lint::{lint, ImageFormat, Intrinsic, LintInput, LintOptions};
use slidx_render::{
    render_deck_card, render_presenter, render_print, render_slide, render_snippets, OgOptions,
    PresenterOptions, PrintOptions, ShellOptions, SnippetOptions,
};

use crate::{parse_options, BuildOptions, BuildResult, BuiltSlide, Finding, SnippetFile};

pub(crate) fn build(source: &str, options: &BuildOptions) -> BuildResult {
    let deck = parse_deck(source, &parse_options(options.separator.as_deref()));
    let theme = resolve_theme(options.theme.as_deref(), deck.meta.theme.as_deref());
    let surfaces = theme.surfaces();

    let mut diagnostics: Vec<Finding> = deck.diagnostics.iter().map(finding).collect();

    // The dialect check runs in the build as well as in `slidx lint` and in the
    // editor, because these are the findings a build is the last chance to say
    // anything about: a `steps:` entry addressing a mark that is not there
    // compiles, ships, and then does nothing when the presenter clicks.
    diagnostics.extend(slidx_dialect::check(&deck, &[]).iter().map(finding));

    // The theme's padding is the safe area the shell enforces, and resolving
    // the theme is the only place that number exists. Without it the linter
    // cannot say whether a venue's caption strip reaches into content.
    let padding = theme.spacing.padding_px / theme.reference_height_px();

    // Keyed the way a slide writes a path, so a rule can look one up without a
    // filesystem — which is the whole reason the caller measured them. Empty
    // when nobody did, and empty means those rules stay silent rather than
    // guess, the same as every other reading the linter cannot take.
    let sizes: BTreeMap<String, Intrinsic> = options
        .assets
        .iter()
        .map(|asset| {
            let format = if asset.scalable { ImageFormat::Svg } else { ImageFormat::Png };
            (asset.path.clone(), Intrinsic { format, width: asset.width, height: asset.height })
        })
        .collect();

    let input = LintInput::new(&deck, &surfaces).with_padding(padding);
    let input = if sizes.is_empty() { input } else { input.with_asset_sizes(&sizes) };

    let findings = lint(&input, &LintOptions::default());
    diagnostics.extend(findings.iter().map(finding));

    // Placement is checked here rather than inside the linter because it needs
    // the resolved theme: which regions exist is the layout's answer, and the
    // linter deliberately knows nothing about themes.
    let placement = slidx_theme::layout::diagnose(&deck);
    diagnostics.extend(placement.iter().map(finding));

    // Every source, and after all of them have been collected. This was read
    // off the parse diagnostics alone, above the lint call, so a deck whose
    // only blocking problem was a rule — a remote asset, text unreadable from
    // the back row — reported that nothing blocked it. Placement joins the same
    // list for the same reason: a source added after this line is a source that
    // does not count.
    let has_blocking =
        deck.diagnostics.has_blocking() || findings.has_blocking() || placement.has_blocking();

    let runtime_src = options.runtime_src.clone().unwrap_or_else(|| "./runtime.js".to_string());
    let shell = ShellOptions {
        theme: theme.clone(),
        runtime_src: runtime_src.clone(),
        ..ShellOptions::default()
    };
    let print_theme = theme.clone();
    let snippet_theme = theme.clone();
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
            steps: slidx_core::step_grid(slide),
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

    // Rendered whenever the deck is rendered rather than behind a flag: a
    // slide that asks for a snippet already shows a QR pointing at its page,
    // and a code on a projector that resolves to nothing is worse than no code
    // at all.
    let snippets = if render {
        render_snippets(&deck, &SnippetOptions { theme: snippet_theme })
            .into_iter()
            .map(|page| SnippetFile { path: page.path, html: page.html })
            .collect()
    } else {
        Vec::new()
    };

    BuildResult {
        og_svg: (render && options.og).then(|| render_deck_card(&deck, &og)),
        snippets,
        title: deck.meta.title.clone(),
        description: deck.meta.description.clone(),
        slides,
        diagnostics,
        has_blocking,
        print_html,
    }
}

/// An explicit theme wins over the deck's own, which wins over the default.
pub(crate) fn resolve_theme(
    requested: Option<&str>,
    from_deck: Option<&str>,
) -> slidx_theme::Theme {
    requested
        .or(from_deck)
        .and_then(slidx_theme::resolve)
        .unwrap_or_else(slidx_theme::default_theme)
}

pub(crate) fn finding(diagnostic: &slidx_core::Diagnostic) -> Finding {
    Finding {
        severity: diagnostic.severity.as_token().to_string(),
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        help: diagnostic.help.clone(),
        slide_index: diagnostic.span.slide_index,
    }
}
