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

use std::fmt::Write as _;

use slidx_highlight::Token;

use crate::palette::{Palette, Scheme};
use crate::scale::REFERENCE_HEIGHT_PX;
use crate::theme::Theme;

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
    css.push_str(&colors(&theme.light));
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
