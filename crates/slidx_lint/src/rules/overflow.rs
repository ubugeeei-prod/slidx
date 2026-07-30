//! Content that does not survive the trip to the wall.
//!
//! The slide is a design box with `overflow: hidden`. Anything outside it is
//! gone with no error anywhere — on the author's screen as much as in the room,
//! which is why nobody notices until a photograph of the slide turns up with
//! the last bullet missing. Two things put content outside that box: the room
//! takes an edge the deck never gets back, or the slide holds more than the box
//! does.
//!
//! # What this rule checks, and what it refuses to
//!
//! **The box is checked here, exactly.** A venue's crop and its caption strip
//! are declared numbers ([`crate::geometry::declare`]) and the renderer's
//! padding is a stated one, so how far the room reaches inside the safe area is
//! arithmetic. Every figure a diagnostic here quotes was computed, not guessed.
//!
//! **The content is not checked here, at all.** Whether a slide's content
//! exceeds its box is a question about line breaking, and line breaking needs
//! font metrics this crate does not have and must not invent. Counting
//! characters against a type scale would produce a number that looks measured
//! and is not: it cannot see `text-wrap: balance`, a heading's `22ch` limit,
//! the intrinsic size of an image, a code block that scrolls itself, or
//! anything a framework island renders. Every one of those is a slide that
//! fits and would be reported as broken.
//!
//! That trade is the whole reason for the silence. A linter's failure mode is
//! not missing a problem — it is being wrong often enough that an author adds
//! the group to `allow`, at which point every *other* rule under it stops
//! running too. So content overflow is measured where it can be measured
//! exactly, in the browser the build already launches for the PDF and the
//! social cards, and it reports `overflow/clipped` from there. Where there is
//! no browser there is no finding, rather than a guess wearing a number.
//!
//! # A region is its own box
//!
//! A layout gives each region its own grid track, so a region can lose content
//! while the slide as a whole fits: a column a third of the width holds a third
//! of the line, and the body's own scroll height never notices. So the browser
//! measures each region as well as the slide, every box gets its own finding, and
//! the finding names the region — "the slide is too tall" would send an author
//! looking at the wrong half of it.

use slidx_core::{Diagnostic, Diagnostics, Severity, Slide, SourceSpan};

use crate::geometry::{self, declare, Bleed, Insets, Side, CUTTING_SHARE};
use crate::surface::{Measurement, RenderTarget};
use crate::{LintInput, LintOptions};

/// Overflow under this share of the box is browser rounding, not lost content.
///
/// Engines disagree about the last fraction of a line box, and a subpixel
/// difference in where a line ends is not something an audience can see. The
/// floor is what keeps a rule that reports real clipping from also reporting
/// the same slide as broken on one engine and fine on another.
const ROUNDING: f64 = 0.01;

pub fn check(input: &LintInput<'_>, options: &LintOptions, sink: &mut Diagnostics) {
    let declared = declare::read(&input.deck.meta.raw, input.target);

    for written in &declared.unreadable {
        sink.push(
            Diagnostic::new(
                "overflow/declaration",
                Severity::Warning,
                format!("safe area declaration not understood — `{written}`"),
            )
            .with_help("give the value a unit: `15%` of the slide, or `120px` on its canvas"),
        );
    }

    // The caller was standing in the room; the deck was written before anyone
    // had seen it. Nothing at all is the common case, and the common case has
    // to stay silent.
    let Some(room) = options.safe_area.or(declared.insets) else { return };

    // Without a stated padding there is no safe area to compare against, and a
    // guessed one would report bleed on a theme that has none. The editor lints
    // this way on every keystroke, long before anything has been rendered.
    let Some(padding) = input.padding else { return };

    for bleed in geometry::bleed(padding, room, input.target) {
        sink.push(report(bleed, room, input.target));
    }
}

fn report(bleed: Bleed, room: Insets, target: RenderTarget) -> Diagnostic {
    let taken = percent(room.share(bleed.side));

    // A strip along the bottom is its own code because it is its own decision:
    // an author can move content off the bottom of a slide, and cannot do
    // anything at all about a projector that crops all four edges.
    let code =
        if bleed.side == Side::Bottom { "overflow/caption-strip" } else { "overflow/safe-area" };

    let severity = if bleed.is_cutting() { Severity::Error } else { Severity::Warning };

    let message = match (bleed.side, bleed.is_cutting()) {
        (Side::Bottom, true) => format!(
            "the caption strip takes the bottom {taken} of the slide, and {} of the content with it",
            percent(bleed.share_of_content)
        ),
        (Side::Bottom, false) => format!(
            "the caption strip takes the bottom {taken} of the slide, {:.0}px past the safe area",
            bleed.past_px
        ),
        (side, true) => format!(
            "the room crops {taken} off the {} edge, and {} of the content with it",
            side.as_token(),
            percent(bleed.share_of_content)
        ),
        (side, false) => format!(
            "the room crops {taken} off the {} edge, {:.0}px past the safe area",
            side.as_token(),
            bleed.past_px
        ),
    };

    Diagnostic::new(code, severity, message).with_help(format!(
        "raise the theme's padding to {}, or keep the {} of every slide clear",
        percent(needed_padding(bleed.side, room, target)),
        bleed.side.as_token()
    ))
}

