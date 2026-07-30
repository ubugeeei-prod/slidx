//! Layouts: the named regions a block can be placed into.
//!
//! # Why a region and not a rectangle
//!
//! The obvious way to let an author move something is a freeform canvas, and
//! the result is four floats in the file. Nobody can review them, they mean a
//! different thing at a different aspect ratio, and no rule can reason about
//! them — the linter cannot say whether text will be legible in a box whose
//! width it only learns at runtime.
//!
//! A region is the same gesture with a name. `{.side}` is reviewable, survives a
//! 4:3 projector, and is something a rule can measure, because the geometry
//! belongs to the layout rather than to the slide.
//!
//! # Why a grid
//!
//! A layout is one `grid-template-areas` and each region is one `grid-area`. The
//! slide is already a size container, so a grid inside it inherits the scaling
//! for free: every region is a share of the slide, at every projector size, with
//! no transform and no script.
//!
//! **A region's name is its grid area.** One string describes the geometry
//! rather than two that can disagree, which is why names have to be CSS
//! identifiers and why [`REGION_NAMES`] can be checked against what the layouts
//! actually declare.
//!
//! # Where layouts live
//!
//! Here rather than on [`Theme`](crate::Theme): all four built-in themes want
//! the same geometry, and a per-theme field nothing varies would be a promise
//! about extensibility that no theme yet keeps. When a theme package needs its
//! own, this is the vocabulary it will override.

use serde::{Deserialize, Serialize};

pub mod css;
mod place;

pub use css::{render as css, LAYOUT_ATTRIBUTE, REGION_ATTRIBUTE};
pub use place::{diagnose, place, Misplaced, PlacedRegion, Placement};

/// How content sits in a region when it does not fill it.
///
/// A slide is a frame, not a page: content pinned to the top of an otherwise
/// empty frame reads as unfinished, so centred is the default. `Start` is for
/// the regions where the top edge is the point — a title band, or a long list
/// that would otherwise straddle the centre line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegionAlign {
    Center,
    Start,
}

impl RegionAlign {
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::Start => "start",
        }
    }
}

/// One named area of a slide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    /// The name an author writes as a class, and the layout's grid area.
    pub name: String,
    /// One line on what belongs here, shown in the editor's region list.
    pub summary: String,
    pub align: RegionAlign,
}

impl Region {
    fn new(name: &str, summary: &str, align: RegionAlign) -> Self {
        Self { name: name.into(), summary: summary.into(), align }
    }

    fn centred(name: &str, summary: &str) -> Self {
        Self::new(name, summary, RegionAlign::Center)
    }
}

/// A named set of regions and the grid that places them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// What an author writes as `layout:`.
    pub id: String,
    /// One line on when to reach for this layout.
    pub summary: String,
    /// `grid-template-areas`, one entry per row, area names only.
    pub areas: Vec<String>,
    pub columns: String,
    pub rows: String,
    pub regions: Vec<Region>,
    /// Where a block that names no region goes.
    ///
    /// Every layout has one, because a slide whose blocks say nothing is the
    /// common case and it has to render somewhere.
    pub default_region: String,
}

impl Layout {
    pub fn region(&self, name: &str) -> Option<&Region> {
        self.regions.iter().find(|region| region.name == name)
    }

    pub fn has_region(&self, name: &str) -> bool {
        self.region(name).is_some()
    }

    /// The region a block with nothing to say about placement lands in.
    pub fn fallback(&self) -> &Region {
        self.region(&self.default_region)
            .or_else(|| self.regions.first())
            .expect("a layout declares at least one region")
    }

    /// Region names in the order they are offered to an author.
    pub fn region_names(&self) -> Vec<&str> {
        self.regions.iter().map(|region| region.name.as_str()).collect()
    }
}

/// Every region name any built-in layout uses.
///
/// The reason this is written down as well as derived: a class that names one of
/// these but is not a region of *this* slide's layout is an author who changed
/// `layout:` and left a block behind, and that is worth a diagnostic. A class
/// that names none of them is a theme style class such as `.accent`, and nothing
/// may be said about it — a warning on every styled block would be a warning
/// nobody keeps switched on.
///
/// `every_region_name_is_declared` checks this against [`all`], so a region
/// added to a layout cannot go unlisted here.
pub const REGION_NAMES: &[&str] = &[
    "body",
    "title",
    "left",
    "right",
    "main",
    "side",
    "top-left",
    "top-right",
    "bottom-left",
    "bottom-right",
];

