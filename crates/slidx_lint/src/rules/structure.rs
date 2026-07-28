//! Content rules.
//!
//! These read the Markdown body, so they catch problems before a renderer or a
//! browser is involved — which means they run in the editor as the author
//! types, not only at build time.

use slidx_core::scanner::{heading_text, list_item_indent, FenceTracker};
use slidx_core::{Diagnostic, Diagnostics, Severity, Slide, SourceSpan};

use crate::{LintInput, LintOptions};

/// Bullets past which a slide stops being scannable.
///
/// Presentation guidance converges on four or five; six is the point at which
/// it is worth saying something.
const MAX_BULLETS: usize = 6;

pub fn check(input: &LintInput<'_>, _options: &LintOptions, sink: &mut Diagnostics) {
    for slide in &input.deck.slides {
        check_images(slide, sink);
        check_bullets(slide, sink);
        check_heading_order(slide, sink);
        check_link_text(slide, sink);
    }
}

fn span(slide: &Slide, offset: usize) -> SourceSpan {
    SourceSpan::line(slide.source_line + offset as u32).on_slide(slide.index)
}

/// Flags images with no alternative text.
fn check_images(slide: &Slide, sink: &mut Diagnostics) {
    for (offset, line) in prose_lines(slide) {
        for (start, _) in line.match_indices("![") {
            let rest = &line[start + 2..];
            let Some(end) = rest.find(']') else { continue };

            if rest[..end].trim().is_empty() {
                sink.push(
                    Diagnostic::new(
                        "structure/missing-alt",
                        Severity::Warning,
                        "image has no alternative text",
                    )
                    .at(span(slide, offset))
                    .with_help("describe what the image shows; use `![](…)` only for decoration"),
                );
            }
        }
    }
}

/// Flags slides carrying more bullets than an audience can hold.
fn check_bullets(slide: &Slide, sink: &mut Diagnostics) {
    let bullets = prose_lines(slide)
        .filter(|(_, line)| list_item_indent(line).is_some_and(|indent| indent < 2))
        .count();

    if bullets > MAX_BULLETS {
        sink.push(
            Diagnostic::new(
                "structure/too-many-bullets",
                Severity::Info,
                format!("{bullets} top-level bullets on \"{}\"", slide.display_title()),
            )
            .at(span(slide, 0))
            .with_help("split the slide, or reveal the bullets in stages with `autoSteps: list`"),
        );
    }
}

/// Flags heading levels that skip a rank.
///
/// Screen readers and the generated outline both use heading rank; a jump from
/// `#` to `###` leaves a hole in both.
fn check_heading_order(slide: &Slide, sink: &mut Diagnostics) {
    let mut previous: Option<usize> = None;

    for (offset, line) in prose_lines(slide) {
        if heading_text(line).is_none() {
            continue;
        }

        let level = line.trim_start().chars().take_while(|&c| c == '#').count();
        if let Some(previous) = previous {
            if level > previous + 1 {
                sink.push(
                    Diagnostic::new(
                        "structure/heading-order",
                        Severity::Warning,
                        format!("heading jumps from level {previous} to level {level}"),
                    )
                    .at(span(slide, offset))
                    .with_help("use the next level down so the outline stays complete"),
                );
            }
        }
        previous = Some(level);
    }
}

/// Flags links whose visible text is the URL.
///
/// A raw URL is unreadable on a projector, impossible to say aloud, and
/// useless in a PDF where nobody can click it.
fn check_link_text(slide: &Slide, sink: &mut Diagnostics) {
    for (offset, line) in prose_lines(slide) {
        for (start, _) in line.match_indices('[') {
            if start > 0 && line[..start].ends_with('!') {
                continue;
            }

            let rest = &line[start + 1..];
            let Some(end) = rest.find(']') else { continue };
            if !rest[end + 1..].starts_with('(') {
                continue;
            }

            let text = rest[..end].trim();
            if text.starts_with("http://") || text.starts_with("https://") {
                sink.push(
                    Diagnostic::new("structure/bare-url", Severity::Info, "link text is a raw URL")
                        .at(span(slide, offset))
                        .with_help("name the destination; slidx will add a QR for the URL itself"),
                );
            }
        }
    }
}

