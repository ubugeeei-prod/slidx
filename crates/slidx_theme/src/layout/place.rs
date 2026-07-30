//! Which region each block of a slide belongs to.
//!
//! This is the one question that needs both halves: a block and its attributes
//! come from `slidx_core`, and the regions come from a layout, which is the
//! theme's. Answering it here is what lets `slidx_core` stay ignorant of themes
//! and the renderer stay ignorant of everything except how to emit a grid.
//!
//! A block names its region with a class, because that is the notation an author
//! already writes for a mark and there is no reason for a second one. The
//! ambiguity that follows — `{.side}` places, `{.accent}` styles — is resolved by
//! name: a class that is a region of this slide's layout places the block, and
//! anything else is left alone.
//!
//! The interesting case is in between. `{.side}` on a `split` slide names a
//! region that exists in `aside` and not here, which is what an author who
//! changed `layout:` and forgot a block produces. That is worth naming, and it
//! is the reason [`REGION_NAMES`] is a list rather than a per-layout lookup.

use slidx_core::{Block, Deck, Diagnostic, Diagnostics, Slide, SourceSpan};

use super::{Layout, Region, REGION_NAMES};

/// One region and the blocks that landed in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedRegion {
    pub region: Region,
    /// Indices into the slide's blocks, in source order.
    pub blocks: Vec<usize>,
}

/// A block that asked for a region this layout does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Misplaced {
    /// Index into the slide's blocks.
    pub block: usize,
    /// The region name the block asked for.
    pub requested: String,
}

/// Where every block of a slide goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// Regions in the layout's declared order. Empty regions are included, so a
    /// caller can draw the grid an author is dropping into.
    pub regions: Vec<PlacedRegion>,
    pub misplaced: Vec<Misplaced>,
}

impl Placement {
    /// The blocks that landed in one region, in source order.
    pub fn blocks_in(&self, name: &str) -> &[usize] {
        self.regions
            .iter()
            .find(|placed| placed.region.name == name)
            .map_or(&[], |placed| &placed.blocks)
    }
}

/// Assigns each block to a region of `layout`.
///
/// A block naming no region goes to the layout's default, in source order. A
/// block naming a region the layout does not have goes there too — it still
/// renders, because a slide that lost a block over a stale class is worse than
/// one that shows it in the wrong place — and is reported in `misplaced`.
pub fn place(blocks: &[Block], layout: &Layout) -> Placement {
    let mut regions: Vec<PlacedRegion> = layout
        .regions
        .iter()
        .map(|region| PlacedRegion { region: region.clone(), blocks: Vec::new() })
        .collect();

    let mut misplaced = Vec::new();
    let default_at =
        regions.iter().position(|placed| placed.region.name == layout.fallback().name).unwrap_or(0);

    for (index, block) in blocks.iter().enumerate() {
        let named = block.attributes.classes.iter().find_map(|class| {
            let at = regions.iter().position(|placed| placed.region.name == *class);
            match at {
                Some(at) => Some(Ok(at)),
                // Only a name some layout uses. Anything else is a style class,
                // and a diagnostic on `.accent` would be a diagnostic on styling.
                None if REGION_NAMES.contains(&class.as_str()) => Some(Err(class.clone())),
                None => None,
            }
        });

        let at = match named {
            Some(Ok(at)) => at,
            Some(Err(requested)) => {
                misplaced.push(Misplaced { block: index, requested });
                default_at
            }
            None => default_at,
        };

        if let Some(placed) = regions.get_mut(at) {
            placed.blocks.push(index);
        }
    }

    Placement { regions, misplaced }
}

/// What is wrong with a deck's placements, in the author's words.
///
/// Separate from [`place`] because the renderer wants the assignment and the
/// build wants the report, and computing one from the other twice is cheaper
/// than threading diagnostics through every rendering signature.
pub fn diagnose(deck: &Deck) -> Diagnostics {
    let mut diagnostics = Diagnostics::default();

    for slide in &deck.slides {
        let Some(layout) = resolve(slide, &mut diagnostics) else { continue };
        let placement = place(&slide.blocks, &layout);

        for misplaced in placement.misplaced {
            diagnostics.push(
                Diagnostic::warning(
                    "layout/no-such-region",
                    format!(
                        "`{}` has no `{}` region, so the block is in `{}` instead",
                        layout.id, misplaced.requested, layout.default_region
                    ),
                )
                .at(at(slide))
                .with_help(format!(
                    "`{}` offers {}",
                    layout.id,
                    quoted(&layout.region_names())
                )),
            );
        }
    }

    diagnostics
}

/// The layout a slide asked for, reporting a name that resolves to nothing.
///
/// Returns the default for an unknown name rather than nothing at all: the slide
/// still renders, and its blocks are still worth checking against the regions it
/// will actually get.
fn resolve(slide: &Slide, diagnostics: &mut Diagnostics) -> Option<Layout> {
    let Some(written) = slide.layout.as_deref() else { return Some(super::default_layout()) };

    match super::find(written) {
        Some(layout) => Some(layout),
        None => {
            diagnostics.push(
                Diagnostic::warning(
                    "layout/unknown",
                    format!("there is no layout called `{written}`, so the slide uses `full`"),
                )
                .at(at(slide))
                .with_help(format!("the layouts are {}", quoted(&super::names()))),
            );

            Some(super::default_layout())
        }
    }
}

