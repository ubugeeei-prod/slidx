//! Holding a theme to the same rules as a deck.
//!
//! A theme is where most contrast and legibility problems actually originate:
//! a deck inherits them and the author never sees a colour value. So a theme
//! is linted directly, before any slide exists.
//!
//! This runs over third-party theme packages too. That is the point of
//! describing a theme as [`Surface`]s rather than as CSS — the rules do not
//! need to know where the theme came from.

use slidx_core::{parse_deck, Deck, DeckParseOptions, Diagnostics};
use slidx_lint::{lint, LintInput, LintOptions, RenderTarget, Surface};

use crate::scale::REFERENCE_HEIGHT_PX;
use crate::theme::Theme;

/// Lints a theme on its own.
pub fn audit(theme: &Theme, options: &LintOptions) -> Diagnostics {
    let deck = empty_deck();
    let surfaces = theme.surfaces();

    lint(&input(&deck, &surfaces), options)
}

/// Lints a theme against every room it might be shown in.
///
/// A theme that only holds up in a dark room is a theme that will fail
/// somewhere, and the author will not find out until they are on stage.
pub fn audit_every_room(theme: &Theme) -> Diagnostics {
    use slidx_lint::ProjectorProfile::{BrightRoom, DarkRoom, Typical};

    let deck = empty_deck();
    let surfaces = theme.surfaces();
    let mut all = Diagnostics::default();

    for projector in [DarkRoom, Typical, BrightRoom] {
        let options = LintOptions { projector, ..LintOptions::default() };
        all.extend(lint(&input(&deck, &surfaces), &options));
    }

    all
}

fn input<'a>(deck: &'a Deck, surfaces: &'a [Surface]) -> LintInput<'a> {
    LintInput::new(deck, surfaces).with_target(RenderTarget {
        width_px: REFERENCE_HEIGHT_PX * 16.0 / 9.0,
        height_px: REFERENCE_HEIGHT_PX,
    })
}

fn empty_deck() -> Deck {
    parse_deck("", &DeckParseOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;
    use slidx_lint::{ProjectorProfile, ViewingProfile};

    fn describe(diagnostics: &Diagnostics) -> String {
        diagnostics.iter().map(|d| format!("\n  [{}] {}", d.code, d.message)).collect::<String>()
    }

    #[test]
    fn every_built_in_theme_passes_its_own_linter() {
        // The closed loop: the rules that judge a deck also judge the themes
        // slidx ships. A theme that cannot pass them has no business being a
        // default.
        for theme in builtin::all() {
            let diagnostics = audit(&theme, &LintOptions::default());
            assert!(
                diagnostics.is_empty(),
                "theme `{}` fails its own linter:{}",
                theme.id,
                describe(&diagnostics)
            );
        }
    }

    #[test]
    fn every_built_in_theme_holds_up_in_every_room() {
        for theme in builtin::all() {
            let diagnostics = audit_every_room(&theme);
            assert!(
                diagnostics.is_empty(),
                "theme `{}` fails in some rooms:{}",
                theme.id,
                describe(&diagnostics)
            );
        }
    }

    #[test]
    fn the_contrast_theme_survives_a_hall() {
        // The theme that exists for difficult venues has to clear the hardest
        // combination slidx models.
        let options = LintOptions {
            projector: ProjectorProfile::BrightRoom,
            viewing: ViewingProfile::HALL,
            ..LintOptions::default()
        };

        let diagnostics = audit(&builtin::contrast(), &options);
        assert!(
            diagnostics.is_empty(),
            "contrast theme fails in a hall:{}",
            describe(&diagnostics)
        );
    }

    #[test]
    fn the_audit_is_not_vacuous() {
        // A guard against the loop closing because nothing is being checked.
        let mut broken = builtin::minimal();
        broken.light.text = crate::palette::hex("#d4d4d8");

        let diagnostics = audit(&broken, &LintOptions::default());
        assert!(diagnostics.iter().any(|d| d.code.starts_with("contrast/")));
    }

    #[test]
    fn a_theme_with_undersized_type_is_caught() {
        let mut broken = builtin::minimal();
        broken.scale.base_px = 14.0;

        let diagnostics = audit(&broken, &LintOptions::default());
        assert!(diagnostics.iter().any(|d| d.code == "legibility/font-size"));
    }

    #[test]
    fn strict_mode_is_advisory_rather_than_a_gate() {
        // Built-ins are not required to clear the enhanced 7:1 floor, but the
        // report should be available to authors who want it.
        let options = LintOptions { strict: true, ..LintOptions::default() };

        for theme in builtin::all() {
            for diagnostic in audit(&theme, &options).iter() {
                assert!(
                    !diagnostic.is_blocking(),
                    "strict mode must not produce errors: {} in {}",
                    diagnostic.code,
                    theme.id
                );
            }
        }
    }
}