/// Slide lines outside fenced code, paired with their offset from the body.
fn prose_lines(slide: &Slide) -> impl Iterator<Item = (usize, &str)> {
    let mut fences = FenceTracker::new();
    slide
        .content
        .lines()
        .enumerate()
        .filter(move |(_, line)| fences.feed(line))
        .collect::<Vec<_>>()
        .into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::lint_deck;

    #[test]
    fn a_clean_slide_produces_nothing() {
        assert!(lint_deck("# One\n\n- a\n- b\n\n![a chart of latency](./chart.png)\n").is_empty());
    }

    #[test]
    fn an_image_without_alt_text_is_flagged() {
        let diagnostics = lint_deck("# One\n\n![](./chart.png)\n");
        assert_eq!(diagnostics.as_slice()[0].code, "structure/missing-alt");
    }

    #[test]
    fn whitespace_does_not_count_as_alt_text() {
        assert_eq!(lint_deck("![   ](./a.png)\n").len(), 1);
    }

    #[test]
    fn image_syntax_inside_a_code_fence_is_not_checked() {
        assert!(lint_deck("# One\n\n```md\n![](./example.png)\n```\n").is_empty());
    }

    #[test]
    fn a_slide_with_too_many_bullets_is_noted() {
        let bullets: String = (1..=8).map(|n| format!("- point {n}\n")).collect();
        let diagnostics = lint_deck(&format!("# Agenda\n\n{bullets}"));

        let first = diagnostics.iter().find(|d| d.code == "structure/too-many-bullets").unwrap();
        assert!(first.message.contains('8'));
        assert!(first.help.as_ref().unwrap().contains("autoSteps"));
    }

    #[test]
    fn nested_bullets_do_not_count_towards_the_limit() {
        let source = "# One\n\n- a\n  - a1\n  - a2\n  - a3\n  - a4\n  - a5\n  - a6\n- b\n";
        assert!(lint_deck(source).is_empty());
    }

    #[test]
    fn a_slide_at_the_bullet_limit_passes() {
        let bullets: String = (1..=6).map(|n| format!("- point {n}\n")).collect();
        assert!(lint_deck(&format!("# Agenda\n\n{bullets}")).is_empty());
    }

    #[test]
    fn a_skipped_heading_level_is_flagged() {
        let diagnostics = lint_deck("# One\n\n### Three\n");

        let first = diagnostics.iter().find(|d| d.code == "structure/heading-order").unwrap();
        assert!(first.message.contains("level 1 to level 3"));
    }

    #[test]
    fn descending_one_level_at_a_time_is_fine() {
        assert!(lint_deck("# One\n\n## Two\n\n### Three\n\n## Back\n").is_empty());
    }

    #[test]
    fn heading_order_is_judged_per_slide() {
        // Slides are separate pages, so slide two starting at `#` after slide
        // one ended at `###` is correct, not a jump.
        assert!(lint_deck("# One\n\n## Two\n\n---\n\n# Fresh\n").is_empty());
    }

    #[test]
    fn a_link_whose_text_is_a_url_is_noted() {
        let diagnostics = lint_deck(
            "See [https://example.com/very/long/path](https://example.com/very/long/path)\n",
        );

        let first = diagnostics.iter().find(|d| d.code == "structure/bare-url").unwrap();
        assert_eq!(first.severity, Severity::Info);
        assert!(first.help.as_ref().unwrap().contains("QR"));
    }

    #[test]
    fn a_named_link_is_fine() {
        assert!(lint_deck("See [the docs](https://example.com)\n").is_empty());
    }

    #[test]
    fn an_image_is_not_mistaken_for_a_bare_url_link() {
        assert!(lint_deck("![https://example.com/chart](./chart.png)\n").is_empty());
    }

    #[test]
    fn diagnostics_point_at_the_line_within_the_slide() {
        let diagnostics = lint_deck("# One\n\n---\n\n# Two\n\n![](./a.png)\n");
        let first = &diagnostics.as_slice()[0];

        assert_eq!(first.span.slide_index, Some(1));
        assert!(first.span.line > 5, "expected a line inside slide two, got {}", first.span.line);
    }
}
