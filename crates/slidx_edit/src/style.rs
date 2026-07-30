//! Writing one slide-local custom property into Markdown.
//!
//! A visual choice belongs in the file beside the slide it changes:
//!
//! ```text
//! <style data-slidx>
//! :root {
//!   --slidx-layout: aside;
//! }
//! </style>
//! ```
//!
//! The operation still obeys the edit crate's law. An existing value is one
//! byte-range replacement; a missing property is one inserted line; everything
//! else in the style block and the Markdown stays byte-identical. Removing a
//! property removes every declaration of that name, otherwise an earlier CSS
//! declaration would silently become active again.

use slidx_core::style::{CLOSE, OPEN};
use slidx_core::{find_styles, parse_style_property, ByteSpan, FoundStyle};

use crate::edit::EditBuilder;
use crate::op::{EditError, SlideRef};
use crate::source::DeckSource;

#[derive(Debug, Clone)]
struct Declaration<'a> {
    name: &'a str,
    line: ByteSpan,
    value: ByteSpan,
}

pub(crate) fn set(
    deck: &DeckSource<'_>,
    slide: &SlideRef,
    property: &str,
    value: Option<&str>,
    builder: &mut EditBuilder<'_>,
) -> Result<(), EditError> {
    if !valid_property(property) {
        return Err(EditError::InvalidStyleProperty { property: property.to_string() });
    }

    let value = value.map(str::trim);
    if value.is_some_and(|value| !valid_value(value)) {
        return Err(EditError::InvalidStyleValue { property: property.to_string() });
    }

    let index = deck.resolve(slide)?;
    let body = deck.at(index).body;
    let source = body.slice(deck.source);
    let styles = find_styles(source);
    let declarations: Vec<_> =
        styles.iter().flat_map(|style| declarations(source, *style)).collect();
    let matching: Vec<_> =
        declarations.iter().filter(|declaration| declaration.name == property).collect();

    match value {
        Some(value) if !value.is_empty() => {
            if let Some(found) = matching.last() {
                builder.replace(found.value.shifted(body.start), value);
            } else if let Some(style) = styles.last() {
                insert_declaration(
                    source,
                    *style,
                    &declarations,
                    property,
                    value,
                    deck.newline(),
                    body.start,
                    builder,
                );
            } else {
                insert_block(deck, body, property, value, builder);
            }
        }
        _ => {
            for found in matching {
                builder.delete(found.line.shifted(body.start));
            }
        }
    }

    Ok(())
}

fn valid_property(property: &str) -> bool {
    let probe = format!("--slidx-{property}: value;");
    parse_style_property(&probe).is_some_and(|(name, _)| name == property)
}

fn valid_value(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['\r', '\n', ';'])
        && !value.to_ascii_lowercase().contains(CLOSE)
}

fn declarations(source: &str, style: FoundStyle) -> Vec<Declaration<'_>> {
    let body = style.body.slice(source);
    let mut found = Vec::new();
    let mut cursor = 0usize;

    for line in body.split_inclusive('\n') {
        let text = line.trim_end_matches(['\r', '\n']);
        if let Some((name, value)) = parse_style_property(text) {
            let text_start = text.as_ptr() as usize;
            let value_start = value.as_ptr() as usize - text_start;

            found.push(Declaration {
                name,
                line: ByteSpan::new(
                    style.body.start + cursor,
                    style.body.start + cursor + line.len(),
                ),
                value: ByteSpan::new(
                    style.body.start + cursor + value_start,
                    style.body.start + cursor + value_start + value.len(),
                ),
            });
        }
        cursor += line.len();
    }

    found
}

#[allow(clippy::too_many_arguments)]
fn insert_declaration(
    source: &str,
    style: FoundStyle,
    declarations: &[Declaration<'_>],
    property: &str,
    value: &str,
    newline: &str,
    shift: usize,
    builder: &mut EditBuilder<'_>,
) {
    let inside: Vec<_> =
        declarations.iter().filter(|declaration| style.body.contains(declaration.line)).collect();

    if let Some(last) = inside.last() {
        let line = last.line.slice(source);
        let indent =
            line.chars().take_while(|character| character.is_whitespace()).collect::<String>();
        let (at, before) = if line.ends_with('\n') {
            (last.line.end, String::new())
        } else {
            (last.line.end, newline.to_string())
        };

        builder
            .insert(shift + at, format!("{before}{indent}--slidx-{property}: {value};{newline}"));
        return;
    }

    let body = style.body.slice(source);
    if let Some(close) = closing_root_line(body) {
        builder.insert(
            shift + style.body.start + close,
            format!("  --slidx-{property}: {value};{newline}"),
        );
        return;
    }

    let before = if !body.is_empty() && !body.ends_with('\n') { newline } else { "" };
    builder.insert(
        shift + style.body.end,
        format!("{before}:root {{{newline}  --slidx-{property}: {value};{newline}}}{newline}"),
    );
}

fn closing_root_line(source: &str) -> Option<usize> {
    let mut cursor = 0usize;
    let mut found = None;

    for line in source.split_inclusive('\n') {
        if line.trim() == "}" {
            found = Some(cursor);
        }
        cursor += line.len();
    }

    found
}

fn insert_block(
    deck: &DeckSource<'_>,
    body: ByteSpan,
    property: &str,
    value: &str,
    builder: &mut EditBuilder<'_>,
) {
    let newline = deck.newline();
    let after = if body.is_empty() { newline.to_string() } else { deck.blank() };
    let block = format!(
        "{OPEN}{newline}:root {{{newline}  --slidx-{property}: {value};{newline}}}{newline}{CLOSE}{after}"
    );

    builder.insert(body.start, block);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_keep_the_value_and_whole_line_spans() {
        let source = "<style data-slidx>\n:root {\n  --slidx-layout:   aside ;\n}\n</style>\n";
        let style = find_styles(source)[0];
        let found = declarations(source, style);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "layout");
        assert_eq!(found[0].value.slice(source), "aside");
        assert_eq!(found[0].line.slice(source), "  --slidx-layout:   aside ;\n");
    }

    #[test]
    fn a_closing_root_line_is_found_without_parsing_css() {
        assert_eq!(closing_root_line(":root {\n  color: red;\n}\n"), Some(22));
        assert_eq!(closing_root_line("not a root\n"), None);
    }
}
