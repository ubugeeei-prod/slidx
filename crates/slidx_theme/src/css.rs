//! Emitting a theme as CSS custom properties.
//!
//! Custom properties are the whole integration surface. A Vue island, a React
//! island, a plain Markdown slide, and a third-party theme all read the same
//! variables, which is how slidx stays framework-agnostic without shipping a
//! styling runtime.
//!
//! Sizes are emitted in `cqh` units — percentages of the slide container's
//! height — so a slide scales as one piece to whatever the projector gives it.
//! Nothing downstream ever has to shrink type to make content fit.
//!
//! Leading, tracking and measure are the exception, and deliberately: they are
//! emitted unitless or in `em`, which resolve against the element's own size
//! rather than the slide's. That is what lets one declaration be the right
//! measure on a heading and on a caption without either naming a size — see
//! [`crate::typography`].

use std::fmt::Write as _;

use slidx_highlight::Token;

use crate::palette::{Palette, Scheme};
use crate::scale::REFERENCE_HEIGHT_PX;
use crate::theme::Theme;
use crate::typography::Script;

/// Converts a reference-canvas pixel value into container-height units.
fn cqh(px: f64) -> String {
    format!("{:.4}cqh", px / REFERENCE_HEIGHT_PX * 100.0)
}

/// Renders a theme as a CSS block.
///
/// The dark variant is emitted twice: once under `prefers-color-scheme` so the
/// deck follows the room by default, and once under an explicit
/// `[data-scheme]` attribute so a presenter can override it at the venue when
/// the automatic choice is wrong.
pub fn render(theme: &Theme) -> String {
    let mut css = String::with_capacity(2048);

    let _ = writeln!(css, ":root {{");
    let _ = writeln!(css, "  --slidx-font-sans: {};", theme.font_sans);
    let _ = writeln!(css, "  --slidx-font-mono: {};", theme.font_mono);
    let _ = writeln!(css, "  --slidx-size-heading-1: {};", cqh(theme.scale.heading_px(1)));
    let _ = writeln!(css, "  --slidx-size-heading-2: {};", cqh(theme.scale.heading_px(2)));
    let _ = writeln!(css, "  --slidx-size-heading-3: {};", cqh(theme.scale.heading_px(3)));
    let _ = writeln!(css, "  --slidx-size-body: {};", cqh(theme.scale.body_px()));
    let _ = writeln!(css, "  --slidx-size-code: {};", cqh(theme.scale.code_px()));
    let _ = writeln!(css, "  --slidx-size-caption: {};", cqh(theme.scale.caption_px()));
    let _ = writeln!(css, "  --slidx-space-padding: {};", cqh(theme.spacing.padding_px));
    let _ = writeln!(css, "  --slidx-space-block: {};", cqh(theme.spacing.block_px));
    let _ = writeln!(css, "  --slidx-radius: {:.0}px;", theme.spacing.radius_px);
    let _ = writeln!(css, "  --slidx-hairline: {:.0}px;", theme.spacing.hairline_px);
    css.push_str(&setting(theme, Script::Latin));
    css.push_str(&colors(&theme.light));
    let _ = writeln!(css, "}}\n");

    // Where a subtree that switches back to Latin lands. Only the type
    // settings: sizes and colours are one document-wide answer and inherit from
    // `:root` unchanged.
    let _ = writeln!(css, "[lang] {{");
    css.push_str(&setting(theme, Script::Latin));
    let _ = writeln!(css, "}}\n");

    // The one script-dependent block, and the reason `[lang]` is in both
    // selectors rather than only in this one.
    //
    // A deck that mixes scripts is the normal case, not the exotic one — an
    // English pull-quote in a Japanese talk, a Japanese term in an English one
    // — so this cannot be a whole-document switch on `:root`. `:lang()` matches
    // an element by its *computed* language, so the override has to re-resolve
    // inside any subtree that says it is written in something else.
    //
    // A bare `:lang(ja)` would do that and would also redeclare fourteen custom
    // properties on every element in the document, which the print shell
    // multiplies by every slide in the deck. An element's computed language only
    // *changes* where a `lang` attribute says so, so matching `[lang]` reaches
    // exactly those elements and everything under one inherits the answer. The
    // Latin block above carries `[lang]` for the same reason: it is what a
    // subtree switching back to English lands on.
    let cjk =
        Script::CJK_TAGS.map(|tag| format!(":root:lang({tag}),\n[lang]:lang({tag})")).join(",\n");
    let _ = writeln!(css, "{cjk} {{");
    css.push_str(&setting(theme, Script::Cjk));
    let _ = writeln!(css, "}}\n");

    let _ = writeln!(css, "@media (prefers-color-scheme: dark) {{");
    let _ = writeln!(css, "  :root:not([data-scheme=\"light\"]) {{");
    css.push_str(&indent(&colors(&theme.dark)));
    let _ = writeln!(css, "  }}");
    let _ = writeln!(css, "}}\n");

    let _ = writeln!(css, ":root[data-scheme=\"dark\"] {{");
    css.push_str(&colors(&theme.dark));
    let _ = writeln!(css, "}}");

    css
}

