//! A slide body, rendered block by block into the regions its layout declares.
//!
//! Not to be confused with [`crate::layout`], which is the shell *stylesheet*.
//! This module is about slide layout: where on the slide a block ends up.
//!
//! # What the markup has to make true
//!
//! **A block is a box.** Each one gets a wrapper element carrying its index, so
//! the editor can measure a block, highlight it, and drop it somewhere without
//! guessing which rendered element came from which line of Markdown. A block that
//! did not lay out — `display: contents` — would measure as zero and be
//! invisible to every overlay.
//!
//! **A region is a box.** The regions are the direct children of the body, which
//! is the grid, so a region is one `grid-area` and the theme owns where it is.
//!
//! **Anchors stay with what they stage.** A step marker on its own line resolves
//! to the anchor's previous element sibling, so `slidx_core` keeps an
//! anchor-only chunk inside the block above it and the two land in the same
//! region however the block is moved.
//!
//! # Why per region rather than per block
//!
//! Markdown is parsed once per region rather than once per block: a footnote
//! definition, a reference link, and a loose list all mean something across
//! blocks, and rendering each block alone would break them for no gain. A region
//! is the smallest unit whose content is genuinely independent, because it is
//! laid out independently.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use slidx_core::{Block, Deck, Slide};
use slidx_theme::layout::{
    css::REGION_ATTRIBUTE, place, width, BlockWidth, Layout, REGION_NAMES, WIDTH_ATTRIBUTE,
};
use slidx_theme::Theme;

use crate::markdown::{render, MarkdownOptions};

/// Attribute carrying a block's index in source order.
pub const BLOCK_ATTRIBUTE: &str = "data-slidx-block";

/// The layout a slide renders with.
///
/// The resolution itself lives in `slidx_theme::layout`, because an edit
/// operation has to reach the same answer: a block dropped into a region this
/// slide does not have is a block that is then drawn somewhere else.
pub fn layout_of(slide: &Slide) -> Layout {
    slidx_theme::layout::of(slide)
}

/// The inner HTML of `.slidx-slide-body`, one element per region.
///
/// `appended` goes into the default region rather than beside the regions: a
/// direct child of the grid that no layout has an area for would be placed in an
/// implicit track, which moves everything else down.
pub fn body(
    deck: &Deck,
    slide: &Slide,
    layout: &Layout,
    theme: &Theme,
    options: &MarkdownOptions,
    sizes: &crate::intrinsic::Sizes,
    appended: &str,
) -> String {
    let placement = place(&slide.blocks, layout);
    let sources = block_sources(deck, slide, theme);
    let (components, component_blocks) = component_blocks(slide, options);
    let default = layout.fallback().name.clone();

    let regions: String = placement
        .regions
        .iter()
        .map(|region| {
            let blocks: String = region
                .blocks
                .iter()
                .filter_map(|at| sources.get(*at).map(|markdown| (*at, markdown)))
                .filter_map(|(at, markdown)| {
                    let block = &slide.blocks[at];
                    let share = width::of(block).ok();

                    if let Some(component) = components.get(&at) {
                        Some(wrap(at, block, share, &slide.style, component))
                    } else if component_blocks.contains(&at) {
                        None
                    } else {
                        Some(wrap(
                            at,
                            block,
                            share,
                            &slide.style,
                            &render(markdown, options),
                        ))
                    }
                })
                .collect();

            let extra = if region.region.name == default { appended } else { "" };

            format!(
                "      <div class=\"slidx-region\" {REGION_ATTRIBUTE}=\"{name}\">\n{blocks}{extra}      </div>\n",
                name = region.region.name,
            )
        })
        .collect::<String>();

    // Last, and over the whole body: the build measured every image the deck
    // references, and a browser that does not know an image's ratio reflows the
    // slide when it lands. See `crate::intrinsic`.
    crate::intrinsic::size_images(&regions, sizes)
}

/// Maps a flow component to the first Markdown block it occupies and remembers
/// the later blocks its fallback consumed.
///
/// Later ordinary blocks keep their original indices. That is the property the
/// visual editor relies on when it selects a block after an MDX component and
/// sends an operation back against the untouched `.mdx` source.
fn component_blocks(
    slide: &Slide,
    options: &MarkdownOptions,
) -> (BTreeMap<usize, String>, BTreeSet<usize>) {
    if !options.mdx {
        return (BTreeMap::new(), BTreeSet::new());
    }

    let mut components = BTreeMap::new();
    let mut consumed = BTreeSet::new();

    for component in crate::mdx::flow_replacements(&slide.content, options) {
        let occupied: Vec<usize> = slide
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| {
                block.span.start < component.end && block.span.end > component.start
            })
            .map(|(index, _)| index)
            .collect();
        let Some((&first, rest)) = occupied.split_first() else {
            continue;
        };

        components.insert(first, component.value);
        consumed.extend(rest);
    }

    (components, consumed)
}

