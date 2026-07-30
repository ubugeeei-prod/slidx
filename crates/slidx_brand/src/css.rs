//! The tokens as custom properties.
//!
//! The JSON is the contract; this is the convenience. A page that wants the
//! brand should be able to link one file and have every value, rather than
//! write a build step to turn JSON into CSS — and a hand-written page has no
//! build step at all.
//!
//! Named `--slidx-brand-*` so nothing here can collide with the `--slidx-*`
//! properties a deck theme emits. A documentation page that embeds a live deck
//! has both stylesheets in scope at once, and a shared name would mean the brand
//! silently restyling the deck it is showing off.

use std::fmt::Write as _;

use crate::palette::{self, Palette, Scheme};
use crate::tokens;

/// Every token as a CSS block, both schemes.
///
/// The dark scheme is emitted twice for the reason [`slidx_theme::css`] emits
/// it twice: once under `prefers-color-scheme` so a page follows the reader, and
/// once under an explicit attribute so a page with its own toggle can win.
pub fn render() -> String {
    let brand = tokens::tokens();
    let mut css = String::with_capacity(1536);

    css.push_str(":root {\n");
    let _ = writeln!(css, "  --slidx-brand-font-sans: {};", brand.typography.font_sans);
    let _ = writeln!(css, "  --slidx-brand-font-mono: {};", brand.typography.font_mono);
    let _ =
        writeln!(css, "  --slidx-brand-size-heading-1: {}px;", brand.typography.size_px.heading1);
    let _ =
        writeln!(css, "  --slidx-brand-size-heading-2: {}px;", brand.typography.size_px.heading2);
    let _ =
        writeln!(css, "  --slidx-brand-size-heading-3: {}px;", brand.typography.size_px.heading3);
    let _ = writeln!(css, "  --slidx-brand-size-body: {}px;", brand.typography.size_px.body);
    let _ = writeln!(css, "  --slidx-brand-size-code: {}px;", brand.typography.size_px.code);
    let _ = writeln!(css, "  --slidx-brand-size-caption: {}px;", brand.typography.size_px.caption);
    let _ = writeln!(
        css,
        "  --slidx-brand-heading-tracking: {}em;",
        brand.typography.heading_tracking_em
    );
    let _ = writeln!(css, "  --slidx-brand-heading-weight: {};", brand.typography.heading_weight);
    let _ = writeln!(css, "  --slidx-brand-space-step: {}px;", brand.space.step_px);
    let _ = writeln!(css, "  --slidx-brand-space-padding: {}px;", brand.space.padding_px);
    let _ = writeln!(css, "  --slidx-brand-space-block: {}px;", brand.space.block_px);
    // Zero, and there is a repository-wide check that keeps it that way.
    let _ = writeln!(css, "  --slidx-brand-radius: {}px;", brand.space.radius_px);
    let _ = writeln!(css, "  --slidx-brand-hairline: {}px;", brand.space.hairline_px);
    let _ = writeln!(css, "  --slidx-brand-lockup-gap: {}em;", lockup_gap_em());
    css.push_str(&colors(&palette::of(Scheme::Light)));
    css.push_str("\n  color-scheme: light dark;\n}\n\n");

    css.push_str("@media (prefers-color-scheme: dark) {\n");
    css.push_str("  :root:not([data-slidx-brand-scheme=\"light\"]) {\n");
    css.push_str(&indent(&colors(&palette::of(Scheme::Dark))));
    css.push_str("  }\n}\n\n");

    css.push_str(":root[data-slidx-brand-scheme=\"dark\"] {\n");
    css.push_str(&colors(&palette::of(Scheme::Dark)));
    css.push_str("}\n");

    css
}

/// The lockup gap expressed against the mark's height, so a page that sizes the
/// mark in `em` gets the right gap without arithmetic.
fn lockup_gap_em() -> f64 {
    let geometry = crate::mark::Geometry::default();
    let lockup = crate::wordmark::Lockup::default();

    f64::from(lockup.gap_units(geometry)) / f64::from(geometry.grid)
}

fn colors(palette: &Palette) -> String {
    let mut css = String::with_capacity(256);

    for (name, value) in [
        ("paper", palette.paper),
        ("ink", palette.ink),
        ("muted", palette.muted),
        ("signal", palette.signal),
        ("line", palette.line),
    ] {
        let _ = writeln!(css, "  --slidx-brand-{name}: {};", value.to_hex());
    }

    css
}

fn indent(block: &str) -> String {
    block.lines().map(|line| format!("  {line}\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_becomes_a_custom_property() {
        let css = render();

        for role in ["paper", "ink", "muted", "signal", "line"] {
            assert!(css.contains(&format!("--slidx-brand-{role}:")), "missing {role}");
        }
    }

    #[test]
    fn nothing_here_can_collide_with_a_deck_themes_properties() {
        // A documentation page showing a live deck has both stylesheets in
        // scope. A shared property name would mean the brand restyling the deck
        // it is demonstrating.
        for line in render().lines().filter(|line| line.trim_start().starts_with("--slidx")) {
            assert!(
                line.trim_start().starts_with("--slidx-brand-"),
                "{line} is in the deck theme's namespace"
            );
        }
    }

    #[test]
    fn the_dark_scheme_follows_the_reader_and_yields_to_an_explicit_choice() {
        let css = render();

        assert!(css.contains("@media (prefers-color-scheme: dark)"));
        assert!(css.contains(":root:not([data-slidx-brand-scheme=\"light\"])"));
        assert!(css.contains(":root[data-slidx-brand-scheme=\"dark\"]"));
    }

    #[test]
    fn light_and_dark_emit_different_values() {
        let css = render();

        assert!(css.contains(&palette::light().signal.to_hex()));
        assert!(css.contains(&palette::dark().signal.to_hex()));
    }

    #[test]
    fn the_radius_is_zero() {
        assert!(render().contains("--slidx-brand-radius: 0px;"));
    }

    #[test]
    fn the_lockup_gap_is_a_quarter_of_the_marks_height() {
        // Two modules of eight. Stated in em so a page that sizes the mark in em
        // does no arithmetic.
        assert_eq!(lockup_gap_em(), 0.25);
        assert!(render().contains("--slidx-brand-lockup-gap: 0.25em;"));
    }

    #[test]
    fn braces_balance() {
        let css = render();
        assert_eq!(css.matches('{').count(), css.matches('}').count(), "unbalanced:\n{css}");
    }

    #[test]
    fn the_stylesheet_is_the_tokens_rather_than_a_second_set_of_numbers() {
        let brand = tokens::tokens();
        let css = render();

        assert!(
            css.contains(&format!("--slidx-brand-size-body: {}px;", brand.typography.size_px.body))
        );
        assert!(css.contains(&format!("--slidx-brand-signal: {};", brand.color.light.signal)));
    }
}