/// The leading, tracking and measure for one script, as custom properties.
///
/// Driven off the same size list the sizes above are, so a role that gets a
/// `--slidx-size-*` gets a leading and a tracking to go with it. A role with a
/// size and no leading would silently inherit the one belonging to whatever
/// contains it, which reads as a theme that forgot to set it — because it is.
fn setting(theme: &Theme, script: Script) -> String {
    let mut css = String::with_capacity(512);
    let scale = &theme.scale;
    let base = scale.base_px;

    for (role, size) in [
        ("heading-1", scale.heading_px(1)),
        ("heading-2", scale.heading_px(2)),
        ("heading-3", scale.heading_px(3)),
        ("body", scale.body_px()),
        ("code", scale.code_px()),
        ("caption", scale.caption_px()),
    ] {
        let _ = writeln!(
            css,
            "  --slidx-leading-{role}: {:.3};",
            theme.typography.leading(size, base, script)
        );
        // `normal` rather than `0em` where the curve lands on nothing, because
        // they are not the same declaration: `0` *forbids* the adjustment a
        // font asks for, and `normal` is the absence of an instruction. The
        // base size is precisely where nothing is being asked for — every other
        // step of the scale is stated relative to it — so a zero there would be
        // an instruction the model never intended to give.
        let tracking = theme.typography.tracking_em(size, base, script);
        let _ = if tracking == 0.0 {
            writeln!(css, "  --slidx-tracking-{role}: normal;")
        } else {
            writeln!(css, "  --slidx-tracking-{role}: {tracking:.4}em;")
        };
    }

    // One length for both scripts. Thirty em is sixty Latin characters or
    // thirty Japanese ones, and those are the same sentence — the reasoning is
    // in `crate::typography`.
    let _ = writeln!(css, "  --slidx-measure-prose: {:.1}em;", theme.typography.prose_measure_em);
    let _ =
        writeln!(css, "  --slidx-measure-heading: {:.1}em;", theme.typography.heading_measure_em);

    css
}

fn colors(palette: &Palette) -> String {
    let mut css = String::with_capacity(512);

    for (name, value) in [
        ("canvas", palette.canvas),
        ("surface", palette.surface),
        ("text", palette.text),
        ("muted", palette.muted),
        ("heading", palette.heading),
        ("accent", palette.accent),
        ("border", palette.border),
        ("code-surface", palette.code_surface),
        ("code-text", palette.code_text),
    ] {
        let _ = writeln!(css, "  --slidx-color-{name}: {};", value.to_hex());
    }

    // Driven off the highlighter's own token list rather than a second list
    // here. A token the scanner emits and the theme never names would be a
    // class with no colour, which is invisible in review and obvious on stage.
    let syntax = palette.syntax();
    for token in Token::COLOURED {
        let _ = writeln!(
            css,
            "  --slidx-color-code-{}: {};",
            token.as_token(),
            syntax.get(token).to_hex()
        );
    }

    css
}

fn indent(block: &str) -> String {
    block.lines().map(|line| format!("  {line}\n")).collect()
}