fn at(slide: &Slide) -> SourceSpan {
    SourceSpan::line(slide.source_line).on_slide(slide.index)
}

fn quoted<S: AsRef<str>>(names: &[S]) -> String {
    names.iter().map(|name| format!("`{}`", name.as_ref())).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn deck(source: &str) -> Deck {
        parse_deck(source, &DeckParseOptions::default())
    }

    fn placed(source: &str, layout: &str) -> Vec<(String, Vec<String>)> {
        let deck = deck(source);
        let slide = &deck.slides[0];
        let placement = place(&slide.blocks, &super::super::find(layout).unwrap());

        placement
            .regions
            .into_iter()
            .map(|region| {
                let blocks = region
                    .blocks
                    .into_iter()
                    .map(|at| slide.blocks[at].span.slice(&slide.content).to_string())
                    .collect();

                (region.region.name, blocks)
            })
            .collect()
    }

    #[test]
    fn a_block_that_names_no_region_goes_to_the_default_in_source_order() {
        let regions = placed("# One\n\nSecond.\n\nThird.\n", "split");

        assert_eq!(
            regions[0],
            ("left".into(), vec!["# One".into(), "Second.".into(), "Third.".into()])
        );
        assert_eq!(regions[1], ("right".into(), Vec::<String>::new()));
    }

    #[test]
    fn a_block_that_names_a_region_lands_in_it() {
        let regions = placed("# One\n\n{.right}\nBeside it.\n", "split");

        assert_eq!(regions[0].1, vec!["# One".to_string()]);
        assert_eq!(regions[1].1, vec!["Beside it.".to_string()]);
    }

    #[test]
    fn a_style_class_is_not_a_region_and_is_said_nothing_about() {
        // `.accent` is the theme's. A rule that treated every class as a
        // placement attempt would warn on every styled block in every deck.
        let deck = deck("{.accent}\n# One\n");
        let placement = place(&deck.slides[0].blocks, &super::super::find("split").unwrap());

        assert!(placement.misplaced.is_empty());
        assert_eq!(placement.blocks_in("left"), [0]);
    }

    #[test]
    fn a_region_this_layout_does_not_have_still_renders_and_is_reported() {
        // The author changed `layout:` and left a block behind. Dropping the
        // block would be the tool punishing them on stage for a stale class.
        let deck = deck("---\nlayout: split\n---\n\n{.side}\n# One\n");
        let placement = place(&deck.slides[0].blocks, &super::super::find("split").unwrap());

        assert_eq!(placement.blocks_in("left"), [0], "the block still renders");
        assert_eq!(placement.misplaced.len(), 1);
        assert_eq!(placement.misplaced[0].requested, "side");
    }

    #[test]
    fn the_diagnostic_names_the_regions_the_layout_does_have() {
        let diagnostics = diagnose(&deck("---\nlayout: split\n---\n\n{.side}\n# One\n"));
        let first = diagnostics.iter().find(|d| d.code == "layout/no-such-region").unwrap();

        assert!(first.message.contains("`side`"), "got: {}", first.message);
        assert!(first.help.as_ref().unwrap().contains("`right`"), "got: {:?}", first.help);
    }

    #[test]
    fn a_placement_problem_never_blocks_a_build() {
        // A deck edited minutes before a talk renders whatever it can.
        let diagnostics = diagnose(&deck("---\nlayout: split\n---\n\n{.side}\n# One\n"));
        assert!(!diagnostics.has_blocking());
    }

    #[test]
    fn a_layout_name_that_resolves_to_nothing_is_reported_rather_than_absorbed() {
        // The bug this whole feature started from: `layout:` was documented,
        // parsed, and changed nothing, so a name nobody implemented looked
        // exactly like one that worked.
        let diagnostics = diagnose(&deck("---\nlayout: statement\n---\n\n# One\n"));
        let first = diagnostics.iter().find(|d| d.code == "layout/unknown").unwrap();

        assert!(first.message.contains("`statement`"));
        assert!(first.help.as_ref().unwrap().contains("`aside`"));
    }

    #[test]
    fn a_deck_that_places_nothing_reports_nothing() {
        assert!(diagnose(&deck("# One\n\n- a\n- b\n")).is_empty());
    }

    #[test]
    fn a_diagnostic_points_at_the_slide_it_is_about() {
        let diagnostics = diagnose(&deck("# One\n\n---\nlayout: nope\n---\n\n# Two\n"));
        let first = &diagnostics.as_slice()[0];

        assert_eq!(first.span.slide_index, Some(1));
        assert!(first.span.line > 1, "the jump goes to the second slide");
    }
}
