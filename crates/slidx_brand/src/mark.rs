//! The mark.
//!
//! # What the form is about
//!
//! slidx compiles **one document into a sequence of pages**. That is the whole
//! product in one sentence, and it is what the mark draws: a single full-height
//! block, a gutter, and a run of pages beside it that add up to exactly the same
//! height. One form or many — the same document either way.
//!
//! The colour does the semantic half. The document is **ink**, because it is the
//! thing the author writes; the pages are **signal**, because they are the thing
//! slidx produces. A mark whose colours were reversed would be arguing the
//! opposite.
//!
//! # The construction
//!
//! A 24-unit square on a module of **3**, so the grid is 8 × 8 modules and every
//! edge lands on it. Nothing here is eyeballed:
//!
//! | part         | modules | units          |
//! | ------------ | ------- | -------------- |
//! | the document | 3 × 8   | x 0…9, y 0…24  |
//! | the gutter   | 1       | x 9…12         |
//! | a page       | 4 × 2   | x 12…24        |
//! | a gap        | 1       | y 6…9, 15…18   |
//!
//! Across: 3 + 1 + 4 = 8. Down: 2 + 1 + 2 + 1 + 2 = 8.
//!
//! **The corner radius is zero.** Not a taste: the built-in themes are flat
//! because a projector turns a radius and a shadow to mud before it loses
//! anything else, and a mark that broke that rule would be the one asset in the
//! repository exempt from the argument the rest of it makes.
//!
//! # Why it survives 16 pixels
//!
//! The smallest feature is one module, and one module is an eighth of the mark.
//! At the 16-pixel minimum that is 2 device pixels, which a browser can still
//! resolve as a gap rather than smearing three pages into one block. That bound
//! is asserted rather than eyeballed — see `no_feature_is_smaller_than_one_module`.

use serde::{Deserialize, Serialize};
use slidx_lint::Rgba;

use crate::palette::{self, Scheme};

/// The mark's grid, emitted into the tokens so a consumer can lay the mark out
/// without re-deriving it from the SVG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    /// Side of the square the mark is drawn in, in units.
    pub grid: u32,
    /// The unit every edge is a multiple of.
    pub module: u32,
    pub document_width: u32,
    /// Between the document and the pages.
    pub gutter: u32,
    pub page_width: u32,
    pub page_height: u32,
    pub page_gap: u32,
    pub pages: u32,
    /// Below this the gaps stop resolving and the pages read as one block.
    pub min_px: u32,
}

impl Default for Geometry {
    fn default() -> Self {
        Self {
            grid: 24,
            module: 3,
            document_width: 9,
            gutter: 3,
            page_width: 12,
            page_height: 6,
            page_gap: 3,
            pages: 3,
            min_px: 16,
        }
    }
}

impl Geometry {
    /// Where the pages begin.
    pub fn pages_x(self) -> u32 {
        self.document_width + self.gutter
    }

    /// Top edge of page `index`, zero-based.
    pub fn page_y(self, index: u32) -> u32 {
        index * (self.page_height + self.page_gap)
    }

    /// The smallest drawn or undrawn feature, in units. One module, by
    /// construction — which is what the minimum size is derived from.
    pub fn smallest_feature(self) -> u32 {
        self.module
    }
}

/// The mark, in one scheme.
pub fn render(scheme: Scheme) -> String {
    let palette = palette::of(scheme);
    svg(&palette.ink.to_hex(), &palette.signal.to_hex())
}

/// The mark in a single colour, for the places that only have one.
///
/// A favicon mask, a stencil, a terminal. The composition still reads: the
/// gutter and the gaps are the same module, so the pages stay separate from the
/// document without a second colour doing the work.
pub fn render_mono(fill: &str) -> String {
    svg(fill, fill)
}

/// The mark inset on a field of paper, for a platform that crops.
///
/// An app icon is masked to a circle or a squircle by the platform, so the mark
/// cannot run to the edge. The inset is the brand's clear space — the width of
/// the document bar — which puts the drawn area inside 80% of the tile and
/// therefore inside every maskable safe zone in use.
pub fn render_tile(scheme: Scheme) -> String {
    let geometry = Geometry::default();
    let palette = palette::of(scheme);
    let inset = geometry.document_width;
    let side = geometry.grid + inset * 2;

    let mark = svg(&palette.ink.to_hex(), &palette.signal.to_hex());
    let inner: String = mark
        .lines()
        .filter(|line| line.trim_start().starts_with("<rect"))
        .map(|line| format!("    {}\n", line.trim()))
        .collect();

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {side} {side}\" \
         width=\"{side}\" height=\"{side}\" role=\"img\" aria-label=\"slidx\">\n  \
         <!-- The mark inset by the brand's clear space, which is the width of the\n       \
         document bar. Everything drawn therefore sits inside 80% of the tile,\n       \
         inside every maskable safe zone a platform applies. -->\n  \
         <rect width=\"{side}\" height=\"{side}\" fill=\"{paper}\"/>\n  \
         <g transform=\"translate({inset} {inset})\">\n{inner}  </g>\n</svg>\n",
        paper = palette.paper.to_hex(),
    )
}

