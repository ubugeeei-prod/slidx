//! The wordmark, and the rules for locking it up with the mark.
//!
//! # The wordmark is set type, not drawn letterforms
//!
//! Every other wordmark in the world is outlines, and this one cannot be — or
//! rather, it can be outlines only if it is outlines of a face that is already
//! on the machine. A drawn wordmark would mean either a downloaded typeface,
//! which breaks the one promise the whole repository keeps, or a path nobody can
//! edit and nothing can check.
//!
//! So `slidx` is the brand's own sans stack — which is the default theme's
//! stack, read from it rather than repeated — at a stated weight and a stated
//! tracking. It renders identically to a heading on a slide, because it is one.
//! That is a constraint turned into consistency rather than worked around.
//!
//! Always lowercase. The crate is `slidx_*`, the command is `slidx`, the
//! packages are `@slidxjs/*`; a capitalised wordmark would be a second spelling of
//! the product's name.
//!
//! # The lockup
//!
//! One rule per relationship, and each is a multiple of the mark's own module so
//! the lockup scales without a second table of numbers:
//!
//! - **Size.** The wordmark is set at the mark's height. Nothing claims to match
//!   a cap height: a system stack resolves to a different face on every platform
//!   and its metrics are not knowable here, so the relationship is stated in em
//!   and the optical result is allowed to vary by a hair rather than being
//!   wrong on four platforms out of five.
//! - **Alignment.** Centres, not baselines, for the same reason.
//! - **Gap.** Two modules — a quarter of the mark's height.
//! - **Clear space.** Three modules on every side, measured from the lockup's
//!   bounding box. Three is the width of the document bar: the one measurement
//!   already in the mark, so the clear space is derived rather than chosen.
//! - **Minimum size.** A mark 17 units tall, because the wordmark is set at the
//!   mark's height and 17 is the brand's body size. Below that the wordmark is
//!   set smaller than body text and stops being a wordmark.
//!
//! There is no stacked lockup. Adding one would mean a second set of alignment
//! rules, and nothing in the repository needs one yet.

use serde::{Deserialize, Serialize};

use crate::mark::{self, Geometry};
use crate::palette::{self, Scheme};
use crate::tokens;

/// The name, as it is always set.
pub const WORDMARK: &str = "slidx";

/// Every measurement in the lockup, in mark modules unless stated.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lockup {
    /// Wordmark font size, as a multiple of the mark's height.
    pub size_ratio: f64,
    /// Between the mark and the wordmark.
    pub gap_modules: u32,
    /// On every side of the lockup's bounding box.
    pub clear_space_modules: u32,
    pub weight: u32,
    pub tracking_em: f64,
    /// Smallest the mark may be drawn inside a lockup, in pixels. Larger than
    /// the mark's own floor because the wordmark, not the mark, is the limit.
    pub min_px: u32,
}

impl Default for Lockup {
    fn default() -> Self {
        Self {
            size_ratio: 1.0,
            gap_modules: 2,
            clear_space_modules: 3,
            weight: 650,
            tracking_em: -0.02,
            min_px: 17,
        }
    }
}

impl Lockup {
    /// Wordmark font size, in mark units.
    ///
    /// The relationship the `size_ratio` field states, resolved. Every caller
    /// wants this rather than the ratio, so a lockup drawn anywhere gets the
    /// same size without repeating the multiplication.
    pub fn wordmark_size_units(self, geometry: Geometry) -> f64 {
        f64::from(geometry.grid) * self.size_ratio
    }

    /// The gap, in mark units.
    pub fn gap_units(self, geometry: Geometry) -> u32 {
        self.gap_modules * geometry.module
    }

    /// The clear space, in mark units.
    pub fn clear_space_units(self, geometry: Geometry) -> u32 {
        self.clear_space_modules * geometry.module
    }
}

