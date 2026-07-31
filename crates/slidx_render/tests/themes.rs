//! Every built-in theme, in every layout, in both colour schemes.
//!
//! Issue #3 asked for "visual regression snapshots for each theme × layout ×
//! light/dark" and nothing held it: a change to a theme was reviewed by reading
//! its tokens and hoping, which is exactly the review a design system cannot
//! survive. Four themes and six layouts is twenty-four slides, and nobody opens
//! twenty-four slides to check a colour.
//!
//! # Why the markup and not a screenshot
//!
//! Because a screenshot is a photograph of a font renderer. The same slide is a
//! different image on macOS and on Linux — different hinting, different
//! subpixel geometry — so a pixel snapshot either fails on the machine that did
//! not take it or is compared so loosely that it stops seeing anything. This
//! repository learned the platform-dependent-gate lesson expensively enough
//! today already.
//!
//! What actually decides how a slide looks here is a token and a grid. A theme
//! is a token document; a layout is one `grid-template-areas`. Both are text,
//! both are what the browser is handed, and a diff of them is a diff a person
//! can read — which a pixel diff is not.
//!
//! So the guarantee is narrower than a photograph and much sharper: **nothing
//! about what the browser is told changes without somebody seeing the change.**
//! Whether a browser then draws it correctly is what
//! `packages/vite-plugin/test/browser.test.ts` is for, in three engines.

use slidx_core::{parse_deck, DeckParseOptions};
use slidx_render::shell::{render_slide, ShellOptions};

/// A slide that reaches every part of a theme.
///
/// A heading for the display scale, prose for the body, a list for the rhythm,
/// a marked phrase for the accent, inline code and a fence for the code
/// palette, and a block placed into a region so the layout has two things to
/// arrange rather than one.
const SLIDE: &str = "\
# The heading, at display size

Prose of the length prose usually is, with `inline code` and a
[marked phrase]{#accent .accent} in it.

- The first point
- The second

{.side}
```rust
fn measured(bytes: usize) -> usize { bytes }
```
";

fn slide_for(theme: &str, layout: &str) -> String {
    let source = format!("---\ntheme: {theme}\nlayout: {layout}\n---\n\n{SLIDE}");
    let deck = parse_deck(&source, &DeckParseOptions::default());
    let resolved = slidx_theme::resolve(theme).expect("a built-in theme");

    render_slide(
        &deck,
        &deck.slides[0],
        &ShellOptions { theme: resolved, ..ShellOptions::default() },
    )
}

/// What a theme tells the browser, which is the whole of how a slide looks.
///
/// The custom properties and the grid, taken out of the rendered page rather
/// than read off the theme document — a token the shell forgets to emit is
/// exactly the regression this exists to catch, and only the page knows.
/// A declaration can span lines, and the one that decides the layout does.
///
/// `grid-template-areas` is written a row per line, so reading one line of it
/// captures the property name and none of the answer. The first version of this
/// did exactly that, and every layout produced a byte-identical snapshot — a
/// suite of twenty-four files that covered four themes and, silently, one
/// layout.
fn declarations(page: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut lines = page.lines().map(str::trim).peekable();

    while let Some(line) = lines.next() {
        if !line.starts_with("--slidx") && !line.starts_with("grid-template-areas") {
            continue;
        }

        let mut declaration = line.to_string();
        while !declaration.ends_with(';') {
            match lines.next() {
                Some(rest) => {
                    declaration.push(' ');
                    declaration.push_str(rest);
                }
                None => break,
            }
        }

        found.push(declaration.trim_end_matches(';').to_string());
    }

    found.sort();
    found.dedup();
    found
}

/// What a slide's body was arranged into: the regions, and what landed in them.
///
/// This is the half a token cannot decide. Every page carries every layout's
/// grid — one stylesheet, and the slide picks a row of it — so the CSS is
/// identical whichever `layout:` a deck names, and the difference is entirely
/// in which regions the markup produced.
fn regions(page: &str) -> Vec<String> {
    page.lines()
        .map(str::trim)
        // Elements, not the stylesheet's selectors for them. Every page carries
        // rules for all six layouts, so including those would put the same
        // hundred lines in every file and bury the six that differ.
        .filter(|line| line.starts_with('<'))
        .filter(|line| line.contains("data-slidx-region") || line.contains("data-slidx-block"))
        .map(|line| line.split('>').next().unwrap_or(line).to_string())
        .collect()
}

#[test]
fn every_theme_declares_what_it_declared_before() {
    for theme in slidx_theme::builtin::all() {
        insta::assert_yaml_snapshot!(theme.id.clone(), declarations(&slide_for(&theme.id, "full")));
    }
}

#[test]
fn every_layout_arranges_a_slide_the_way_it_did_before() {
    for layout in slidx_theme::layout::all() {
        insta::assert_yaml_snapshot!(
            format!("layout-{}", layout.id),
            regions(&slide_for("minimal", &layout.id))
        );
    }
}

/// The reason four themes and six layouts are ten snapshots and not twenty-four.
///
/// A theme is a token document and a layout is a grid, and neither can reach
/// the other: every page ships every layout's grid, so what a theme declares is
/// the same in all six, and what a layout arranges is the same under all four.
///
/// That independence is what makes the matrix a sum rather than a product — so
/// it is asserted here rather than assumed, and a change that made a theme able
/// to move a region would fail this instead of quietly needing fourteen more
/// snapshot files.
#[test]
fn a_theme_and_a_layout_cannot_reach_each_other() {
    for theme in slidx_theme::builtin::all() {
        let baseline = declarations(&slide_for(&theme.id, "full"));

        for layout in slidx_theme::layout::all() {
            assert_eq!(
                declarations(&slide_for(&theme.id, &layout.id)),
                baseline,
                "{} declares something different under {}",
                theme.id,
                layout.id
            );
        }
    }

    for layout in slidx_theme::layout::all() {
        let baseline = regions(&slide_for("minimal", &layout.id));

        for theme in slidx_theme::builtin::all() {
            assert_eq!(
                regions(&slide_for(&theme.id, &layout.id)),
                baseline,
                "{} arranges {} differently",
                theme.id,
                layout.id
            );
        }
    }
}

/// Both schemes, because half a theme is the half nobody checks.
///
/// A theme carries a light palette and a dark one, and the dark one is reached
/// through `prefers-color-scheme` — so it is the half a person developing in
/// daylight never sees, and the half a projector in a dark room shows.
#[test]
fn every_theme_declares_a_dark_scheme_as_well_as_a_light_one() {
    for theme in slidx_theme::builtin::all() {
        let page = slide_for(&theme.id, "full");

        assert!(
            page.contains("prefers-color-scheme: dark"),
            "{} emits no dark scheme at all",
            theme.id
        );

        let dark = page
            .split_once("prefers-color-scheme: dark")
            .map(|(_, rest)| rest)
            .expect("the block just asserted");

        insta::assert_yaml_snapshot!(format!("{}-dark", theme.id), declarations(dark));
    }
}

/// The count itself, so a theme or a layout cannot be added unsnapshotted.
///
/// A snapshot suite that silently covers five of six layouts is the failure
/// mode of every snapshot suite, and it looks identical to a passing one.
#[test]
fn the_snapshots_cover_every_theme_and_every_layout() {
    let themes = slidx_theme::builtin::all().len();
    let layouts = slidx_theme::layout::all().len();

    assert_eq!(
        (themes, layouts),
        (4, 6),
        "a theme or a layout was added — snapshot it with `cargo insta accept`, \
         and read what it drew before you do"
    );
}