/// One mark, two fills.
///
/// The construction is restated in the file rather than only in this module,
/// because the SVG is what someone opens when they want to know how the mark is
/// built and a comment there is the only documentation that travels with it.
fn svg(document: &str, pages: &str) -> String {
    let geometry = Geometry::default();
    let Geometry { grid, module, document_width, page_width, page_height, .. } = geometry;

    let mut rects = format!(
        "  <rect x=\"0\" y=\"0\" width=\"{document_width}\" height=\"{grid}\" fill=\"{document}\"/>\n"
    );

    for index in 0..geometry.pages {
        rects.push_str(&format!(
            "  <rect x=\"{}\" y=\"{}\" width=\"{page_width}\" height=\"{page_height}\" fill=\"{pages}\"/>\n",
            geometry.pages_x(),
            geometry.page_y(index),
        ));
    }

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {grid} {grid}" width="{grid}" height="{grid}" role="img" aria-label="slidx">
  <!--
    One document, compiled into a sequence of pages.

    A {grid}-unit square on a module of {module}, so the grid is {modules} x {modules} modules
    and every edge below lands on it.

      the document  3 modules wide, 8 tall   x 0..{document_width}    y 0..{grid}
      the gutter    1 module                 x {document_width}..{pages_x}
      a page        4 modules wide, 2 tall   x {pages_x}..{grid}
      the gaps      1 module                 y {page_height}..{first_gap}, {second_gap_start}..{second_gap_end}

    Across: 3 + 1 + 4 = 8. Down: 2 + 1 + 2 + 1 + 2 = 8. Both sides of the
    gutter are exactly the same height, which is the thing being said: the
    same document, whether it is one form or a run of pages.

    The document is ink because the author writes it; the pages are signal
    because slidx produces them. No radius, no shadow, no gradient -- the
    built-in themes are flat because a projector turns those to mud first,
    and the mark is held to the same rule.
  -->
{rects}</svg>
"#,
        modules = grid / module,
        pages_x = geometry.pages_x(),
        first_gap = geometry.page_y(1),
        second_gap_start = geometry.page_y(1) + page_height,
        second_gap_end = geometry.page_y(2),
    )
}