/// Every built-in layout, in the order they are offered to an author.
pub fn all() -> Vec<Layout> {
    vec![full(), top(), stack(), split(), aside(), quad()]
}

/// Resolves a layout by the name an author wrote.
///
/// `None` rather than a silent fallback, for the same reason a theme resolves
/// that way: a typo in `layout:` should be reported instead of producing a slide
/// that looks subtly unlike what was asked for.
pub fn find(id: &str) -> Option<Layout> {
    all().into_iter().find(|layout| layout.id == id)
}

/// The layout a slide gets when it names none.
pub fn default_layout() -> Layout {
    full()
}

/// Names an author may write, for completion and for diagnostics.
pub fn names() -> Vec<String> {
    all().into_iter().map(|layout| layout.id).collect()
}

/// One region, filling the slide. The default.
fn full() -> Layout {
    Layout {
        id: "full".into(),
        summary: "One region, the whole slide. The default.".into(),
        areas: vec!["body".into()],
        columns: "1fr".into(),
        rows: "1fr".into(),
        regions: vec![Region::centred("body", "Everything on the slide.")],
        default_region: "body".into(),
    }
}

/// One region, content against the top edge.
fn top() -> Layout {
    Layout {
        id: "top".into(),
        summary: "One region, content against the top edge rather than centred.".into(),
        areas: vec!["body".into()],
        columns: "1fr".into(),
        rows: "1fr".into(),
        regions: vec![Region::new(
            "body",
            "Everything on the slide, from the top down.",
            RegionAlign::Start,
        )],
        default_region: "body".into(),
    }
}

/// A title band above the body.
fn stack() -> Layout {
    Layout {
        id: "stack".into(),
        summary: "A title band above the body.".into(),
        areas: vec!["title".into(), "body".into()],
        columns: "1fr".into(),
        // The band takes what its content needs and no more, so a one-line title
        // does not push the body off centre.
        rows: "auto 1fr".into(),
        regions: vec![
            Region::new("title", "The heading band.", RegionAlign::Start),
            Region::centred("body", "Everything under the title."),
        ],
        default_region: "body".into(),
    }
}

/// Two equal columns.
fn split() -> Layout {
    Layout {
        id: "split".into(),
        summary: "Two equal columns.".into(),
        areas: vec!["left right".into()],
        columns: "1fr 1fr".into(),
        rows: "1fr".into(),
        regions: vec![
            Region::centred("left", "The left column."),
            Region::centred("right", "The right column."),
        ],
        default_region: "left".into(),
    }
}

/// A wide main column and a narrow side.
fn aside() -> Layout {
    Layout {
        id: "aside".into(),
        summary: "A wide main column and a narrow side.".into(),
        areas: vec!["main side".into()],
        // Two to one: narrower than this and the side cannot hold a readable
        // line, which is the failure a layout with a sidebar usually ships with.
        columns: "2fr 1fr".into(),
        rows: "1fr".into(),
        regions: vec![
            Region::centred("main", "The main column."),
            Region::centred("side", "A narrow column for an image, a code, or a caption."),
        ],
        default_region: "main".into(),
    }
}