/// One block's box, carrying its index and the width it asks for.
///
/// The content-sized default writes no attribute, the same way a block in the
/// default region writes no class. Every fixed share, including `full`, stays
/// explicit so the editor reads absence as `fit` without guessing.
fn wrap(
    index: usize,
    block: &Block,
    width: Option<BlockWidth>,
    style: &BTreeMap<String, String>,
    html: &str,
) -> String {
    let classes = block
        .attributes
        .classes
        .iter()
        // A region is placement, not presentation. Filtering the vocabulary
        // across every layout also keeps a stale region name from becoming a
        // style merely because the current layout does not offer it.
        .filter(|class| !REGION_NAMES.contains(&class.as_str()))
        .map(|class| format!(" slidx-{class}"))
        .collect::<String>();

    let width = match width.filter(|width| *width != BlockWidth::Fit) {
        Some(width) => format!(" {WIDTH_ATTRIBUTE}=\"{}\"", width.as_token()),
        None => String::new(),
    };
    let visual = visual_attributes(block, style);

    // The rendered Markdown is emitted verbatim. Indenting it would indent the
    // inside of every `<pre>`, where whitespace is content.
    format!(
        "        <div class=\"slidx-block{classes}\" {BLOCK_ATTRIBUTE}=\"{index}\"{width}{visual}>\n{html}\n        </div>\n"
    )
}

fn visual_attributes(block: &Block, style: &BTreeMap<String, String>) -> String {
    let Some(key) = block.attributes.key.as_deref() else {
        return String::new();
    };
    let prefix = slidx_core::style::block_style_prefix(key);
    let properties: Vec<_> = style
        .keys()
        .filter_map(|name| name.strip_prefix(&prefix).map(|property| (name, property)))
        .filter(|(_, property)| !property.is_empty())
        .collect();

    let mut attributes = format!(" data-slidx-key=\"{}\"", escape_attribute(key));
    if properties.is_empty() {
        return attributes;
    }

    if properties
        .iter()
        .any(|(_, property)| matches!(*property, "x" | "y" | "width" | "height" | "inset"))
    {
        attributes.push_str(" data-slidx-freeform");
    }
    if properties.iter().any(|(_, property)| *property == "inset") {
        attributes.push_str(" data-slidx-freeform-frame");
    }
    if properties.iter().any(|(_, property)| *property == "color") {
        attributes.push_str(" data-slidx-element-color");
    }

    attributes.push_str(" style=\"");
    for (name, property) in properties {
        let _ = write!(attributes, "--slidx-element-{property}: var(--slidx-{name}); ");
    }
    if attributes.ends_with(' ') {
        attributes.pop();
    }
    attributes.push('"');

    attributes
}

