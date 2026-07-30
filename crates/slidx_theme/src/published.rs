//! The theme this repository publishes as a package.
//!
//! # Why a theme lives here and is not built in
//!
//! [`crate::builtin`] is four themes on purpose, and the reason is stated
//! there: a long list of themes is a long list of ways to ship a deck nobody at
//! the back can read. Adding a fifth to that list would be arguing against it.
//!
//! But a distribution path nobody has walked is a path that does not work.
//! `workshop` exists so the package format is exercised by something real —
//! generated here, published as `@slidx/theme-workshop`, and reached by a deck
//! through [`crate::package`] with no shortcut back into `builtin::find`.
//! [`crate::resolve`] does not consult this module, and a test asserts it: the
//! moment `workshop` resolves as a built-in, the path stops being tested.
//!
//! # Why the document is generated rather than written
//!
//! `packages/theme-workshop/theme.json` is machine output, checked in and
//! checked against this module, the same arrangement `assets/brand/` has. A
//! theme package's palette is the one thing that must not be a list of hex
//! literals somebody pasted — that is the mistake `scripts/check-borrowed.mjs`
//! exists because of — so the colours are mixed by [`Recipe`] from a hue, a
//! chroma and a lightness, and the JSON is what falls out.
//!
//! # What `workshop` is for
//!
//! A hands-on session, which is the one room the four built-ins deliberately
//! do not cover. The audience is typing along, so the slide is a reference
//! somebody glances up at rather than a thing they watch:
//!
//! - **Code is set at body size**, because the line being copied is the
//!   content and prose is the caption on it.
//! - **The type scale is narrow**, so a heading does not take the room a
//!   snippet needs.
//! - **Light, not dark.** A workshop room stays lit so people can see their own
//!   keyboards, and ambient light is what washes a dark slide out — which is
//!   the opposite of `terminal`'s reasoning, and correct for the opposite room.
//! - **The transition is barely there.** A workshop deck is stepped backwards
//!   as often as forwards, because somebody always missed the last command.

use slidx_core::Easing;

use crate::builtin::recipe::Recipe;
use crate::scale::TypeScale;
use crate::theme::{Motion, Spacing, Theme};

/// What a deck writes in `theme:` to reach this theme.
pub const ID: &str = "workshop";

/// The npm package that carries it.
pub const PACKAGE: &str = "@slidx/theme-workshop";

/// System faces only, the same rule the built-ins are held to.
///
/// A theme package is the easiest place in this project to break the offline
/// guarantee, because a font stack in a published file is a long way from the
/// rule that would catch a remote asset in a deck.
///
/// Written out again rather than shared with [`crate::builtin`], and that is
/// the point rather than an oversight. This stack is *published*: it is frozen
/// into a document on a registry the moment the package ships, so a built-in
/// changing its own faces next year must not silently change what a deck
/// installed. A theme package owns its typography or it does not own anything.
const SANS: &str = "system-ui, -apple-system, 'Segoe UI', 'Helvetica Neue', \
                    'Hiragino Sans', 'Noto Sans JP', 'Yu Gothic UI', sans-serif";
const MONO: &str = "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, \
                    'Roboto Mono', 'Noto Sans Mono CJK JP', monospace";

/// The theme, as the package ships it.
pub fn workshop() -> Theme {
    Theme {
        id: ID.into(),
        name: "Workshop".into(),
        description: "Light, code at body size, for a session the audience is typing along with."
            .into(),
        // A teal nothing built in uses, so a workshop deck is recognisably not
        // one of the four — which is the only thing a theme package can offer
        // that a built-in cannot.
        light: Recipe { hue: 197.0, accent_chroma: 0.13, wash: 0.09, sheet: 0.98, ink: 0.25 }
            .palette(),
        // Carried because every theme carries both: a room whose lights go down
        // for a demo is still the same deck.
        dark: Recipe { hue: 197.0, accent_chroma: 0.12, wash: 0.09, sheet: 0.20, ink: 0.94 }
            .palette(),
        scale: TypeScale { base_px: 33.0, ratio: 1.18, code_factor: 1.0 },
        // Tighter than any built-in, and the one number that says what this
        // theme is for: the unit of content is a line somebody is retyping, so
        // every pixel of padding is a character that wraps. It is still a real
        // safe area — the guard's floor is well below it.
        spacing: Spacing { padding_px: 72.0, block_px: 26.0, ..Spacing::default() },
        motion: Motion { transition_ms: 120, transition_easing: Easing::EaseOut },
        font_sans: SANS.into(),
        font_mono: MONO.into(),
    }
}