/// The scheme a deck should start in, given a preference.
pub fn initial_scheme(prefers_dark: bool) -> Scheme {
    if prefers_dark {
        Scheme::Dark
    } else {
        Scheme::Light
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;

    #[test]
    fn every_colour_role_becomes_a_custom_property() {
        let css = render(&builtin::minimal());

        for name in
            ["canvas", "surface", "text", "muted", "heading", "accent", "border", "code-surface"]
        {
            assert!(css.contains(&format!("--slidx-color-{name}:")), "missing {name}");
        }
    }

    #[test]
    fn sizes_are_emitted_in_container_units_so_a_slide_scales_as_one_piece() {
        let css = render(&builtin::minimal());

        assert!(css.contains("--slidx-size-body:"));
        assert!(css.contains("cqh"));
        assert!(!css.contains("--slidx-size-body: 32px"), "absolute sizes would not scale");
    }

    #[test]
    fn the_reference_body_size_maps_onto_its_share_of_the_canvas() {
        // 32px on a 1080 canvas is 2.963% of the height.
        let css = render(&builtin::minimal());
        assert!(css.contains("--slidx-size-body: 2.9630cqh;"), "got:\n{css}");
    }

    #[test]
    fn every_token_the_highlighter_can_emit_has_a_colour() {
        // A class the scanner writes and the theme never names is invisible in
        // review and obvious on stage: that code renders in the inherited
        // colour and the highlighting silently does nothing.
        let css = render(&builtin::terminal());

        for token in Token::COLOURED {
            assert!(
                css.contains(&format!("--slidx-color-code-{}:", token.as_token())),
                "no colour for {}",
                token.as_token()
            );
        }
    }

    #[test]
    fn a_theme_with_no_syntax_colours_still_defines_every_property() {
        // Otherwise a deck on an older theme package gets a stylesheet with
        // undefined custom properties, and `var()` with no fallback inherits.
        let mut theme = builtin::minimal();
        theme.light.syntax = None;
        theme.dark.syntax = None;

        let css = render(&theme);

        for token in Token::COLOURED {
            assert!(css.contains(&format!("--slidx-color-code-{}:", token.as_token())));
        }
        assert!(css
            .contains(&format!("--slidx-color-code-comment: {}", theme.light.code_text.to_hex())));
    }

    #[test]
    fn a_cjk_deck_is_set_more_open_and_without_negative_tracking() {
        let css = render(&builtin::minimal());
        let cjk = css.split(":root:lang(ja)").nth(1).expect("a CJK block");

        assert!(cjk.contains("--slidx-leading-body: 1.700;"), "got:\n{cjk}");
        assert!(cjk.contains("--slidx-tracking-heading-1: normal;"), "got:\n{cjk}");
    }

    #[test]
    fn the_latin_setting_is_reachable_again_inside_a_cjk_deck() {
        // A deck that mixes scripts is the normal case. An English pull-quote in
        // a Japanese talk carries `lang="en"`, and it has to land on something —
        // without the bare `[lang]` block it would inherit the CJK leading from
        // the document and be set as Japanese in English.
        let css = render(&builtin::minimal());

        assert!(css.contains("\n[lang] {"), "no Latin block for a subtree to land on:\n{css}");
    }

    #[test]
    fn the_script_override_reaches_only_the_elements_that_change_language() {
        // A bare `:lang(ja)` is correct and redeclares every property on every
        // element in the document — which the print shell multiplies by every
        // slide. A language only *changes* where an attribute says so.
        let css = render(&builtin::minimal());

        assert!(css.contains("[lang]:lang(ja)"));
        assert!(
            !css.contains("\n:lang(ja)"),
            "the override must be anchored to an attribute, not to every element:\n{css}"
        );
    }

    #[test]
    fn every_size_has_a_leading_and_a_tracking_to_go_with_it() {
        // A role with a size and no leading silently inherits whatever contains
        // it, which reads as a theme that forgot to set it.
        let css = render(&builtin::editorial());

        for role in ["heading-1", "heading-2", "heading-3", "body", "code", "caption"] {
            assert!(css.contains(&format!("--slidx-size-{role}:")), "no size for {role}");
            assert!(css.contains(&format!("--slidx-leading-{role}:")), "no leading for {role}");
            assert!(css.contains(&format!("--slidx-tracking-{role}:")), "no tracking for {role}");
        }
    }

    #[test]
    fn the_measure_is_in_em_so_it_resolves_against_the_type_it_holds() {
        // `ch` is the advance of `0` — a Latin metric, which is how `22ch` came
        // to be about twelve characters of Japanese.
        let css = render(&builtin::minimal());

        assert!(css.contains("--slidx-measure-prose: 30.0em;"), "got:\n{css}");
        assert!(!css.contains("measure-prose: 30.0ch"));
    }

    #[test]
    fn the_dark_variant_follows_the_system_by_default() {
        let css = render(&builtin::minimal());
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
    }

    #[test]
    fn an_explicit_scheme_overrides_the_system_preference() {
        // A presenter who finds the automatic choice wrong at the venue needs
        // to be able to say so, and have it win.
        let css = render(&builtin::minimal());

        assert!(css.contains(":root[data-scheme=\"dark\"]"));
        assert!(
            css.contains(":root:not([data-scheme=\"light\"])"),
            "the media query must yield to an explicit light choice"
        );
    }

    #[test]
    fn light_and_dark_emit_different_values() {
        let theme = builtin::minimal();
        let css = render(&theme);

        assert!(css.contains(&theme.light.surface.to_hex()));
        assert!(css.contains(&theme.dark.surface.to_hex()));
    }

    #[test]
    fn flat_themes_emit_a_zero_radius() {
        assert!(render(&builtin::minimal()).contains("--slidx-radius: 0px;"));
    }

    #[test]
    fn every_built_in_theme_renders_without_panicking() {
        for theme in builtin::all() {
            assert!(!render(&theme).is_empty(), "{} rendered nothing", theme.id);
        }
    }

    #[test]
    fn braces_balance() {
        let css = render(&builtin::editorial());
        let opens = css.matches('{').count();
        let closes = css.matches('}').count();

        assert_eq!(opens, closes, "unbalanced braces in:\n{css}");
    }

    #[test]
    fn the_initial_scheme_follows_the_stated_preference() {
        assert_eq!(initial_scheme(true), Scheme::Dark);
        assert_eq!(initial_scheme(false), Scheme::Light);
    }
}
