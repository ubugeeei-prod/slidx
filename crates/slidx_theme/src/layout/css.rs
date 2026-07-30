//! Layouts as CSS.
//!
//! One rule per layout naming its grid, one rule per region naming its area, and
//! nothing else. The sizes are all `fr` and `auto`, so the whole thing resolves
//! against the slide — which is already a size container — and a region is a
//! share of the slide at every projector size, with no transform and no script.
//!
//! Generated rather than written by hand because it has to agree with
//! [`super::all`] exactly: a region whose area is missing from the template gets
//! placed in an implicit track, which looks like the layout works until a second
//! block lands there.

use super::{Layout, Region};

/// The attribute the renderer writes on a slide to select its layout.
pub const LAYOUT_ATTRIBUTE: &str = "data-slidx-layout";

/// The attribute the renderer writes on a region element.
pub const REGION_ATTRIBUTE: &str = "data-slidx-region";

/// Every built-in layout, as rules to inline into a page.
pub fn render(layouts: &[Layout]) -> String {
    let mut css = String::new();

    for layout in layouts {
        css.push_str(&one(layout));
    }

    css
}

fn one(layout: &Layout) -> String {
    let areas: String =
        layout.areas.iter().map(|row| format!("\n    \"{row}\"")).collect::<Vec<_>>().join("");

    let mut css = format!(
        "[{LAYOUT_ATTRIBUTE}=\"{id}\"] .slidx-slide-body {{\n  \
         grid-template-areas:{areas};\n  \
         grid-template-columns: {columns};\n  \
         grid-template-rows: {rows};\n}}\n",
        id = layout.id,
        areas = areas,
        columns = layout.columns,
        rows = layout.rows,
    );

    for region in &layout.regions {
        css.push_str(&area(layout, region));
    }

    css
}

fn area(layout: &Layout, region: &Region) -> String {
    format!(
        "[{LAYOUT_ATTRIBUTE}=\"{id}\"] > .slidx-slide-body > [{REGION_ATTRIBUTE}=\"{name}\"] \
         {{ grid-area: {name}; justify-content: {align}; }}\n",
        id = layout.id,
        name = region.name,
        align = match region.align {
            super::RegionAlign::Center => "center",
            super::RegionAlign::Start => "flex-start",
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout;

    fn css() -> String {
        render(&layout::all())
    }

    #[test]
    fn every_layout_gets_a_grid_and_every_region_gets_an_area() {
        let css = css();

        for layout in layout::all() {
            assert!(
                css.contains(&format!("[data-slidx-layout=\"{}\"] .slidx-slide-body", layout.id)),
                "{} has no grid",
                layout.id
            );

            for region in &layout.regions {
                assert!(
                    css.contains(&format!("grid-area: {};", region.name)),
                    "{}/{} has no area",
                    layout.id,
                    region.name
                );
            }
        }
    }

    #[test]
    fn a_split_layout_is_two_equal_columns() {
        let css = css();
        let at = css.find("[data-slidx-layout=\"split\"]").unwrap();
        let rule = &css[at..at + css[at..].find('}').unwrap()];

        assert!(rule.contains("\"left right\""));
        assert!(rule.contains("grid-template-columns: 1fr 1fr"));
    }

    #[test]
    fn a_title_band_takes_only_what_it_needs() {
        // `1fr 1fr` would put a one-line title in the top half of the slide and
        // push the body off centre.
        let css = css();
        let at = css.find("[data-slidx-layout=\"stack\"]").unwrap();

        assert!(css[at..].starts_with(
            "[data-slidx-layout=\"stack\"] .slidx-slide-body {\n  grid-template-areas:\n    \"title\"\n    \"body\";"
        ));
        assert!(css[at..].contains("grid-template-rows: auto 1fr;"));
    }

    #[test]
    fn a_region_that_wants_the_top_edge_says_so() {
        assert!(css().contains(
            "[data-slidx-layout=\"top\"] > .slidx-slide-body > [data-slidx-region=\"body\"] { grid-area: body; justify-content: flex-start; }"
        ));
    }

    #[test]
    fn every_track_a_layout_declares_is_a_share_rather_than_a_length() {
        // A region measured in pixels is a region that is the wrong size on the
        // next projector. `fr` and `auto` resolve against the slide, which is
        // what keeps the whole thing scaling as one piece.
        for layout in layout::all() {
            for track in layout.columns.split_whitespace().chain(layout.rows.split_whitespace()) {
                assert!(
                    track == "auto" || track.ends_with("fr"),
                    "{} declares the track {track}",
                    layout.id
                );
            }
        }
    }

    #[test]
    fn braces_balance() {
        assert_eq!(css().matches('{').count(), css().matches('}').count());
    }
}