/// Where the generated document is committed, from the workspace root.
pub const DOCUMENT_PATH: &str = "packages/theme-workshop/theme.json";

/// The package's theme document, as the file holds it.
///
/// Pretty-printed with a trailing newline: it is committed, so it is read in
/// review as often as it is parsed.
pub fn document() -> String {
    format!("{}\n", serde_json::to_string_pretty(&workshop()).expect("a theme serialises"))
}

/// The workspace root, from this crate's manifest.
fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// Writes the document the package ships. Used by the emitter, and nothing else.
pub fn write() -> std::io::Result<std::path::PathBuf> {
    let path = workspace_root().join(DOCUMENT_PATH);
    std::fs::write(&path, document())?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{Catalogue, Published};
    use crate::{audit, builtin};
    use slidx_lint::LintOptions;

    #[test]
    fn the_committed_document_is_what_this_module_produces() {
        // The `assets/brand/` arrangement: a generated file is committed so its
        // consumers never have to run Rust, and this fails the moment the two
        // stop agreeing. `vp run generate:theme` is how you fix it.
        let committed = std::fs::read_to_string(workspace_root().join(DOCUMENT_PATH))
            .unwrap_or_else(|_| panic!("{DOCUMENT_PATH} is missing. Run `vp run generate:theme`."));

        assert_eq!(committed, document(), "run `vp run generate:theme`");
    }

    #[test]
    fn the_published_theme_is_not_reachable_as_a_built_in() {
        // The whole reason it is published rather than added to the four. If
        // this ever passed through `builtin::find`, every test below would be
        // exercising the built-in path and the package path would be untested.
        assert!(crate::resolve(ID).is_none());
        assert!(builtin::find(ID).is_none());
    }

    #[test]
    fn a_deck_naming_it_resolves_once_the_package_is_installed() {
        let catalogue = Catalogue::read(&[Published::new(PACKAGE, document())]);

        assert!(catalogue.diagnostics().is_empty(), "{:?}", catalogue.diagnostics());
        assert_eq!(catalogue.resolve(ID).unwrap().theme, workshop());
        assert_eq!(catalogue.resolve(ID).unwrap().source.as_deref(), Some(PACKAGE));
    }

    #[test]
    fn the_published_theme_passes_the_linter_the_built_ins_pass() {
        let diagnostics = audit::audit(&workshop(), &LintOptions::default());

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn the_published_theme_holds_up_in_every_room_slidx_models() {
        let diagnostics = audit::audit_every_room(&workshop());

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn the_guard_finds_nothing_to_repair_in_the_theme_this_repository_ships() {
        // A published theme that needed its own guard would be shipping the
        // problem the guard exists for.
        let held = crate::package::guard::hold(workshop(), &crate::default_theme());

        assert_eq!(held.repairs, vec![]);
    }

    #[test]
    fn it_reaches_for_no_webfont() {
        let theme = workshop();

        for stack in [&theme.font_sans, &theme.font_mono] {
            assert!(!stack.contains("http"), "{stack}");
            assert!(!stack.contains("url("), "{stack}");
        }
    }

    #[test]
    fn it_sets_code_at_body_size_because_the_line_being_copied_is_the_content() {
        let scale = workshop().scale;

        assert_eq!(scale.code_px(), scale.body_px());
    }

    #[test]
    fn it_pads_less_than_any_built_in_because_a_line_of_code_has_to_fit() {
        let tightest =
            builtin::all().iter().map(|theme| theme.spacing.padding_px).fold(f64::MAX, f64::min);

        assert!(workshop().spacing.padding_px < tightest);
    }

    #[test]
    fn its_tight_padding_is_still_a_safe_area_the_guard_accepts() {
        // The line between a theme with an opinion about density and a theme
        // whose first row is cropped by the room.
        let held = crate::package::guard::hold(workshop(), &crate::default_theme());

        assert_eq!(held.theme.spacing.padding_px, workshop().spacing.padding_px);
    }

    #[test]
    fn it_is_flat_like_everything_else_slidx_draws() {
        assert_eq!(workshop().spacing.radius_px, 0.0);
    }

    #[test]
    fn it_carries_a_hue_no_built_in_theme_uses() {
        // A theme package that looked like one of the four would demonstrate
        // the format and nothing else.
        for theme in builtin::all() {
            assert_ne!(theme.light.accent, workshop().light.accent, "same accent as {}", theme.id);
        }
    }

    #[test]
    fn its_document_round_trips_through_the_format_a_package_ships() {
        let parsed: Theme = serde_json::from_str(&document()).unwrap();

        assert_eq!(parsed, workshop());
    }
}
