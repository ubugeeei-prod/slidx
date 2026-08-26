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
    render_deck_card, render_presenter, render_print, render_remote, render_robots, render_sitemap,
    render_slide, render_snippets, validate_mdx, MarkdownOptions, OgOptions, PresenterOptions,
    PrintOptions, RemoteOptions, SeoOptions, ShellOptions, SnippetOptions,
};
use slidx_theme::{Catalogue, Published, Resolved};

use crate::{
    parse_options, BuildOptions, BuildResult, BuiltSlide, Finding, LayoutChoice, SnippetFile,
    ThemeChoice, TransitionChoice,
};

pub(crate) fn build(source: &str, options: &BuildOptions) -> BuildResult {
    let deck = parse_deck(source, &parse_options(options.separator.as_deref()));

    let catalogue = catalogue(options);
    let resolved = resolve_theme(&catalogue, options.theme.as_deref(), deck.meta.theme.as_deref());
    let theme = resolved.theme.clone();
    let active_theme = theme.id.clone();
    let themes = slidx_theme::builtin::all()
        .into_iter()
        .chain(catalogue.installed().map(|(_, installed)| installed.clone()))
        .map(|choice| ThemeChoice::from(&choice))
        .collect();
    let surfaces = theme.surfaces();

    let mut diagnostics: Vec<Finding> = deck.diagnostics.iter().map(finding).collect();
    let markdown = MarkdownOptions { mdx: options.mdx, ..MarkdownOptions::default() };
    let mdx_findings: Vec<Finding> = if options.mdx {
        deck.slides
            .iter()
            .flat_map(|slide| {
                validate_mdx(&slide.content, &markdown).into_iter().map(move |issue| Finding {
                    severity: "error".to_string(),
                    code: issue.code.to_string(),
                    message: issue.message,
                    help: Some(issue.help.to_string()),
                    slide_index: Some(slide.index),
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    let mdx_has_blocking = !mdx_findings.is_empty();
    diagnostics.extend(mdx_findings);

    // What the packages themselves were found to be, before anything is said
    // about the deck: a document that is not a theme, a name a deck could not
    // write, a value that could have left the declaration it was written into.
    let packaging = catalogue.diagnostics().clone();
    diagnostics.extend(packaging.iter().map(finding));

    // And the linter's own verdict on the theme this deck actually renders
    // with. A package has no gate of its own — its author's CI is not this one
    // — so a theme that ships text nobody at the back can read is caught in the
    // build that uses it. Built-ins return nothing here: `slidx_theme` already
    // holds all four to these rules in every room it models.
    let theme_findings = resolved.audit(&LintOptions::default());
    diagnostics.extend(theme_findings.iter().map(finding));

    // The dialect check runs in the build as well as in `slidx lint` and in the
    // editor, because these are the findings a build is the last chance to say
    // anything about: a `steps:` entry addressing a mark that is not there
    // compiles, ships, and then does nothing when the presenter clicks.
    //
    // It is told what the project installed, or `theme: workshop` is a typo on
    // every build of the deck that installed `workshop`.
    //
    // Only the package ids. `Catalogue::names` is the picker's list and holds
    // the built-ins too, which the check already knows — passing it would put
    // every built-in twice into the help text of a genuine typo.
    let installed = slidx_dialect::Installed {
        themes: catalogue.installed().map(|(_, theme)| theme.id.clone()).collect(),
    };
    diagnostics.extend(slidx_dialect::check(&deck, &[], &installed).iter().map(finding));

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

    // The same measurements, for a second reason. A browser that does not know
    // an image's ratio reflows the slide when it lands, on the wifi a venue has
    // rather than the one it advertises — and every image here has already been
    // measured for the linter. See `slidx_render::intrinsic`.
    let drawn: slidx_render::intrinsic::Sizes = sizes
        .iter()
        .filter(|(_, intrinsic)| intrinsic.format != ImageFormat::Svg)
        .map(|(path, intrinsic)| (path.clone(), (intrinsic.width, intrinsic.height)))
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
    // the back row — reported that nothing blocked it. Placement and the theme
    // packages join the same list for the same reason: a source added after
    // this line is a source that does not count.
    let has_blocking = deck.diagnostics.has_blocking()
        || mdx_has_blocking
        || findings.has_blocking()
        || placement.has_blocking()
        || packaging.has_blocking()
        || theme_findings.has_blocking();

    let runtime_src = options.runtime_src.clone().unwrap_or_else(|| "./runtime.js".to_string());
    let camera_src = options.camera_src.clone().unwrap_or_else(|| "./camera.js".to_string());
    let media_src = options.media_src.clone().unwrap_or_else(|| "./media.js".to_string());
    let presenter_runtime_src =
        options.presenter_runtime_src.clone().unwrap_or_else(|| "./presenter.js".to_string());
    let rehearsal_src =
        options.rehearsal_src.clone().unwrap_or_else(|| "./rehearsal.js".to_string());
    let remote_src = options.remote_src.clone();

    // An address the caller states wins over the deck's own, for the same reason
    // an explicit theme does: the file describes the deck, and the build knows
    // which deployment of it this is.
    let seo = SeoOptions {
        deck_url: options.deck_url.clone().or_else(|| deck.meta.talk.url.clone()),
        deck_path: options.deck_path.clone().unwrap_or_else(|| "/".to_string()),
        cards: options.og,
        presenter: options.presenter,
    };

    // One `ShellOptions` for the whole deck, so the theme is compiled to CSS
    // once rather than once per slide — this is the loop that made that matter.
    let shell = ShellOptions {
        markdown,
        runtime_src: runtime_src.clone(),
        camera_src,
        media_src: media_src.clone(),
        seo: seo.clone(),
        asset_sizes: std::sync::Arc::new(drawn.clone()),
        ..ShellOptions::default()
    }
    .with_theme(theme.clone());
    let print_theme = theme.clone();
    let snippet_theme = theme.clone();
    let og_theme = theme.clone();
    let presenter = PresenterOptions {
        theme: theme.clone(),
        markdown,
        runtime_src: runtime_src.clone(),
        presenter_runtime_src,
        rehearsal_src,
        media_src,
        remote_src: remote_src.clone(),
    };

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
            budget_seconds: slide.budget_seconds,
            estimated_seconds: slide.estimated_seconds(),
            optional: slide.optional,
            style: slide.style.clone(),
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
                markdown,
                inline_runtime: options.print_runtime.clone(),
                asset_sizes: drawn,
                ..PrintOptions::default()
            },
        )
    });

    // Rendered whenever the deck is, and not behind a flag. It costs one page
    // per deck rather than one per slide, it is the only way to reach a slide
    // by sight rather than by counting, and a deck that shipped without it
    // would leave `/overview/` a 404 on exactly the decks long enough to want
    // one.
    let overview_html = render.then(|| slidx_render::overview::render_overview(&deck, &shell));

    let remote_html = (render && remote_src.is_some()).then(|| {
        render_remote(
            &deck,
            &RemoteOptions {
                theme: theme.clone(),
                remote_src: remote_src.unwrap_or_else(|| "./remote.js".to_string()),
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

    // Files rather than tags, and composed here for the same reason the snippet
    // pages are: this side of the boundary has no filesystem, and the decision
    // about what a crawler is told belongs with the deck rather than with
    // whichever caller happens to be writing the output.
    let sitemap = render.then(|| render_sitemap(&deck, &seo)).flatten();
    let robots = render.then(|| render_robots(&deck, &seo));

    BuildResult {
        og_svg: (render && options.og).then(|| render_deck_card(&deck, &og)),
        snippets,
        sitemap,
        robots,
        title: deck.meta.title.clone(),
        description: deck.meta.description.clone(),
        duration_seconds: deck.meta.duration_seconds,
        active_theme,
        theme_locked: options.theme.is_some(),
        themes,
        transitions: slidx_theme::transition::Transition::ALL
            .into_iter()
            .map(TransitionChoice::from)
            .collect(),
        layouts: slidx_theme::layout::all()
            .into_iter()
            .map(|layout| LayoutChoice {
                id: layout.id,
                summary: layout.summary,
                areas: layout.areas,
                columns: layout.columns,
                rows: layout.rows,
            })
            .collect(),
        slides,
        diagnostics,
        has_blocking,
        print_html,
        overview_html,
        remote_html,
    }
}

/// The themes this project can name, from the documents the caller found.
pub(crate) fn catalogue(options: &BuildOptions) -> Catalogue {
    let published: Vec<Published> = options
        .theme_packages
        .iter()
        .map(|package| Published::new(package.source.clone(), package.document.clone()))
        .collect();

    Catalogue::read(&published)
}

/// An explicit theme wins over the deck's own, which wins over the default.
///
/// Within either, a built-in wins over a package — that precedence is
/// [`Catalogue::resolve`]'s, and `slidx_theme::package` says why.
///
/// A name nothing answers to still falls back rather than failing, because a
/// deck edited minutes before a talk has to render something. What stops that
/// from being silent is `dialect/unknown-theme`, which reports the same name
/// this function could not resolve.
pub(crate) fn resolve_theme(
    catalogue: &Catalogue,
    requested: Option<&str>,
    from_deck: Option<&str>,
) -> Resolved {
    requested
        .or(from_deck)
        .and_then(|id| catalogue.resolve(id))
        .unwrap_or_else(|| Resolved { theme: slidx_theme::default_theme(), source: None })
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
