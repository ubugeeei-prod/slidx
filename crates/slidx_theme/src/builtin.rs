//! The built-in themes.
//!
//! Four, deliberately. A long list of themes is a long list of ways to ship a
//! deck nobody at the back can read; these four cover the situations a speaker
//! actually finds themselves in, and every one of them is held to the linter
//! by [`crate::audit`].
//!
//! All four are flat: no gradients, no shadows, no decorative radius. That is
//! a legibility decision before it is a taste one — gradients and shadows are
//! the first thing a projector turns to mud.
//!
//! Font stacks name system faces only. A theme that reaches for a webfont
//! fails the offline check, and a venue with no network is the normal case.

use crate::palette::{hex, Palette};
use crate::scale::TypeScale;
use crate::theme::{Spacing, Theme};

const SANS: &str = "system-ui, -apple-system, 'Segoe UI', 'Helvetica Neue', \
                    'Hiragino Sans', 'Noto Sans JP', 'Yu Gothic UI', sans-serif";
const MONO: &str = "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, \
                    'Roboto Mono', 'Noto Sans Mono CJK JP', monospace";

/// Every built-in theme.
pub fn all() -> Vec<Theme> {
    vec![minimal(), editorial(), terminal(), contrast()]
}

/// Looks up a built-in theme by id.
pub fn find(id: &str) -> Option<Theme> {
    all().into_iter().find(|theme| theme.id == id)
}

/// The default. Quiet, neutral, and out of the way.
pub fn minimal() -> Theme {
    Theme {
        id: "minimal".into(),
        name: "Minimal".into(),
        description: "Neutral greys and a single accent. The default.".into(),
        light: Palette {
            canvas: hex("#e7e7e9"),
            surface: hex("#ffffff"),
            text: hex("#18181b"),
            muted: hex("#52525b"),
            heading: hex("#09090b"),
            accent: hex("#1d4ed8"),
            border: hex("#d4d4d8"),
            code_surface: hex("#f4f4f5"),
            code_text: hex("#27272a"),
        },
        dark: Palette {
            canvas: hex("#09090b"),
            surface: hex("#18181b"),
            text: hex("#f4f4f5"),
            muted: hex("#a1a1aa"),
            heading: hex("#fafafa"),
            accent: hex("#bfdbfe"),
            border: hex("#3f3f46"),
            code_surface: hex("#27272a"),
            code_text: hex("#e4e4e7"),
        },
        scale: TypeScale::default(),
        spacing: Spacing::default(),
        font_sans: SANS.into(),
        font_mono: MONO.into(),
    }
}

/// Warmer, with a wider type scale. For talks that are mostly prose.
pub fn editorial() -> Theme {
    Theme {
        id: "editorial".into(),
        name: "Editorial".into(),
        description: "Warm neutrals and a dramatic type scale, for prose-led talks.".into(),
        light: Palette {
            canvas: hex("#e7e5e4"),
            surface: hex("#fafaf9"),
            text: hex("#1c1917"),
            muted: hex("#57534e"),
            heading: hex("#0c0a09"),
            accent: hex("#9a3412"),
            border: hex("#d6d3d1"),
            code_surface: hex("#f5f5f4"),
            code_text: hex("#292524"),
        },
        dark: Palette {
            canvas: hex("#0c0a09"),
            surface: hex("#1c1917"),
            text: hex("#f5f5f4"),
            muted: hex("#a8a29e"),
            heading: hex("#fafaf9"),
            accent: hex("#fdba74"),
            border: hex("#44403c"),
            code_surface: hex("#292524"),
            code_text: hex("#e7e5e4"),
        },
        scale: TypeScale { base_px: 34.0, ratio: 1.333, code_factor: 0.95 },
        spacing: Spacing { padding_px: 112.0, block_px: 32.0, ..Spacing::default() },
        font_sans: SANS.into(),
        font_mono: MONO.into(),
    }
}

/// Dark, monospace-forward, sized for code. For live-coding talks.
pub fn terminal() -> Theme {
    Theme {
        id: "terminal".into(),
        name: "Terminal".into(),
        description: "Dark and monospace-forward, sized for code-heavy talks.".into(),
        light: Palette {
            canvas: hex("#e4e4e7"),
            surface: hex("#fafafa"),
            text: hex("#18181b"),
            muted: hex("#52525b"),
            heading: hex("#09090b"),
            accent: hex("#166534"),
            border: hex("#d4d4d8"),
            code_surface: hex("#f4f4f5"),
            code_text: hex("#18181b"),
        },
        dark: Palette {
            canvas: hex("#000000"),
            surface: hex("#0c0c0c"),
            text: hex("#e4e4e7"),
            muted: hex("#a1a1aa"),
            heading: hex("#fafafa"),
            accent: hex("#86efac"),
            border: hex("#3f3f46"),
            code_surface: hex("#18181b"),
            code_text: hex("#e4e4e7"),
        },
        // Code is the point of this theme, so it is set at full body size.
        scale: TypeScale { base_px: 32.0, ratio: 1.2, code_factor: 1.0 },
        spacing: Spacing { padding_px: 80.0, block_px: 24.0, ..Spacing::default() },
        font_sans: MONO.into(),
        font_mono: MONO.into(),
    }
}

