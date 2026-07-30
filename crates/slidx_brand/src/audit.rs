//! Holding the brand to the linter the decks are held to.
//!
//! A brand whose own colours failed `slidx lint` would be the most expensive
//! kind of embarrassment available to this project: the argument on the
//! documentation site's front page, contradicted by the page it is printed on.
//!
//! So the palette describes itself as [`Surface`]s and goes through
//! [`slidx_lint::lint`] unchanged — the same function, the same floors, the same
//! projector model. Nothing here reimplements a ratio.

use slidx_core::{parse_deck, Deck, DeckParseOptions, Diagnostics};
use slidx_lint::{lint, LintInput, LintOptions, ProjectorProfile, RenderTarget, ViewingProfile};

use crate::palette::{self, Scheme};
use crate::tokens;

/// The room the brand's *type* is read in.
///
/// A documentation page on a laptop: a 0.3m-tall viewport at half a metre. Not a
/// conference room, and stating so is the point — the legibility rule holds text
/// to the angular size it subtends from the reader, and applying a back row at
/// fifteen metres to a 17px paragraph would report every heading on the site as
/// unreadable while measuring nothing true.
///
/// This is the same model the deck linter uses, given the distance that actually
/// applies. It is why the brand's base size may be 17 where a theme's is 32, and
/// it is checked rather than asserted: at this distance the body floor is about
/// 6.5px, so 17 has real headroom.
pub const READING_ROOM: ViewingProfile = ViewingProfile { screen_height_m: 0.3, back_row_m: 0.5 };

/// The page the brand's sizes are quoted against, in CSS pixels.
const PAGE: RenderTarget = RenderTarget { width_px: 1440.0, height_px: 900.0 };

/// Lints one scheme of the brand palette.
pub fn audit(scheme: Scheme, options: &LintOptions) -> Diagnostics {
    let deck = empty_deck();
    let surfaces = palette::of(scheme).surfaces(scheme, tokens::TYPE_SCALE.base_px);

    lint(&LintInput::new(&deck, &surfaces).with_target(PAGE), options)
}

/// Lints both schemes under every lighting condition slidx models.
///
/// The projector profiles are here even though the brand's type is read on a
/// screen, and that is deliberate: the mark and the signal end up on a social
/// card, a title slide, and a projected screenshot, and the linter's washout
/// model does not depend on how large the text is. The bright room is the
/// profile that decided which blue the signal is.
pub fn audit_every_room() -> Diagnostics {
    let mut all = Diagnostics::default();

    for scheme in Scheme::ALL {
        for projector in [
            ProjectorProfile::Direct,
            ProjectorProfile::DarkRoom,
            ProjectorProfile::Typical,
            ProjectorProfile::BrightRoom,
        ] {
            let options =
                LintOptions { projector, viewing: READING_ROOM, ..LintOptions::default() };
            all.extend(audit(scheme, &options));
        }
    }

    all
}

/// The linter needs a deck; the brand is not one.
///
/// An empty deck contributes no diagnostics of its own, so every finding that
/// comes back is about a colour. The theme audit does the same thing for the
/// same reason.
fn empty_deck() -> Deck {
    parse_deck("", &DeckParseOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_lint::{Surface, ViewingProfile};

    /// Named so a failure says which pair broke rather than only that one did.
    fn describe(diagnostics: &Diagnostics) -> String {
        diagnostics.iter().map(|d| format!("\n  [{}] {}", d.code, d.message)).collect()
    }

    fn reading(projector: ProjectorProfile) -> LintOptions {
        LintOptions { projector, viewing: READING_ROOM, ..LintOptions::default() }
    }

    #[test]
    fn the_brand_passes_the_linter_it_ships() {
        for scheme in Scheme::ALL {
            let diagnostics = audit(scheme, &reading(ProjectorProfile::default()));
            assert!(
                diagnostics.is_empty(),
                "the brand's {} scheme fails slidx lint:{}",
                scheme.as_token(),
                describe(&diagnostics)
            );
        }
    }

    #[test]
    fn the_brands_base_size_clears_the_legibility_floor_at_reading_distance() {
        // The number that makes 17px defensible rather than merely conventional,
        // and the reason the brand may differ from a theme's 32px base at all.
        let floor =
            slidx_lint::min_font_px(slidx_lint::TextRole::Body, PAGE.height_px, READING_ROOM);

        assert!(floor < 8.0, "the reading floor came out at {floor:.1}px, which is a slide's");
        assert!(
            tokens::TYPE_SCALE.base_px > floor * 2.0,
            "base {} is close to the {floor:.1}px reading floor",
            tokens::TYPE_SCALE.base_px
        );
    }

    #[test]
    fn the_legibility_rule_still_has_teeth_at_reading_distance() {
        // Widening the viewing profile could have switched the size rule off
        // entirely, which would be a check that passes because it stopped
        // asking. Six-point type on a page is still six-point type.
        let deck = empty_deck();
        let surfaces = palette::light().surfaces(Scheme::Light, 4.0);
        let diagnostics = lint(
            &LintInput::new(&deck, &surfaces).with_target(PAGE),
            &reading(ProjectorProfile::default()),
        );

        assert!(diagnostics.iter().any(|d| d.code == "legibility/font-size"));
    }

    #[test]
    fn the_brand_holds_up_in_every_room_slidx_models() {
        let diagnostics = audit_every_room();
        assert!(diagnostics.is_empty(), "the brand fails in some rooms:{}", describe(&diagnostics));
    }

    #[test]
    fn the_brands_colours_survive_a_hall_with_the_lights_up() {
        // The hardest lighting the model offers, which is the one a mark on a
        // projected title screen actually meets. Only the contrast half is
        // asserted here: the sizes on a slide come from the theme's scale, so
        // holding the brand's reading sizes to a back row would be checking a
        // number nothing sets.
        let options = LintOptions {
            projector: ProjectorProfile::BrightRoom,
            viewing: ViewingProfile::HALL,
            ..LintOptions::default()
        };

        for scheme in Scheme::ALL {
            let contrast: Diagnostics = audit(scheme, &options)
                .into_iter()
                .filter(|d| d.code.starts_with("contrast/"))
                .collect();

            assert!(
                contrast.is_empty(),
                "the brand's {} scheme loses contrast in a hall:{}",
                scheme.as_token(),
                describe(&contrast)
            );
        }
    }

    #[test]
    fn the_audit_is_not_vacuous() {
        // The loop could close because nothing is being checked. This is the
        // guard: the obvious blue for this palette measures 4.46:1 on brand
        // paper in a bright room, and the audit has to say so.
        let deck = empty_deck();
        let mut palette = palette::light();
        palette.signal = palette::hex("#1d4ed8");

        let surfaces = palette.surfaces(Scheme::Light, tokens::TYPE_SCALE.base_px);
        let options =
            LintOptions { projector: ProjectorProfile::BrightRoom, ..LintOptions::default() };
        let diagnostics = lint(&LintInput::new(&deck, &surfaces), &options);

        assert!(
            diagnostics.iter().any(|d| d.code.starts_with("contrast/")),
            "the near-miss blue passed, so the audit is measuring nothing"
        );
    }

    #[test]
    fn a_surface_list_the_linter_never_sees_cannot_pass_it() {
        // Guards the other half of the same worry: that `surfaces` returns
        // nothing and the audit is empty for the trivial reason.
        let surfaces: Vec<Surface> = palette::light().surfaces(Scheme::Light, 17.0);
        assert!(surfaces.iter().map(|surface| surface.text.len()).sum::<usize>() >= 5);
    }
}