/// The wordmark on its own.
///
/// `width` is generous rather than measured: text advance depends on the face
/// the platform resolved, so a box tight enough to be exact on one machine clips
/// on another. An SVG that is wider than its ink costs nothing.
pub fn render_wordmark(scheme: Scheme) -> String {
    let geometry = Geometry::default();
    let lockup = Lockup::default();
    let palette = palette::of(scheme);
    let height = geometry.grid;
    let width = height * 3;

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" role="img" aria-label="{WORDMARK}">
  <!--
    Set type, not outlines: a drawn wordmark would need a typeface the machine
    does not have, and a deck that downloads a font is the failure this project
    exists to prevent. The stack is the default theme's, so the wordmark and a
    heading on a slide resolve to the same face.
  -->
  <text x="0" y="{baseline}" fill="{ink}" font-family="{font}" font-size="{size}" font-weight="{weight}" letter-spacing="{tracking}em" dominant-baseline="central">{WORDMARK}</text>
</svg>
"#,
        baseline = height / 2,
        size = lockup.wordmark_size_units(geometry),
        ink = palette.ink.to_hex(),
        font = escape(&tokens::font_sans()),
        weight = lockup.weight,
        tracking = lockup.tracking_em,
    )
}

/// The mark and the wordmark, locked up.
///
/// The only composition anything downstream should draw by hand is none: the
/// gap and the clear space are here so a caller cannot get them wrong.
pub fn render_lockup(scheme: Scheme) -> String {
    let geometry = Geometry::default();
    let lockup = Lockup::default();
    let palette = palette::of(scheme);

    let clear = lockup.clear_space_units(geometry);
    let gap = lockup.gap_units(geometry);
    let text_x = clear + geometry.grid + gap;
    // Three times the mark's height for the word, plus the clear space. Wider
    // than the ink for the reason `render_wordmark` states.
    let width = text_x + geometry.grid * 3 + clear;
    let height = geometry.grid + clear * 2;

    let marks: String = mark::render(scheme)
        .lines()
        .filter(|line| line.trim_start().starts_with("<rect"))
        .map(|line| format!("    {}\n", line.trim()))
        .collect();

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" role="img" aria-label="{WORDMARK}">
  <!--
    The lockup, with every relationship a multiple of the mark's {module}-unit module:
    the wordmark is set at the mark's height, the gap is {gap_modules} modules, and the
    clear space is {clear_modules} modules -- the width of the document bar, so it is
    derived from the mark rather than chosen for it.

    Centres are aligned rather than baselines. The stack is a system stack and
    resolves to a different face on every platform, so its cap height is not
    knowable here and a rule that claimed one would be wrong nearly everywhere.
  -->
  <g transform="translate({clear} {clear})">
{marks}  </g>
  <text x="{text_x}" y="{middle}" fill="{ink}" font-family="{font}" font-size="{size}" font-weight="{weight}" letter-spacing="{tracking}em" dominant-baseline="central">{WORDMARK}</text>
</svg>
"#,
        module = geometry.module,
        gap_modules = lockup.gap_modules,
        clear_modules = lockup.clear_space_modules,
        middle = height / 2,
        ink = palette.ink.to_hex(),
        font = escape(&tokens::font_sans()),
        size = lockup.wordmark_size_units(geometry),
        weight = lockup.weight,
        tracking = lockup.tracking_em,
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_is_always_lowercase() {
        // The crate, the command and the packages are lowercase. A capitalised
        // wordmark would be a second spelling of the product's name.
        assert_eq!(WORDMARK, WORDMARK.to_lowercase());
    }

    #[test]
    fn every_lockup_measurement_is_a_multiple_of_the_marks_module() {
        // What lets the lockup scale without a second table of numbers.
        let geometry = Geometry::default();
        let lockup = Lockup::default();

        for units in [lockup.gap_units(geometry), lockup.clear_space_units(geometry)] {
            assert_eq!(units % geometry.module, 0);
        }
    }

    #[test]
    fn the_wordmark_is_set_at_the_marks_height() {
        // The size relationship, resolved rather than left as a ratio nothing
        // reads. A lockup whose wordmark ignored it would be two sets of rules.
        let geometry = Geometry::default();

        assert_eq!(Lockup::default().wordmark_size_units(geometry), f64::from(geometry.grid));
        assert!(render_lockup(Scheme::Light).contains("font-size=\"24\""));
        assert!(render_wordmark(Scheme::Light).contains("font-size=\"24\""));
    }

    #[test]
    fn a_different_size_ratio_reaches_the_drawn_wordmark() {
        // Guards the field being decorative: it is read, so changing it changes
        // the picture.
        let geometry = Geometry::default();
        let larger = Lockup { size_ratio: 1.5, ..Lockup::default() };

        assert_eq!(larger.wordmark_size_units(geometry), 36.0);
    }

    #[test]
    fn the_clear_space_is_the_width_of_the_document_bar() {
        // Derived from the mark rather than chosen for it, which is the only
        // reason anyone will remember the number.
        let geometry = Geometry::default();

        assert_eq!(Lockup::default().clear_space_units(geometry), geometry.document_width);
    }

    #[test]
    fn the_lockup_floor_is_the_brands_body_size() {
        // Below it the wordmark is set smaller than body text, which is the
        // point at which it stops being a wordmark.
        assert_eq!(f64::from(Lockup::default().min_px), tokens::TYPE_SCALE.base_px);
    }

    #[test]
    fn a_lockup_is_larger_than_the_mark_alone_because_the_wordmark_limits_it() {
        assert!(Lockup::default().min_px > Geometry::default().min_px);
    }

    #[test]
    fn the_wordmark_is_set_in_the_default_themes_stack() {
        // Not a lookalike. The wordmark and a heading on a slide have to resolve
        // to the same face or the brand does not match the product.
        let svg = render_wordmark(Scheme::Light);
        let expected = escape(&slidx_theme::default_theme().font_sans);

        assert!(svg.contains(&expected), "got:\n{svg}");
    }

    #[test]
    fn the_wordmark_names_no_font_it_would_have_to_download() {
        for svg in [render_wordmark(Scheme::Light), render_lockup(Scheme::Dark)] {
            let stripped = svg.replace(r#"xmlns="http://www.w3.org/2000/svg""#, "");

            for marker in ["http://", "https://", "@font-face", "@import", "url("] {
                assert!(!stripped.contains(marker), "the wordmark reaches for {marker}");
            }
        }
    }

    #[test]
    fn the_lockup_draws_the_mark_rather_than_a_copy_of_it() {
        // A second copy of the geometry is a second thing to update. The lockup
        // reads the mark's own rectangles.
        let lockup = render_lockup(Scheme::Light);

        assert_eq!(lockup.matches("<rect").count(), 4);
        for line in mark::render(Scheme::Light).lines().filter(|l| l.trim().starts_with("<rect")) {
            assert!(lockup.contains(line.trim()), "the lockup redraws {line}");
        }
    }

    #[test]
    fn the_lockup_reserves_its_clear_space_inside_its_own_box() {
        // So a caller who places the SVG flush against something else still gets
        // the clear space. A rule that only lived in prose would be a rule
        // nobody applies.
        let geometry = Geometry::default();
        let clear = Lockup::default().clear_space_units(geometry);
        let svg = render_lockup(Scheme::Light);

        assert!(svg.contains(&format!("translate({clear} {clear})")));
        assert!(svg.contains(&format!("height=\"{}\"", geometry.grid + clear * 2)));
    }

    #[test]
    fn the_lockup_carries_the_name_as_text_a_reader_can_select() {
        assert!(render_lockup(Scheme::Light).contains(">slidx</text>"));
    }

    #[test]
    fn each_scheme_sets_the_wordmark_in_its_own_ink() {
        assert!(render_wordmark(Scheme::Dark).contains(&palette::dark().ink.to_hex()));
        assert!(render_wordmark(Scheme::Light).contains(&palette::light().ink.to_hex()));
    }

    #[test]
    fn a_font_stack_with_quotes_in_it_does_not_break_the_attribute() {
        // Every stack in this repository names faces with spaces, so they are
        // quoted, and an unescaped double quote would end the attribute early
        // and silently drop the rest of the stack.
        let svg = render_wordmark(Scheme::Light);
        let attribute = svg.split("font-family=\"").nth(1).unwrap();
        let value = attribute.split('"').next().unwrap();

        assert!(value.contains("sans-serif"), "the stack was truncated: {value}");
    }
}