/// The mark's fills, for a caller drawing it into a larger document.
pub fn fills(scheme: Scheme) -> (Rgba, Rgba) {
    let palette = palette::of(scheme);
    (palette.ink, palette.signal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_closes_in_both_directions() {
        // The claim the construction rests on. If either sum stopped landing on
        // the grid the mark would be a drawing rather than a construction.
        let geometry = Geometry::default();

        assert_eq!(geometry.document_width + geometry.gutter + geometry.page_width, geometry.grid);
        assert_eq!(
            geometry.pages * geometry.page_height + (geometry.pages - 1) * geometry.page_gap,
            geometry.grid
        );
    }

    #[test]
    fn every_edge_lands_on_the_module() {
        let geometry = Geometry::default();
        let module = geometry.module;

        for edge in [
            geometry.grid,
            geometry.document_width,
            geometry.gutter,
            geometry.page_width,
            geometry.page_height,
            geometry.page_gap,
            geometry.pages_x(),
            geometry.page_y(1),
            geometry.page_y(2),
        ] {
            assert_eq!(edge % module, 0, "{edge} is off the {module}-unit module");
        }
    }

    #[test]
    fn the_document_and_the_pages_are_the_same_height() {
        // The whole argument of the mark. A run of pages that came to less than
        // the document would read as loss rather than as compilation.
        let geometry = Geometry::default();
        let pages: u32 = (0..geometry.pages).map(|_| geometry.page_height).sum();

        assert_eq!(pages + (geometry.pages - 1) * geometry.page_gap, geometry.grid);
    }

    #[test]
    fn no_feature_is_smaller_than_one_module() {
        // What makes the 16-pixel floor honest: one module is an eighth of the
        // mark, so at the minimum size the gaps are 2 device pixels and a
        // browser still resolves them.
        let geometry = Geometry::default();
        let smallest_px = f64::from(geometry.min_px) * f64::from(geometry.smallest_feature())
            / f64::from(geometry.grid);

        assert!(smallest_px >= 2.0, "a gap is {smallest_px}px at the stated minimum");
    }

    #[test]
    fn the_mark_is_four_rectangles_and_nothing_else() {
        // A mark built from more primitives than it needs is a mark that will
        // not survive being 16 pixels wide.
        let svg = render(Scheme::Light);

        assert_eq!(svg.matches("<rect").count(), 4);
        for shape in ["<path", "<circle", "<polygon", "<ellipse", "<text"] {
            assert!(!svg.contains(shape), "the mark reaches for {shape}");
        }
    }

    #[test]
    fn the_mark_carries_no_text() {
        // The wordmark is a separate lockup. Text inside the mark would be text
        // that has to survive 16 pixels, and it cannot.
        assert!(!render(Scheme::Light).contains("<text"));
    }

    #[test]
    fn the_document_is_ink_and_the_pages_are_signal() {
        let palette = palette::of(Scheme::Light);
        let svg = render(Scheme::Light);

        let document = format!("height=\"24\" fill=\"{}\"", palette.ink.to_hex());
        assert!(svg.contains(&document), "the document is not ink:\n{svg}");
        assert_eq!(svg.matches(&format!("fill=\"{}\"", palette.signal.to_hex())).count(), 3);
    }

    #[test]
    fn each_scheme_draws_the_same_geometry_in_its_own_colours() {
        let light = render(Scheme::Light);
        let dark = render(Scheme::Dark);

        assert_ne!(light, dark);
        assert_eq!(light.matches("<rect").count(), dark.matches("<rect").count());
        assert!(dark.contains(&palette::dark().signal.to_hex()));
    }

    #[test]
    fn a_single_colour_mark_still_separates_the_pages() {
        // The gaps carry the composition when the second colour is gone, which
        // is what makes a one-colour favicon mask possible at all.
        let mono = render_mono("currentColor");

        assert_eq!(mono.matches("fill=\"currentColor\"").count(), 4);
        assert!(mono.contains("y=\"9\""), "the gaps are gone");
    }

    #[test]
    fn nothing_in_the_mark_is_fetched() {
        // The offline promise, in the one asset most likely to be dropped into
        // someone else's page. `xmlns` is exempt and only that: the SVG spec
        // requires it as an identifier and nothing dereferences it.
        for svg in [render(Scheme::Light), render_mono("#000000"), render_tile(Scheme::Dark)] {
            let stripped = svg.replace(r#"xmlns="http://www.w3.org/2000/svg""#, "");

            for marker in ["http://", "https://", "<image", "xlink:href", "@import", "url("] {
                assert!(!stripped.contains(marker), "the mark reaches for {marker}");
            }
        }
    }

    #[test]
    fn the_svg_states_its_own_construction() {
        // The SVG is what someone opens when they want to know how the mark is
        // built, so the grid has to travel with the file rather than only
        // living in this module.
        let svg = render(Scheme::Light);

        assert!(svg.contains("One document, compiled into a sequence of pages"));
        assert!(svg.contains("module of 3"));
    }

    #[test]
    fn a_tile_keeps_the_mark_inside_every_maskable_safe_zone() {
        // A platform crops an app icon to its own shape. The mark has to be
        // inside 80% of the tile to survive that, and the inset is the brand's
        // clear space rather than a number picked to clear the bound.
        let geometry = Geometry::default();
        let side = geometry.grid + geometry.document_width * 2;
        let drawn = f64::from(geometry.grid) / f64::from(side);

        assert!(drawn <= 0.8, "the mark covers {drawn:.2} of the tile");
        assert!(render_tile(Scheme::Light).contains(&format!("viewBox=\"0 0 {side} {side}\"")));
    }

    #[test]
    fn a_tile_is_drawn_on_paper_so_a_crop_has_something_to_cut() {
        let tile = render_tile(Scheme::Light);

        assert!(tile.contains(&format!("fill=\"{}\"", palette::light().paper.to_hex())));
        assert_eq!(tile.matches("<rect").count(), 5, "a paper field and the four shapes");
    }

    #[test]
    fn the_same_scheme_always_draws_the_same_mark() {
        // The rasters are content-addressed by the icon script; a mark that
        // differed run to run would rewrite every PNG for nothing.
        assert_eq!(render(Scheme::Light), render(Scheme::Light));
        assert_eq!(render_tile(Scheme::Dark), render_tile(Scheme::Dark));
    }

    #[test]
    fn the_fills_a_caller_draws_with_are_the_ones_in_the_svg() {
        let (document, pages) = fills(Scheme::Dark);
        let svg = render(Scheme::Dark);

        assert!(svg.contains(&document.to_hex()));
        assert!(svg.contains(&pages.to_hex()));
    }
}
