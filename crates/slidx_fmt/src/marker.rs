//! Step marker spelling: `<!-- step -->` and `<!-- step: fly-in -->`.
//!
//! `extract_step_markers` accepts `<!--step-->`, `<!-- step:fly-in -->` and
//! every other arrangement of the whitespace around the keyword. They all
//! compile to the same action, so the file may as well say the one thing — and
//! a marker is easier to spot at a glance in a paragraph when every one of them
//! looks identical.
//!
//! The preset name is copied through exactly as written, never re-emitted from
//! what the parser made of it. `<!-- step: wiggle -->` is a typo that costs the
//! author an animation, and a formatter that rewrote it to `<!-- step -->`
//! would delete the evidence of the mistake while leaving the mistake.

use slidx_core::markers::marker_body;
use slidx_core::ByteSpan;
use slidx_edit::EditBuilder;

use crate::claim;

const OPEN: &str = "<!--";
const CLOSE: &str = "-->";

/// Normalises every step marker in a slide body.
pub fn format(
    source: &str,
    body: ByteSpan,
    claimed: &mut Vec<ByteSpan>,
    builder: &mut EditBuilder,
) {
    for (span, written) in markers(source, body) {
        if !claim(claimed, span) {
            continue;
        }

        builder.replace(span, canonical(written));
    }
}

/// Every step marker in `body`, with the preset text as the author wrote it.
fn markers(source: &str, body: ByteSpan) -> Vec<(ByteSpan, &str)> {
    let text = &source[body.start..body.end];
    let mut found = Vec::new();
    let mut cursor = 0usize;

    while let Some(open_at) = text[cursor..].find(OPEN).map(|at| cursor + at) {
        let inner_at = open_at + OPEN.len();
        let Some(close_at) = text[inner_at..].find(CLOSE).map(|at| inner_at + at) else {
            // An unterminated comment has no extent, so it is not yet a marker.
            break;
        };

        if let Some(preset) = marker_body(&text[inner_at..close_at]) {
            found.push((
                ByteSpan::new(body.start + open_at, body.start + close_at + CLOSE.len()),
                preset,
            ));
        }

        cursor = close_at + CLOSE.len();
    }

    found
}

fn canonical(preset: &str) -> String {
    if preset.is_empty() {
        return format!("{OPEN} step {CLOSE}");
    }

    format!("{OPEN} step: {preset} {CLOSE}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format_all(source: &str) -> String {
        let mut builder = EditBuilder::new(source);
        format(source, ByteSpan::new(0, source.len()), &mut Vec::new(), &mut builder);
        builder.build().apply(source)
    }

    #[test]
    fn a_marker_with_no_spaces_gains_them() {
        assert_eq!(format_all("- one <!--step-->\n"), "- one <!-- step -->\n");
    }

    #[test]
    fn a_marker_naming_a_preset_is_spelled_with_one_space_after_the_colon() {
        assert_eq!(format_all("- one <!--step:fly-in-->\n"), "- one <!-- step: fly-in -->\n");
        assert_eq!(format_all("- one <!-- step  :  zoom  -->\n"), "- one <!-- step: zoom -->\n");
    }

    #[test]
    fn a_marker_written_without_a_colon_keeps_meaning_what_it_meant() {
        // `<!-- step fly-in -->` is accepted by the parser, so the formatter
        // has to canonicalise it rather than leave a second spelling behind.
        assert_eq!(format_all("- one <!-- step fly-in -->\n"), "- one <!-- step: fly-in -->\n");
    }

    #[test]
    fn a_preset_slidx_does_not_know_keeps_the_name_it_was_written_with() {
        // The parser ignores an unknown preset, so re-emitting the parsed value
        // would silently delete the typo and leave the author wondering why
        // their animation never appeared.
        assert_eq!(format_all("- one <!--step: wiggle-->\n"), "- one <!-- step: wiggle -->\n");
    }

    #[test]
    fn a_comment_that_is_not_a_marker_is_left_alone() {
        for comment in ["<!-- stepper -->", "<!-- notes: x -->", "<!-- TODO -->", "<!---->"] {
            let source = format!("# One\n\n{comment}\n");
            assert_eq!(format_all(&source), source, "{comment}");
        }
    }

    #[test]
    fn an_unterminated_comment_is_not_a_marker_yet() {
        // Every marker passes through this state while somebody types it.
        assert_eq!(format_all("- one <!--step\n"), "- one <!--step\n");
    }

    #[test]
    fn several_markers_on_one_slide_are_all_normalised() {
        assert_eq!(
            format_all("- a <!--step-->\n- b <!--step: zoom-->\n"),
            "- a <!-- step -->\n- b <!-- step: zoom -->\n"
        );
    }

    #[test]
    fn a_marker_inside_a_fence_is_left_alone() {
        let source = "```md\n- a <!--step-->\n```\n";
        let mut builder = EditBuilder::new(source);
        let whole = ByteSpan::new(0, source.len());
        format(source, whole, &mut vec![whole], &mut builder);

        assert!(builder.build().is_empty());
    }
}