/// Reports content a browser found outside the design box.
///
/// Separate from [`check`] because it answers a different question with a
/// different kind of evidence: [`check`] compares two declared numbers, this
/// one reads what a real layout did. Registering it separately also means the
/// exact geometry keeps running for an author who has switched this off.
pub fn check_measured(input: &LintInput<'_>, _options: &LintOptions, sink: &mut Diagnostics) {
    for slide in &input.deck.slides {
        for axis in [Axis::Height, Axis::Width] {
            // The worst stop rather than every stop: ten reveals over the same
            // box is one problem with one slide, and ten copies of it is a
            // report nobody reads to the end of.
            //
            // The slide and each of its regions are separate boxes, so each gets
            // its own worst stop — a region that loses a column of text while
            // the slide as a whole fits is invisible in the slide's own numbers.
            for measured in worst_per_box(input.measured, slide.index, axis) {
                if axis.of(measured) >= ROUNDING {
                    sink.push(clipped(slide, measured, axis));
                }
            }
        }
    }
}

/// Which way the content escaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Height,
    Width,
}

impl Axis {
    fn of(self, measured: &Measurement) -> f64 {
        match self {
            Self::Height => measured.over_height,
            Self::Width => measured.over_width,
        }
    }
}

/// The worst stop for the slide's own box and for each of its regions.
///
/// Ordered slide-first and then by region name, so a report is stable across
/// runs: the browser hands back regions in document order, which is the layout's
/// order and not something a reader can predict.
fn worst_per_box(measured: &[Measurement], slide_index: u32, axis: Axis) -> Vec<&Measurement> {
    let mut boxes: Vec<Option<&str>> = measured
        .iter()
        .filter(|found| found.slide_index == slide_index)
        .map(|found| found.region.as_deref())
        .collect();

    boxes.sort_unstable();
    boxes.dedup();

    boxes
        .into_iter()
        .filter_map(|region| {
            measured
                .iter()
                .filter(|found| {
                    found.slide_index == slide_index && found.region.as_deref() == region
                })
                .max_by(|a, b| axis.of(a).total_cmp(&axis.of(b)))
        })
        .collect()
}

fn clipped(slide: &Slide, found: &Measurement, axis: Axis) -> Diagnostic {
    let over = axis.of(found);
    let severity = if over >= CUTTING_SHARE { Severity::Error } else { Severity::Warning };

    // The slide is named by the span, and a report prefixes every finding with
    // it. Repeating it here would read as two slides.
    let at = if found.stop == 0 { String::new() } else { format!("at stop {}, ", found.stop + 1) };

    // A region is its own box, so its own overflow. Saying "the slide" when one
    // column of two is the problem sends an author looking at the wrong half.
    let (what, box_) = match &found.region {
        Some(region) => (format!("the `{region}` region's content"), "region"),
        None => ("the content".to_string(), "design box"),
    };

    let (bigger, edge, help) = match (axis, found.region.is_some()) {
        (Axis::Height, false) => (
            "taller",
            "the bottom of it is cut off",
            "split the slide, or move something to the next one — the shell will not shrink type to fit",
        ),
        (Axis::Height, true) => (
            "taller",
            "the bottom of it is cut off",
            "move a block to another region, or split the slide — a region does not shrink type to fit either",
        ),
        (Axis::Width, false) => (
            "wider",
            "the right of it is cut off",
            "shorten the longest line; a code block that scrolls on a laptop is simply missing on a wall",
        ),
        (Axis::Width, true) => (
            "wider",
            "the right of it is cut off",
            "a region is narrower than the slide, so a line that fitted before the block was moved need not fit now",
        ),
    };

    Diagnostic::new(
        "overflow/clipped",
        severity,
        format!("{at}{what} is {} {bigger} than the {box_}, and {edge}", percent(over)),
    )
    .at(SourceSpan::line(slide.source_line).on_slide(slide.index))
    .with_help(help)
}

