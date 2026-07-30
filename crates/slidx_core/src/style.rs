//! Slide-local style written in the Markdown itself.
//!
//! The visual editor needs a durable place for choices that are presentation,
//! not prose: layout first, then colour, density, alignment, and whatever an
//! extension adds later. A tagged style block is ordinary, inspectable Markdown
//! content and valid CSS:
//!
//! ```text
//! <style data-slidx>
//! :root {
//!   --slidx-layout: aside;
//! }
//! </style>
//! ```
//!
//! Only `--slidx-*` declarations inside the tagged block belong to this model.
//! An untagged `<style>` remains the author's raw HTML, and a block inside a
//! code fence remains an example. The parser removes the tagged block before
//! Markdown rendering, then the renderer places the properties on that slide's
//! own element. That scoping matters in the print document, where every slide
//! shares one page and one global `:root`.

use std::collections::BTreeMap;

use crate::scanner::FenceTracker;
use crate::ByteSpan;

pub const OPEN: &str = "<style data-slidx>";
pub const CLOSE: &str = "</style>";
const PREFIX: &str = "--slidx-";

/// One complete tagged style block in a slide body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoundStyle {
    /// Opening tag through closing tag, including their trailing newlines.
    pub span: ByteSpan,
    /// CSS between the two tags.
    pub body: ByteSpan,
}

/// A body with its managed styles separated from its Markdown content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtractedStyle {
    pub content: String,
    pub properties: BTreeMap<String, String>,
}

/// Finds complete tagged style blocks outside code fences.
pub fn find_styles(source: &str) -> Vec<FoundStyle> {
    let mut found = Vec::new();
    let mut fences = FenceTracker::new();
    let mut opened: Option<(usize, usize)> = None;
    let mut at = 0usize;

    for line in source.split_inclusive('\n') {
        let text = line.trim_end_matches(['\r', '\n']);
        let end = at + line.len();

        match opened {
            Some((start, body)) if text.trim() == CLOSE => {
                found.push(FoundStyle {
                    span: ByteSpan::new(start, end),
                    body: ByteSpan::new(body, at),
                });
                opened = None;
            }
            Some(_) => {}
            None if fences.feed(text) && text.trim() == OPEN => {
                opened = Some((at, end));
            }
            None => {}
        }

        at = end;
    }

    found
}

/// Removes managed style blocks and reads their custom properties.
///
/// Later blocks win, just as later CSS declarations do. Everything outside the
/// tagged ranges is copied byte for byte.
pub fn extract_style(source: &str) -> ExtractedStyle {
    let found = find_styles(source);
    if found.is_empty() {
        return ExtractedStyle { content: source.to_string(), properties: BTreeMap::new() };
    }

    let mut content = String::with_capacity(source.len());
    let mut properties = BTreeMap::new();
    let mut cursor = 0usize;

    for style in found {
        content.push_str(&source[cursor..style.span.start]);
        properties.extend(properties_in(style.body.slice(source)));
        cursor = style.span.end;
    }
    content.push_str(&source[cursor..]);

    ExtractedStyle { content, properties }
}

fn properties_in(source: &str) -> BTreeMap<String, String> {
    source
        .lines()
        .filter_map(parse_style_property)
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// Reads one complete `--slidx-*` declaration.
///
/// The returned slices point into `line`. That lets an editor replace only the
/// value's bytes while the parser and writer still share one definition of
/// which declarations belong to the managed style model.
pub fn parse_style_property(line: &str) -> Option<(&str, &str)> {
    let declaration = line.trim().strip_prefix(PREFIX)?;
    let (name, value) = declaration.split_once(':')?;
    let name = name.trim();
    let value = value.trim().strip_suffix(';')?.trim();

    if !valid_name(name) || value.is_empty() {
        return None;
    }

    Some((name, value))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tagged_css_becomes_slide_properties_and_not_content() {
        let source = concat!(
            "<style data-slidx>\n",
            ":root {\n",
            "  --slidx-layout: aside;\n",
            "  --slidx-color-surface: oklch(20% 0.02 260);\n",
            "}\n",
            "</style>\n",
            "\n",
            "# One\n",
        );
        let styled = extract_style(source);

        assert_eq!(styled.properties.get("layout").map(String::as_str), Some("aside"));
        assert_eq!(
            styled.properties.get("color-surface").map(String::as_str),
            Some("oklch(20% 0.02 260)")
        );
        assert_eq!(styled.content, "\n# One\n");
    }

    #[test]
    fn an_example_in_a_fence_remains_markdown() {
        let source =
            "```html\n<style data-slidx>\n:root { --slidx-layout: aside; }\n</style>\n```\n";
        let styled = extract_style(source);

        assert!(styled.properties.is_empty());
        assert_eq!(styled.content, source);
    }

    #[test]
    fn an_unclosed_block_remains_content_while_somebody_is_typing_it() {
        let source = "<style data-slidx>\n:root {\n  --slidx-layout: aside;\n";
        let styled = extract_style(source);

        assert!(styled.properties.is_empty());
        assert_eq!(styled.content, source);
    }

    #[test]
    fn only_complete_prefixed_declarations_cross_the_boundary() {
        let source = concat!(
            "<style data-slidx>\n",
            ":root {\n",
            "  color: red;\n",
            "  --other-layout: split;\n",
            "  --slidx-BAD: no;\n",
            "  --slidx-layout: aside\n",
            "  --slidx-density: compact;\n",
            "}\n",
            "</style>\n",
        );

        assert_eq!(
            extract_style(source).properties,
            BTreeMap::from([("density".to_string(), "compact".to_string())])
        );
    }

    #[test]
    fn a_property_keeps_slices_of_the_line_the_editor_can_replace() {
        let line = "  --slidx-layout:   aside ; ";
        let (name, value) = parse_style_property(line).unwrap();

        assert_eq!(name, "layout");
        assert_eq!(value, "aside");
        assert_eq!(name.as_ptr() as usize - line.as_ptr() as usize, 10);
        assert_eq!(value.as_ptr() as usize - line.as_ptr() as usize, 20);
    }

    #[test]
    fn later_blocks_win_like_later_css_declarations() {
        let source = concat!(
            "<style data-slidx>\n:root {\n  --slidx-layout: split;\n}\n</style>\n",
            "<style data-slidx>\n:root {\n  --slidx-layout: aside;\n}\n</style>\n",
        );

        assert_eq!(
            extract_style(source).properties.get("layout").map(String::as_str),
            Some("aside")
        );
    }
}
