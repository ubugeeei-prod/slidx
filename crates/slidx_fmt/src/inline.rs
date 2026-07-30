//! Attribute order inside a mark's `{…}` group.
//!
//! `Mark::attributes_source` is already canonical — key, then classes as
//! written, then properties sorted — because the visual editor writes marks and
//! must never produce a diff nobody asked for. This rule is therefore not a new
//! opinion: it applies the one serialisation already agreed on to the marks a
//! person typed by hand, so a file the editor has touched and a file it has not
//! read the same.
//!
//! Only the braces and what is between them are replaced. The marked text is
//! prose, and `Mark::to_source` escapes brackets on the way out — so re-emitting
//! a whole mark would turn `[a [nested] b]{#k}` into `[a \[nested\] b]{#k}`,
//! which is the class of diff this crate exists to avoid.

use slidx_core::mark::find_marks;
use slidx_core::ByteSpan;
use slidx_edit::EditBuilder;

use crate::claim;

/// Normalises every mark's attribute group in a slide body.
pub fn format(
    source: &str,
    body: ByteSpan,
    claimed: &mut Vec<ByteSpan>,
    builder: &mut EditBuilder,
) {
    let text = &source[body.start..body.end];

    for found in find_marks(text) {
        // The whole mark, not just its braces: an unclosed `]{` reaches down the
        // slide and swallows the comment below it, and a mark overlapping
        // something another rule owns is not the mark the parser will see.
        let mark = ByteSpan::new(body.start + found.start, body.start + found.end);
        if !claim(claimed, mark) {
            continue;
        }

        let group = ByteSpan::new(body.start + found.attributes_start, body.start + found.end);
        builder.replace(group, format!("{{{}}}", found.mark.attributes_source()));
    }
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
    fn a_key_comes_first_and_properties_come_sorted() {
        assert_eq!(
            format_all("The result was [3.2x]{font=mono .accent color=danger #result}.\n"),
            "The result was [3.2x]{#result .accent color=danger font=mono}.\n"
        );
    }

    #[test]
    fn classes_keep_the_order_they_were_written_in() {
        // Class order is the cascade, so sorting it could change which one
        // wins. The editor does not reorder them and neither does this.
        assert_eq!(format_all("[x]{.b .a}\n"), "[x]{.b .a}\n");
    }

    #[test]
    fn a_bare_word_is_spelled_as_the_class_it_means() {
        assert_eq!(format_all("[x]{accent}\n"), "[x]{.accent}\n");
    }

    #[test]
    fn quotes_are_dropped_where_a_value_does_not_need_them() {
        assert_eq!(format_all("[x]{color=\"danger\"}\n"), "[x]{color=danger}\n");
        assert_eq!(format_all("[x]{caption=\"two words\"}\n"), "[x]{caption=\"two words\"}\n");
    }

    #[test]
    fn the_marked_text_is_never_re_emitted() {
        // `Mark::to_source` escapes brackets, so writing the whole mark back
        // would add backslashes to a phrase the author typed.
        assert_eq!(format_all("[a [nested] b]{.a #k}\n"), "[a [nested] b]{#k .a}\n");
    }

    #[test]
    fn a_japanese_mark_is_spliced_at_a_character_boundary() {
        // Byte offsets, not character offsets: this deck's own slides are in
        // Japanese, and getting it wrong panics rather than misbehaves.
        assert_eq!(
            format_all("結果は [3.2倍速く]{color=danger #結果} なった。\n"),
            "結果は [3.2倍速く]{#結果 color=danger} なった。\n"
        );
    }

    #[test]
    fn a_link_is_not_a_mark_and_is_left_alone() {
        let source = "see [the docs](https://example.com)\n";
        assert_eq!(format_all(source), source);
    }

    #[test]
    fn a_half_typed_mark_is_left_as_text() {
        for source in ["[unclosed\n", "[text]{unclosed\n", "[text]\n"] {
            assert_eq!(format_all(source), source, "{source:?}");
        }
    }

    #[test]
    fn two_takes_of_one_value_are_both_normalised() {
        // Adjacent marks sharing a key are one changing element, and the
        // formatter must not merge or reorder them.
        assert_eq!(
            format_all("Latency dropped to [120ms]{ #latency }[38ms]{ #latency }.\n"),
            "Latency dropped to [120ms]{#latency}[38ms]{#latency}.\n"
        );
    }

    #[test]
    fn a_mark_inside_a_fence_is_left_alone() {
        let source = "```md\n[x]{.a #k}\n```\n";
        let mut builder = EditBuilder::new(source);
        let whole = ByteSpan::new(0, source.len());
        format(source, whole, &mut vec![whole], &mut builder);

        assert!(builder.build().is_empty());
    }
}