/// Maximum separation, for a bright room or a tired projector.
///
/// What `slidx doctor` recommends switching to when it measures a washed-out
/// display. Pure black on pure white is avoided deliberately — at full
/// separation it produces halation that makes text harder to read, not easier.
pub fn contrast() -> Theme {
    Theme {
        id: "contrast".into(),
        name: "High contrast".into(),
        description: "Maximum separation and larger type, for bright rooms and weak projectors."
            .into(),
        light: Palette {
            canvas: hex("#ffffff"),
            surface: hex("#ffffff"),
            text: hex("#000000"),
            muted: hex("#2b2b2b"),
            heading: hex("#000000"),
            accent: hex("#0b3d91"),
            border: hex("#000000"),
            code_surface: hex("#f2f2f2"),
            code_text: hex("#000000"),
        },
        dark: Palette {
            canvas: hex("#000000"),
            surface: hex("#000000"),
            text: hex("#ffffff"),
            muted: hex("#d6d6d6"),
            heading: hex("#ffffff"),
            accent: hex("#9ec5ff"),
            border: hex("#ffffff"),
            code_surface: hex("#141414"),
            code_text: hex("#ffffff"),
        },
        scale: TypeScale { base_px: 38.0, ratio: 1.25, code_factor: 0.95 },
        spacing: Spacing { padding_px: 88.0, block_px: 32.0, ..Spacing::default() },
        font_sans: SANS.into(),
        font_mono: MONO.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<String> = all().into_iter().map(|theme| theme.id).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();

        assert_eq!(ids.len(), total);
    }

    #[test]
    fn every_theme_is_findable_by_id() {
        for theme in all() {
            assert_eq!(find(&theme.id).map(|found| found.id), Some(theme.id));
        }
        assert!(find("nonexistent").is_none());
    }

    #[test]
    fn every_theme_names_itself_and_says_when_to_use_it() {
        for theme in all() {
            assert!(!theme.name.is_empty(), "{} has no name", theme.id);
            assert!(!theme.description.is_empty(), "{} has no description", theme.id);
        }
    }

    #[test]
    fn no_theme_reaches_for_a_webfont() {
        // A font that is not on the machine is a font the venue will not have.
        for theme in all() {
            for stack in [&theme.font_sans, &theme.font_mono] {
                assert!(!stack.contains("http"), "{} names a remote font", theme.id);
                assert!(!stack.contains("url("), "{} names a remote font", theme.id);
            }
        }
    }

    #[test]
    fn every_stack_ends_in_a_generic_family() {
        for theme in all() {
            for stack in [&theme.font_sans, &theme.font_mono] {
                let last = stack.rsplit(',').next().unwrap().trim();
                assert!(
                    matches!(last, "sans-serif" | "serif" | "monospace"),
                    "{} ends in `{last}` rather than a generic family",
                    theme.id
                );
            }
        }
    }

    #[test]
    fn every_stack_carries_a_cjk_fallback() {
        // A Japanese deck that falls back to a Latin-only face renders as
        // tofu, which is unrecoverable on stage.
        for theme in all() {
            for stack in [&theme.font_sans, &theme.font_mono] {
                assert!(
                    stack.contains("Hiragino") || stack.contains("Noto Sans"),
                    "{} has no CJK fallback",
                    theme.id
                );
            }
        }
    }

    #[test]
    fn every_theme_is_flat() {
        // Gradients and shadows are the first thing a projector turns to mud,
        // so the built-ins do not offer the option.
        for theme in all() {
            assert_eq!(theme.spacing.radius_px, 0.0, "{} is not flat", theme.id);
        }
    }

    #[test]
    fn the_contrast_theme_sets_larger_type_than_the_default() {
        assert!(contrast().scale.base_px > minimal().scale.base_px);
    }

    #[test]
    fn the_terminal_theme_sets_code_at_full_body_size() {
        let theme = terminal();
        assert_eq!(theme.scale.code_px(), theme.scale.body_px());
    }

    #[test]
    fn dark_variants_are_actually_darker_than_their_light_ones() {
        for theme in all() {
            assert!(
                theme.dark.surface.relative_luminance() < theme.light.surface.relative_luminance(),
                "{} has a dark variant that is not darker",
                theme.id
            );
        }
    }
}