fn escape_attribute(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Each block's Markdown, with any shared-code figure that belongs to it.
///
/// The figures are placed here rather than by [`crate::snippet::stage`] because a
/// block that moves region has to take its code with it. Their offsets are into
/// the slide's own content, which is what the block spans are measured in.
fn block_sources(deck: &Deck, slide: &Slide, theme: &Theme) -> Vec<String> {
    let figures = crate::snippet::figures(deck, slide, theme);

    let mut sources: Vec<String> = slide
        .blocks
        .iter()
        .map(|block: &Block| block.span.slice(&slide.content).to_string())
        .collect();

    for figure in figures {
        // The last block that begins before the code does. A figure sits just
        // past the fence it belongs to, which is inside or at the end of that
        // block's span.
        let owner = slide.blocks.iter().rposition(|block| block.span.start < figure.after);

        match owner.and_then(|at| sources.get_mut(at)) {
            Some(source) => source.push_str(&figure.html),
            // No block to attach to, which means the slide is nothing but a
            // fence the parser could not close. The code still ships.
            None => sources.push(figure.html),
        }
    }

    sources
}

#[cfg(test)]
mod tests {
    use super::*;
    use slidx_core::{parse_deck, DeckParseOptions};

    fn rendered(source: &str) -> String {
        let deck = parse_deck(source, &DeckParseOptions::default());
        let slide = &deck.slides[0];

        body(
            &deck,
            slide,
            &layout_of(slide),
            &slidx_theme::default_theme(),
            &MarkdownOptions::default(),
            &crate::intrinsic::Sizes::new(),
            "",
        )
    }

    /// What is inside one region of the rendered body.
    fn region(html: &str, name: &str) -> String {
        let open = format!("{REGION_ATTRIBUTE}=\"{name}\"");
        let at = html.find(&open).unwrap_or_else(|| panic!("no {name} region in {html}"));
        let rest = &html[at..];

        rest[..rest.find("</div>\n      </div>").unwrap_or(rest.len())].to_string()
    }

    #[test]
    fn a_slide_that_places_nothing_puts_everything_in_the_default_region() {
        let html = rendered("# One\n\n- a\n- b\n");

        assert!(html.contains("data-slidx-region=\"body\""));
        assert!(html.contains("<h1"));
        assert!(html.contains("<li>a</li>"));
    }

    #[test]
    fn a_block_that_names_a_region_is_rendered_inside_it() {
        let html = rendered("---\nlayout: split\n---\n\n# Left\n\n{.right}\nBeside it.\n");

        assert!(region(&html, "left").contains("<h1"));
        assert!(region(&html, "right").contains("Beside it."));
        assert!(!region(&html, "left").contains("Beside it."));
        assert!(!html.contains("slidx-right"), "placement leaked into styling: {html}");
    }

    #[test]
    fn a_block_style_reaches_the_same_theme_class_as_an_inline_mark() {
        let html = rendered("{.accent .muted}\n# Styled\n");

        assert!(
            html.contains("class=\"slidx-block slidx-accent slidx-muted\""),
            "classes were dropped: {html}"
        );
    }

    #[test]
    fn managed_block_styles_bind_to_the_addressed_rendered_box() {
        let html = rendered(concat!(
            "<style data-slidx>\n",
            ":root {\n",
            "  --slidx-block-id-hero-x: 12%;\n",
            "  --slidx-block-id-hero-width: 40%;\n",
            "  --slidx-block-id-hero-inset: 12% 48% 58% 10%;\n",
            "  --slidx-block-id-hero-color: var(--slidx-color-accent);\n",
            "}\n",
            "</style>\n",
            "\n",
            "{#hero}\n",
            "# Styled\n",
        ));

        assert!(html.contains("data-slidx-key=\"hero\""));
        assert!(html.contains("data-slidx-freeform"));
        assert!(html.contains("data-slidx-freeform-frame"));
        assert!(html.contains("data-slidx-element-color"));
        assert!(html.contains("--slidx-element-x: var(--slidx-block-id-hero-x)"));
        assert!(html.contains("--slidx-element-width: var(--slidx-block-id-hero-width)"));
        assert!(html.contains("--slidx-element-inset: var(--slidx-block-id-hero-inset)"));
        assert!(html.contains("--slidx-element-color: var(--slidx-block-id-hero-color)"));
        assert!(!html.contains("<style data-slidx>"));
    }

    #[test]
    fn a_region_with_nothing_in_it_is_still_in_the_markup() {
        // The editor draws the grid an author is dropping into, and a region that
        // only exists once something is in it is a region nobody can aim at.
        let html = rendered("---\nlayout: quad\n---\n\n# One\n");

        for name in ["top-left", "top-right", "bottom-left", "bottom-right"] {
            assert!(html.contains(&format!("data-slidx-region=\"{name}\"")), "no {name}");
        }
    }

    #[test]
    fn every_block_carries_its_index_in_source_order() {
        let html = rendered("# One\n\nSecond.\n\nThird.\n");

        for index in 0..3 {
            assert!(html.contains(&format!("data-slidx-block=\"{index}\"")), "no block {index}");
        }
    }

    #[test]
    fn a_flow_mdx_component_spans_blocks_without_renumbering_what_follows() {
        let source =
            "# One\n\n<Counter start={1}>\n\n**fallback**\n\n</Counter>\n\nAfter the island.\n";
        let deck = parse_deck(source, &DeckParseOptions::default());
        let slide = &deck.slides[0];
        let options = MarkdownOptions { mdx: true, ..MarkdownOptions::default() };
        let html = body(
            &deck,
            slide,
            &layout_of(slide),
            &slidx_theme::default_theme(),
            &options,
            &crate::intrinsic::Sizes::new(),
            "",
        );

        assert!(html.contains("data-slidx-island=\"Counter\""), "{html}");
        assert!(html.contains("<strong>fallback</strong>"), "{html}");
        assert!(html.contains("data-slidx-block=\"4\""), "{html}");
        assert!(html.contains("After the island."), "{html}");
    }

    #[test]
    fn a_block_that_names_a_share_of_its_region_carries_it_onto_the_page() {
        let html = rendered("---\nlayout: aside\n---\n\n{.side width=half}\n![D](./a.svg)\n");

        assert!(html.contains("data-slidx-width=\"half\""), "{html}");
    }

    #[test]
    fn a_block_that_takes_its_whole_region_says_so_explicitly() {
        let html = rendered("# One\n\n{width=full}\nSecond.\n");

        assert!(html.contains("data-slidx-width=\"full\""), "{html}");
    }

    #[test]
    fn a_block_with_no_width_attribute_uses_the_content_sized_default() {
        let html = rendered("# One\n\nSecond.\n");

        assert!(!html.contains("data-slidx-width"), "{html}");
    }

    #[test]
    fn an_explicit_fit_token_is_canonicalised_to_the_missing_attribute() {
        let html = rendered("# One\n\n{width=fit}\nSecond.\n");

        assert!(!html.contains("data-slidx-width"), "{html}");
    }

    #[test]
    fn a_width_that_is_not_a_share_leaves_the_block_at_the_safe_default() {
        // A pixel is refused by the vocabulary and reported by the linter. The
        // slide still renders, because a slide that lost a block over a typo is
        // worse than one that safely ignores the typo.
        let html = rendered("# One\n\n{width=340px}\nSecond.\n");

        assert!(!html.contains("data-slidx-width"), "{html}");
        assert!(html.contains("Second."));
    }

    #[test]
    fn a_block_that_names_a_region_the_layout_lacks_still_renders() {
        // Dropping it would punish an author on stage for changing `layout:` and
        // leaving one class behind.
        let html = rendered("---\nlayout: split\n---\n\n{.side}\n# Stranded\n");

        assert!(region(&html, "left").contains("Stranded"));
        assert!(!html.contains("slidx-side"), "a stale placement became styling: {html}");
    }

    #[test]
    fn a_code_fence_keeps_its_indentation() {
        // Pretty-printing the body would indent the inside of every `<pre>`,
        // where whitespace is content: every code block on every slide would gain
        // a phantom indent.
        let html = rendered("```rust\nfn main() {\n    let x = 1;\n}\n```\n");
        let block = html.split_once("<code class=\"language-rust\">").expect("a block").1;

        assert!(block.contains("\n    "), "the indent was reflowed:\n{block}");
    }

    #[test]
    fn a_step_anchor_stays_next_to_the_block_it_stages() {
        // The runtime resolves a block anchor to the previous element sibling, so
        // an anchor that ended up in another region would stage the wrong thing —
        // or nothing.
        let deck = parse_deck(
            "---\nlayout: split\nautoSteps: block\n---\n\n{.right}\n# Placed\n",
            &DeckParseOptions::default(),
        );
        let slide = &deck.slides[0];
        let html = body(
            &deck,
            slide,
            &layout_of(slide),
            &slidx_theme::default_theme(),
            &MarkdownOptions::default(),
            &crate::intrinsic::Sizes::new(),
            "",
        );

        let right = region(&html, "right");
        assert!(right.contains("<h1"), "the heading left its region: {html}");
        assert!(right.contains("data-slidx-step"), "the anchor left its block: {html}");
    }

    #[test]
    fn a_shared_block_takes_its_code_into_the_region_it_moved_to() {
        // A QR on one half of the slide for a block on the other is a code the
        // audience cannot connect to anything.
        let source = concat!(
            "---\nurl: https://example.com/talk/\nlayout: split\n---\n\n",
            "# Two halves\n\n{.right}\n```rust {#retry .share}\nfn retry() {}\n```\n",
        );
        let deck = parse_deck(source, &DeckParseOptions::default());
        let slide = &deck.slides[0];
        let html = body(
            &deck,
            slide,
            &layout_of(slide),
            &slidx_theme::default_theme(),
            &MarkdownOptions::default(),
            &crate::intrinsic::Sizes::new(),
            "",
        );

        assert!(region(&html, "right").contains("snippets/retry.html"));
        assert!(!region(&html, "left").contains("snippets/retry.html"));
    }

    #[test]
    fn appended_markup_goes_into_the_default_region() {
        // A direct child of the grid with no area lands in an implicit track,
        // which pushes every region down by one row.
        let deck = parse_deck("---\nlayout: split\n---\n\n# One\n", &DeckParseOptions::default());
        let slide = &deck.slides[0];
        let html = body(
            &deck,
            slide,
            &layout_of(slide),
            &slidx_theme::default_theme(),
            &MarkdownOptions::default(),
            &crate::intrinsic::Sizes::new(),
            "      <figure class=\"slidx-demo\"></figure>\n",
        );

        assert!(region(&html, "left").contains("slidx-demo"));
        assert!(!region(&html, "right").contains("slidx-demo"));
    }

    #[test]
    fn an_unknown_layout_name_renders_with_the_default() {
        let deck =
            parse_deck("---\nlayout: nonsense\n---\n\n# One\n", &DeckParseOptions::default());
        assert_eq!(layout_of(&deck.slides[0]).id, "full");
    }

    #[test]
    fn an_empty_slide_renders_an_empty_region_rather_than_nothing() {
        let html = rendered("");

        assert!(html.contains("data-slidx-region=\"body\""));
        assert!(!html.contains("data-slidx-block"));
    }
}