/// Four quarters.
fn quad() -> Layout {
    Layout {
        id: "quad".into(),
        summary: "Four quarters.".into(),
        areas: vec!["top-left top-right".into(), "bottom-left bottom-right".into()],
        columns: "1fr 1fr".into(),
        rows: "1fr 1fr".into(),
        regions: vec![
            Region::centred("top-left", "The top left quarter."),
            Region::centred("top-right", "The top right quarter."),
            Region::centred("bottom-left", "The bottom left quarter."),
            Region::centred("bottom-right", "The bottom right quarter."),
        ],
        default_region: "top-left".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layouts_the_roadmap_names_are_all_present() {
        for id in ["full", "split", "aside", "stack", "quad"] {
            assert!(find(id).is_some(), "{id} does not resolve");
        }
    }

    #[test]
    fn a_typo_in_a_layout_name_does_not_silently_fall_back() {
        assert!(find("splitt").is_none());
    }

    #[test]
    fn the_default_layout_is_one_region_covering_the_slide() {
        let layout = default_layout();

        assert_eq!(layout.regions.len(), 1);
        assert_eq!(layout.fallback().name, "body");
    }

    #[test]
    fn every_layout_declares_a_default_region_that_it_has() {
        // A block that names no region has to land somewhere, and a default
        // pointing at a region the layout lacks would drop it.
        for layout in all() {
            assert!(
                layout.has_region(&layout.default_region),
                "{} defaults to a region it does not have",
                layout.id
            );
        }
    }

    #[test]
    fn every_region_appears_exactly_once_in_its_layouts_grid() {
        // The region's name *is* its grid area. A region missing from the
        // template is a region with no geometry, and CSS would silently place
        // its content in an implicit track.
        for layout in all() {
            let areas = layout.areas.join(" ");
            let cells: Vec<&str> = areas.split_whitespace().collect();

            for region in &layout.regions {
                assert!(
                    cells.contains(&region.name.as_str()),
                    "{} has no cell for {}",
                    layout.id,
                    region.name
                );
            }

            for cell in cells {
                assert!(
                    layout.has_region(cell),
                    "{} names an area {cell} it has no region for",
                    layout.id
                );
            }
        }
    }

    #[test]
    fn every_grid_row_has_the_same_number_of_cells() {
        // `grid-template-areas` with ragged rows is invalid and the whole
        // declaration is dropped, which loses every region at once.
        for layout in all() {
            let widths: Vec<usize> =
                layout.areas.iter().map(|row| row.split_whitespace().count()).collect();

            assert!(
                widths.windows(2).all(|pair| pair[0] == pair[1]),
                "{} has ragged rows: {widths:?}",
                layout.id
            );
            assert_eq!(
                widths[0],
                layout.columns.split_whitespace().count(),
                "{} declares a different number of columns than areas",
                layout.id
            );
            assert_eq!(
                layout.areas.len(),
                layout.rows.split_whitespace().count(),
                "{} declares a different number of rows than areas",
                layout.id
            );
        }
    }

    #[test]
    fn every_region_name_is_a_css_identifier() {
        // It is written into `grid-template-areas` verbatim. Anything else makes
        // the declaration invalid, and an invalid one is dropped entirely.
        for layout in all() {
            for region in &layout.regions {
                let first = region.name.chars().next().unwrap();

                assert!(first.is_ascii_alphabetic(), "{} is not an identifier", region.name);
                assert!(
                    region.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
                    "{} is not an identifier",
                    region.name
                );
            }
        }
    }

    #[test]
    fn every_region_name_is_declared() {
        // The list a diagnostic reads is derived from the layouts, checked here
        // rather than trusted: a region added to a layout and missing from the
        // list would be a misplaced block nobody is told about.
        let mut declared: Vec<String> = all()
            .iter()
            .flat_map(|layout| layout.region_names().into_iter().map(String::from))
            .collect();
        declared.sort();
        declared.dedup();

        let mut listed: Vec<String> = REGION_NAMES.iter().map(|name| name.to_string()).collect();
        listed.sort();

        assert_eq!(declared, listed);
    }

    #[test]
    fn no_region_name_collides_with_a_theme_style_class() {
        // `{.accent}` styles a block and `{.side}` places one, and the renderer
        // tells them apart by name. A class that meant both would place a block
        // the author only wanted coloured.
        for style in ["accent", "muted", "code", "share"] {
            assert!(
                !REGION_NAMES.contains(&style),
                "{style} is both a style class and a region name"
            );
        }
    }

    #[test]
    fn every_layout_is_described_in_its_own_words() {
        for layout in all() {
            assert!(!layout.summary.is_empty(), "{} has no summary", layout.id);
            for region in &layout.regions {
                assert!(!region.summary.is_empty(), "{}/{} has none", layout.id, region.name);
            }
        }
    }
}
