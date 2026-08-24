//! Reading a slide body.
//!
//! The only place in the linter that knows what a tag looks like. Several rules
//! need the same three things out of a slide — the prose with fenced code taken
//! out of play, the attributes on an element, and whether a reference points off
//! the machine — and a second answer to any of them would be a hole with a
//! workaround in it.
//!
//! Not an HTML parser, and it does not need to be. It needs the element name,
//! the attributes, and quoted values to stay opaque so that a `>` inside alt
//! text cannot end a tag early and hide the `src` that follows it.

use slidx_core::scanner::FenceTracker;
use slidx_core::Slide;

/// The slide body with fenced code blanked out.
///
/// Blanking rather than deleting keeps every byte where it was, so an offset
/// still resolves to the line it came from. Fences are blanked because a fence
/// is displayed, not fetched or drawn: a talk about CDNs has to be able to show
/// `<img src="https://…">` on a slide without failing its own build.
pub(crate) fn scannable(slide: &Slide) -> String {
    let mut fences = FenceTracker::new();
    let mut scannable = String::with_capacity(slide.content.len());

    for line in slide.content.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);

        if fences.feed(body) {
            scannable.push_str(line);
        } else {
            scannable.push_str(&" ".repeat(body.len()));
            scannable.push_str(&line[body.len()..]);
        }
    }

    scannable
}

/// Lines preceding `offset`, for turning a byte offset into a source line.
pub(crate) fn line_at(content: &str, offset: usize) -> u32 {
    content.as_bytes()[..offset.min(content.len())].iter().filter(|&&b| b == b'\n').count() as u32
}

/// The element name just past a `<`, and the offset after it.
pub(crate) fn tag_name(content: &str, start: usize) -> Option<(&str, usize)> {
    let rest = &content[start..];
    if !rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }

    let len = rest.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(rest.len());
    Some((&rest[..len], start + len))
}

/// One attribute inside an open tag.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Attribute<'a> {
    pub name: &'a str,
    pub value: &'a str,
    /// Byte offset of the value, so a multi-line tag reports the right line.
    pub offset: usize,
    /// Where the scanner resumes.
    pub next: usize,
}

/// The next attribute inside an open tag.
///
/// `None` means the element is finished — at its `>`, at the end of the slide,
/// or at the start of the next tag when the author never closed this one.
pub(crate) fn attribute(content: &str, at: usize) -> Option<Attribute<'_>> {
    // A `/` here belongs to `<br />` rather than to a name.
    let start = skip_while(content, at, |c| c.is_whitespace() || c == '/');
    let rest = &content[start..];
    if rest.is_empty() || rest.starts_with(['>', '<']) {
        return None;
    }

    let len = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '=' | '>' | '<' | '/'))
        .unwrap_or(rest.len());
    if len == 0 {
        return None;
    }

    let name = &rest[..len];
    let after_name = skip_while(content, start + len, char::is_whitespace);
    if !content[after_name..].starts_with('=') {
        // A valueless attribute such as `controls`.
        return Some(Attribute { name, value: "", offset: start, next: start + len });
    }

    let offset = skip_while(content, after_name + 1, char::is_whitespace);
    let (value, next) = attribute_value(content, offset);
    Some(Attribute { name, value, offset, next })
}

/// An attribute value: quoted, or a bare token up to whitespace or `>`.
fn attribute_value(content: &str, at: usize) -> (&str, usize) {
    let rest = &content[at..];

    for quote in ['"', '\''] {
        if let Some(body) = rest.strip_prefix(quote) {
            let end = body.find(quote).unwrap_or(body.len());
            return (&body[..end], (at + end + 2).min(content.len()));
        }
    }

    let end =
        rest.find(|c: char| c.is_whitespace() || matches!(c, '>' | '<')).unwrap_or(rest.len());
    (&rest[..end], at + end)
}

/// Offset of the first character at or after `at` that `skip` rejects.
fn skip_while(content: &str, at: usize, skip: impl Fn(char) -> bool) -> usize {
    content.len() - content[at..].trim_start_matches(skip).len()
}

/// The URL scheme, or `None` for a relative reference.
///
/// `slidx_core::asset`'s, because `slidx_render` asks the same question of the
/// same references and two answers is what #307 was.
pub(crate) use slidx_core::asset::scheme;

/// The URL of a Markdown destination, without its `"title"` or `<>` wrapper.
pub(crate) fn markdown_target(destination: &str) -> &str {
    let url = destination.split_whitespace().next().unwrap_or("");
    url.trim_start_matches('<').trim_end_matches('>')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every attribute in one open tag, in order.
    fn attributes(tag: &str) -> Vec<(&str, &str)> {
        let (_, mut cursor) = tag_name(tag, 1).expect("a tag name");
        let mut found = Vec::new();

        while let Some(attribute) = attribute(tag, cursor) {
            cursor = attribute.next;
            found.push((attribute.name, attribute.value));
        }

        found
    }

    #[test]
    fn a_greater_than_inside_a_quoted_value_does_not_end_the_tag() {
        let found = attributes("<img alt=\"before > after\" src=\"c.png\">");
        assert_eq!(found, vec![("alt", "before > after"), ("src", "c.png")]);
    }

    #[test]
    fn an_unquoted_value_ends_at_whitespace() {
        assert_eq!(attributes("<img src=c.png alt=c>"), vec![("src", "c.png"), ("alt", "c")]);
    }

    #[test]
    fn a_valueless_attribute_reads_as_empty_rather_than_swallowing_the_next_one() {
        assert_eq!(
            attributes("<video controls src=\"a.mp4\">"),
            vec![("controls", ""), ("src", "a.mp4")]
        );
    }

    #[test]
    fn a_self_closing_slash_is_not_an_attribute_name() {
        assert_eq!(attributes("<img src=\"a.png\" />"), vec![("src", "a.png")]);
    }

    #[test]
    fn a_tag_spread_over_several_lines_reads_as_one() {
        // Pasted embed codes arrive wrapped, and a formatter rewraps them.
        assert_eq!(
            attributes("<img\n  width=\"800\"\n  src=\"a.png\"\n>"),
            vec![("width", "800"), ("src", "a.png")]
        );
    }

    #[test]
    fn a_path_containing_a_colon_is_not_mistaken_for_a_scheme() {
        assert_eq!(scheme("./slides/chapter:1.png"), None);
        assert_eq!(scheme("https://example.com"), Some("https"));
        assert_eq!(scheme("data:image/png;base64,AA"), Some("data"));
    }

    #[test]
    fn a_markdown_title_is_not_part_of_the_destination() {
        assert_eq!(markdown_target("./c.png \"see https://example.com\""), "./c.png");
        assert_eq!(markdown_target("<./c.png>"), "./c.png");
    }

    #[test]
    fn a_fenced_line_is_blanked_without_moving_the_bytes_after_it() {
        let slide = slidx_core::parse_deck(
            "```html\n<img src=\"a.png\">\n```\n\n<img src=\"b.png\">\n",
            &slidx_core::DeckParseOptions::default(),
        )
        .slides
        .remove(0);

        let content = scannable(&slide);
        assert_eq!(content.len(), slide.content.len(), "offsets must survive blanking");
        assert!(!content.contains("a.png"));
        assert!(content.contains("b.png"));
    }

    #[test]
    fn a_line_number_counts_the_newlines_before_an_offset() {
        let content = "one\ntwo\nthree";
        assert_eq!(line_at(content, 0), 0);
        assert_eq!(line_at(content, 4), 1);
        assert_eq!(line_at(content, 8), 2);
        assert_eq!(line_at(content, 999), 2, "an offset past the end must not panic");
    }
}