/// Padding that would keep content out of the band, as a share of the height.
///
/// One number rather than four, because the shell resolves padding in units of
/// the slide's height and applies it to every side at once.
fn needed_padding(side: Side, room: Insets, target: RenderTarget) -> f64 {
    let share = room.share(side);

    match side {
        Side::Top | Side::Bottom => share,
        _ if target.height_px > 0.0 => share * target.width_px / target.height_px,
        _ => share,
    }
}

/// A share as the author would have written it.
fn percent(share: f64) -> String {
    let value = share * 100.0;

    if (value - value.round()).abs() < 0.05 {
        format!("{value:.0}%")
    } else {
        format!("{value:.1}%")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{lint_deck, lint_deck_rendered};

    /// The share of the slide's height the built-in themes pad by.
    const PADDING: f64 = 96.0 / 1080.0;

    #[test]
    fn a_deck_that_declares_no_room_produces_nothing() {
        assert!(lint_deck_rendered("# One\n", PADDING).is_empty());
    }

    #[test]
    fn a_caption_strip_the_theme_already_covers_produces_nothing() {
        // The case worth being quiet about: the venue eats 5% and the theme's
        // padding keeps content out of the bottom 8.9% anyway.
        let source = "---\nsafeArea:\n  bottom: 5%\n---\n\n# One\n";
        assert!(lint_deck_rendered(source, PADDING).is_empty());
    }

    #[test]
    fn a_venue_that_eats_the_bottom_fifteen_percent_is_an_error() {
        let source = "---\nsafeArea:\n  bottom: 15%\n---\n\n# One\n";
        let diagnostics = lint_deck_rendered(source, PADDING);

        let first = diagnostics.iter().find(|d| d.code == "overflow/caption-strip").unwrap();
        assert_eq!(first.severity, Severity::Error);
        assert!(first.message.contains("bottom 15%"), "got: {}", first.message);
    }

    #[test]
    fn a_strip_that_only_grazes_the_frame_is_a_warning_with_the_distance() {
        // Twelve pixels past the padding is inside the footer's own leading. A
        // rule that called this an error alongside the 15% case is a rule an
        // author turns off, and takes the exact checks down with it.
        let source = "---\nsafeArea:\n  bottom: 10%\n---\n\n# One\n";
        let diagnostics = lint_deck_rendered(source, PADDING);
        let first = &diagnostics.as_slice()[0];

        assert_eq!(first.severity, Severity::Warning);
        assert!(first.message.contains("12px past the safe area"), "got: {}", first.message);
    }

    #[test]
    fn the_error_names_how_much_of_the_content_box_is_gone() {
        let source = "---\nsafeArea:\n  bottom: 15%\n---\n\n# One\n";
        let diagnostics = lint_deck_rendered(source, PADDING);
        let first = &diagnostics.as_slice()[0];

        assert!(first.message.contains("7.4%"), "got: {}", first.message);
    }

    #[test]
    fn the_help_names_the_padding_that_would_fix_it() {
        let source = "---\nsafeArea:\n  bottom: 15%\n---\n\n# One\n";
        let help = lint_deck_rendered(source, PADDING).as_slice()[0].help.clone().unwrap();

        assert!(help.contains("15%"), "got: {help}");
    }

    #[test]
    fn a_crop_off_a_side_edge_asks_for_padding_in_the_same_units_the_shell_uses() {
        // The shell pads in units of the slide's *height*, so covering 10% of
        // a 1920-wide canvas takes 17.8% of its height, not 10%.
        let source = "---\nsafeArea:\n  left: 10%\n---\n\n# One\n";
        let diagnostics = lint_deck_rendered(source, PADDING);
        let first = &diagnostics.as_slice()[0];

        assert_eq!(first.code, "overflow/safe-area");
        assert!(first.help.as_ref().unwrap().contains("17.8%"), "got: {:?}", first.help);
    }

    #[test]
    fn an_overscanning_projector_is_declared_once_and_checked_on_every_edge() {
        let source = "---\nsafeArea: 12%\n---\n\n# One\n";
        let diagnostics = lint_deck_rendered(source, PADDING);

        assert_eq!(diagnostics.len(), 4);
        for side in ["top", "right", "bottom", "left"] {
            assert!(
                diagnostics.iter().any(|d| d.message.contains(side)),
                "no finding for the {side} edge"
            );
        }
    }

    #[test]
    fn nothing_is_reported_without_a_padding_to_compare_against() {
        // The editor lints on every keystroke, before anything is rendered.
        // A padding invented here would report bleed on a theme that has none.
        let source = "---\nsafeArea:\n  bottom: 15%\n---\n\n# One\n";
        assert!(lint_deck(source).is_empty());
    }

    #[test]
    fn a_declaration_that_cannot_be_read_is_reported_rather_than_skipped() {
        // Silence here would be silence in exactly the place the author
        // believed they had asked for a check.
        let source = "---\nsafeArea:\n  bottom: 15\n---\n\n# One\n";
        let diagnostics = lint_deck_rendered(source, PADDING);
        let first = &diagnostics.as_slice()[0];

        assert_eq!(first.code, "overflow/declaration");
        assert!(first.help.as_ref().unwrap().contains("15%"));
    }

    #[test]
    fn a_declaration_is_checked_even_where_there_is_no_padding_to_use_it_with() {
        let source = "---\nsafeArea:\n  bottom: 15\n---\n\n# One\n";
        let diagnostics = lint_deck(source);
        assert_eq!(diagnostics.as_slice()[0].code, "overflow/declaration");
    }

    #[test]
    fn a_room_supplied_by_the_caller_overrides_the_decks_own() {
        // Whoever passes this is standing in the room; the deck was written
        // before anyone had seen it.
        let source = "---\nsafeArea:\n  bottom: 5%\n---\n\n# One\n";
        let diagnostics = crate::test_support::lint_deck_in_room(source, PADDING, |options| {
            options.safe_area = Some(Insets::NONE.with_side(Side::Bottom, 0.2));
        });

        assert!(diagnostics.iter().any(|d| d.message.contains("bottom 20%")));
    }

    #[test]
    fn the_group_can_be_suppressed_as_a_whole() {
        let source = "---\nsafeArea: 12%\n---\n\n# One\n";
        let diagnostics = crate::test_support::lint_deck_in_room(source, PADDING, |options| {
            options.allow = vec!["overflow".to_string()];
        });

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn shares_read_the_way_an_author_wrote_them() {
        assert_eq!(percent(0.15), "15%");
        assert_eq!(percent(0.074), "7.4%");
        assert_eq!(percent(0.0), "0%");
    }

    mod measured {
        use super::*;
        use crate::surface::Measurement;
        use crate::test_support::lint_deck_measured;

        const DECK: &str = "# One\n\n---\n\n# Two\n";

        #[test]
        fn a_deck_nobody_measured_produces_nothing() {
            // Most builds. There is no browser, so there is no finding — not
            // an approximation of what one would have said.
            assert!(lint_deck_measured(DECK, &[]).is_empty());
        }

        #[test]
        fn a_stop_that_fitted_produces_nothing() {
            let fitted = [Measurement::new(0, 0), Measurement::new(1, 0)];
            assert!(lint_deck_measured(DECK, &fitted).is_empty());
        }

        #[test]
        fn a_slide_taller_than_its_box_is_an_error_pointed_at_that_slide() {
            let measured = [Measurement::new(1, 0).over(0.18, 0.0)];
            let diagnostics = lint_deck_measured(DECK, &measured);
            let first = &diagnostics.as_slice()[0];

            assert_eq!(first.code, "overflow/clipped");
            assert_eq!(first.severity, Severity::Error);
            assert_eq!(first.span.slide_index, Some(1));
        }

        #[test]
        fn the_message_does_not_repeat_the_slide_a_report_already_names() {
            let measured = [Measurement::new(1, 0).over(0.18, 0.0)];
            let diagnostics = lint_deck_measured(DECK, &measured);

            assert!(!diagnostics.as_slice()[0].message.contains("Two"));
        }

        #[test]
        fn the_message_says_how_much_is_cut_off() {
            let measured = [Measurement::new(0, 0).over(0.18, 0.0)];
            let diagnostics = lint_deck_measured(DECK, &measured);

            assert!(diagnostics.as_slice()[0].message.contains("18%"));
        }

        #[test]
        fn a_slide_barely_over_its_box_is_a_warning_rather_than_an_error() {
            // Browsers disagree about the last fraction of a line box. A rule
            // that failed a build over two pixels is a rule nobody keeps on.
            let measured = [Measurement::new(0, 0).over(0.02, 0.0)];
            let diagnostics = lint_deck_measured(DECK, &measured);

            assert_eq!(diagnostics.as_slice()[0].severity, Severity::Warning);
        }

        #[test]
        fn an_overflow_smaller_than_a_rounding_error_is_not_reported_at_all() {
            let measured = [Measurement::new(0, 0).over(0.002, 0.0)];
            assert!(lint_deck_measured(DECK, &measured).is_empty());
        }

        #[test]
        fn content_cut_off_the_side_is_reported_separately_from_the_bottom() {
            let measured = [Measurement::new(0, 0).over(0.18, 0.18)];
            let diagnostics = lint_deck_measured(DECK, &measured);

            assert_eq!(diagnostics.len(), 2);
            assert!(diagnostics.iter().any(|d| d.message.contains("taller")));
            assert!(diagnostics.iter().any(|d| d.message.contains("wider")));
        }

        #[test]
        fn the_help_says_to_split_the_slide_rather_than_shrink_the_type() {
            // The shell has no way to shrink type to fit and that is deliberate,
            // so help that suggested it would be help nobody can follow.
            let measured = [Measurement::new(0, 0).over(0.18, 0.0)];
            let diagnostics = lint_deck_measured(DECK, &measured);
            let help = diagnostics.as_slice()[0].help.clone().unwrap();

            assert!(help.contains("split"), "got: {help}");
        }

        #[test]
        fn a_slide_that_only_overflows_on_a_later_reveal_names_the_stop() {
            let source = "# One\n\n- a <!-- step -->\n- b <!-- step -->\n";
            let measured = [Measurement::new(0, 2).over(0.18, 0.0)];
            let diagnostics = lint_deck_measured(source, &measured);

            assert!(diagnostics.as_slice()[0].message.contains("stop 3"));
        }

        #[test]
        fn a_slide_that_overflows_from_the_start_does_not_mention_a_stop() {
            let measured = [Measurement::new(0, 0).over(0.18, 0.0)];
            let diagnostics = lint_deck_measured(DECK, &measured);

            assert!(!diagnostics.as_slice()[0].message.contains("stop"));
        }

        #[test]
        fn a_measurement_of_a_slide_that_no_longer_exists_is_ignored() {
            // The deck can be edited between the build and the measurement.
            // Reporting against a slide index nobody can open is worse than
            // reporting nothing.
            let measured = [Measurement::new(9, 0).over(0.5, 0.0)];
            assert!(lint_deck_measured(DECK, &measured).is_empty());
        }

        #[test]
        fn a_region_that_overflows_while_the_slide_fits_is_still_reported() {
            // The failure a layout introduces: a two-column slide whose right
            // column has lost its last three lines fits perfectly as a slide.
            let measured =
                [Measurement::new(0, 0), Measurement::new(0, 0).over(0.2, 0.0).in_region("right")];
            let diagnostics = lint_deck_measured(DECK, &measured);

            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics.as_slice()[0].message.contains("`right`"));
        }

        #[test]
        fn a_regions_finding_says_to_move_a_block_rather_than_to_split_the_slide() {
            // Splitting a slide whose other column is half empty is the wrong
            // fix, and help nobody follows is help that teaches them to stop
            // reading it.
            let measured = [Measurement::new(0, 0).over(0.2, 0.0).in_region("side")];
            let help = lint_deck_measured(DECK, &measured).as_slice()[0].help.clone().unwrap();

            assert!(help.contains("another region"), "got: {help}");
        }

        #[test]
        fn the_slide_and_a_region_over_the_same_box_are_two_findings() {
            // Two boxes, two fixes: the slide is too tall *and* one region of it
            // is losing content on its own.
            let measured = [
                Measurement::new(0, 0).over(0.2, 0.0),
                Measurement::new(0, 0).over(0.3, 0.0).in_region("side"),
            ];
            let diagnostics = lint_deck_measured(DECK, &measured);

            assert_eq!(diagnostics.len(), 2);
            assert!(diagnostics.iter().any(|d| !d.message.contains("region")));
            assert!(diagnostics.iter().any(|d| d.message.contains("`side`")));
        }

        #[test]
        fn one_region_is_reported_once_however_many_stops_overflow() {
            let measured = [
                Measurement::new(0, 1).over(0.18, 0.0).in_region("side"),
                Measurement::new(0, 2).over(0.24, 0.0).in_region("side"),
            ];
            let diagnostics = lint_deck_measured(DECK, &measured);

            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics.as_slice()[0].message.contains("24%"));
        }

        #[test]
        fn the_same_slide_is_reported_once_however_many_stops_overflow() {
            // Ten stops over the same box is one problem with the slide, and
            // ten copies of it is a report nobody reads.
            let measured = [
                Measurement::new(0, 1).over(0.18, 0.0),
                Measurement::new(0, 2).over(0.24, 0.0),
                Measurement::new(0, 3).over(0.11, 0.0),
            ];
            let diagnostics = lint_deck_measured(DECK, &measured);

            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics.as_slice()[0].message.contains("24%"), "the worst stop is the one");
        }
    }
}
