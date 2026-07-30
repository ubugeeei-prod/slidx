//! The deliberately small, static part of MDX that a deck can make interactive.
//!
//! MDX is opt-in and the Markdown file remains the source of truth. This module
//! reads its JSX AST only while rendering, then turns capitalised component
//! elements into the same static-first island contract authors can write by
//! hand. It never evaluates JavaScript: props must be JSON values, which makes
//! a build deterministic and keeps untrusted deck source from becoming build
//! process code.

use std::collections::BTreeMap;

use markdown::mdast::{
    AttributeContent, AttributeValue, MdxJsxFlowElement, MdxJsxTextElement, Node,
};
use markdown::{to_mdast, ParseOptions};
use serde_json::{Map, Value};

use crate::markdown::MarkdownOptions;

/// One blocking problem in opt-in MDX source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdxIssue {
    pub code: &'static str,
    pub message: String,
    pub help: &'static str,
}

#[derive(Debug)]
pub(crate) struct CompiledMdx {
    pub source: String,
    pub issues: Vec<MdxIssue>,
}

#[derive(Debug)]
pub(crate) struct Replacement {
    pub start: usize,
    pub end: usize,
    pub value: String,
    flow: bool,
}

/// Finds every problem the renderer would make visible without changing the
/// author's source.
pub fn validate(source: &str, options: &MarkdownOptions) -> Vec<MdxIssue> {
    if !options.mdx {
        return Vec::new();
    }

    compile(source, options).issues
}

pub(crate) fn compile(source: &str, options: &MarkdownOptions) -> CompiledMdx {
    let tree = match to_mdast(source, &ParseOptions::mdx()) {
        Ok(tree) => tree,
        Err(error) => {
            return CompiledMdx {
                source: source.to_string(),
                issues: vec![MdxIssue {
                    code: "mdx/syntax",
                    message: error.to_string(),
                    help: "fix the MDX syntax or remove `mdx: true` from this deck",
                }],
            };
        }
    };

    let mut replacements = Vec::new();
    let mut issues = Vec::new();
    collect(&tree, source, options, &mut replacements, &mut issues);

    replacements.sort_by_key(|replacement| replacement.start);
    let mut compiled = source.to_string();
    for replacement in replacements.into_iter().rev() {
        compiled.replace_range(replacement.start..replacement.end, &replacement.value);
    }

    CompiledMdx { source: compiled, issues }
}

/// Complete flow components in one slide.
///
/// The core model intentionally splits top-level Markdown blocks before the
/// renderer sees them, while one MDX component can contain several such
/// blocks. The region renderer uses this view to replace the whole range at the
/// first block and skip the rest, preserving every later block index.
pub(crate) fn flow_replacements(source: &str, options: &MarkdownOptions) -> Vec<Replacement> {
    let Ok(tree) = to_mdast(source, &ParseOptions::mdx()) else {
        return Vec::new();
    };
    let mut replacements = Vec::new();
    let mut issues = Vec::new();
    collect(&tree, source, options, &mut replacements, &mut issues);
    replacements.retain(|replacement| replacement.flow);
    replacements.sort_by_key(|replacement| replacement.start);
    replacements
}

fn collect(
    node: &Node,
    source: &str,
    options: &MarkdownOptions,
    replacements: &mut Vec<Replacement>,
    issues: &mut Vec<MdxIssue>,
) {
    match node {
        // A deck's visual state is deliberately persisted in a Markdown
        // `<style>` element. CSS braces are CSS, never MDX expressions.
        Node::MdxJsxFlowElement(element) if element.name.as_deref() == Some("style") => {}
        Node::MdxJsxTextElement(element) if element.name.as_deref() == Some("style") => {}
        Node::MdxJsxFlowElement(element) if component_name(element.name.as_deref()).is_some() => {
            replacements.push(flow_replacement(element, source, options, issues));
        }
        Node::MdxJsxTextElement(element) if component_name(element.name.as_deref()).is_some() => {
            replacements.push(text_replacement(element, source, options, issues));
        }
        Node::MdxFlowExpression(expression) => {
            replacements.push(expression_replacement(
                &expression.value,
                expression.position.as_ref(),
                false,
                issues,
            ));
        }
        Node::MdxTextExpression(expression) => {
            replacements.push(expression_replacement(
                &expression.value,
                expression.position.as_ref(),
                true,
                issues,
            ));
        }
        _ => {
            if let Some(children) = node.children() {
                for child in children {
                    collect(child, source, options, replacements, issues);
                }
            }
        }
    }
}

fn expression_replacement(
    expression: &str,
    position: Option<&markdown::unist::Position>,
    inline: bool,
    issues: &mut Vec<MdxIssue>,
) -> Replacement {
    let position = position.expect("MDX parser always positions source nodes");
    let parsed = serde_json::from_str::<Value>(expression.trim());
    let value = match parsed {
        Ok(Value::Null) => String::new(),
        Ok(Value::String(value)) => escape_text(&value),
        Ok(value @ (Value::Bool(_) | Value::Number(_))) => escape_text(&value.to_string()),
        Ok(value @ (Value::Array(_) | Value::Object(_))) => escape_text(&value.to_string()),
        Err(_) => {
            issues.push(MdxIssue {
                code: "mdx/non-static-expression",
                message: "an MDX expression is not a JSON value".to_string(),
                help: "use literal Markdown, a JSON value, or move runtime state into a registered island",
            });
            format!(
                "<span data-slidx-mdx-error=\"non-static expression\">{{{}}}</span>",
                escape_text(expression)
            )
        }
    };

    Replacement { start: position.start.offset, end: position.end.offset, value, flow: !inline }
}

fn flow_replacement(
    element: &MdxJsxFlowElement,
    source: &str,
    options: &MarkdownOptions,
    issues: &mut Vec<MdxIssue>,
) -> Replacement {
    element_replacement(
        element.name.as_deref().expect("guarded component has a name"),
        &element.attributes,
        &element.children,
        element.position.as_ref(),
        source,
        options,
        false,
        issues,
    )
}

fn text_replacement(
    element: &MdxJsxTextElement,
    source: &str,
    options: &MarkdownOptions,
    issues: &mut Vec<MdxIssue>,
) -> Replacement {
    element_replacement(
        element.name.as_deref().expect("guarded component has a name"),
        &element.attributes,
        &element.children,
        element.position.as_ref(),
        source,
        options,
        true,
        issues,
    )
}

#[allow(clippy::too_many_arguments)]
fn element_replacement(
    name: &str,
    attributes: &[AttributeContent],
    children: &[Node],
    position: Option<&markdown::unist::Position>,
    source: &str,
    options: &MarkdownOptions,
    inline: bool,
    issues: &mut Vec<MdxIssue>,
) -> Replacement {
    let position = position.expect("MDX parser always positions source nodes");
    let fallback_source = children_source(children, source);
    let fallback = if fallback_source.is_empty() {
        String::new()
    } else {
        let rendered = crate::markdown::render(fallback_source, options);
        if inline {
            unwrap_paragraph(rendered)
        } else {
            rendered
        }
    };

    let (props, valid) = props(attributes, name, issues);
    let tag = if inline { "span" } else { "div" };
    let value = if valid {
        let props = serde_json::to_string(&Value::Object(props))
            .expect("JSON values collected from JSON always serialise");
        format!(
            "<{tag} data-slidx-island=\"{}\" data-slidx-island-props=\"{}\">{fallback}</{tag}>",
            escape_attribute(name),
            escape_attribute(&props)
        )
    } else {
        format!(
            "<{tag} data-slidx-mdx-error=\"invalid props for {}\">{fallback}</{tag}>",
            escape_attribute(name)
        )
    };

    Replacement { start: position.start.offset, end: position.end.offset, value, flow: !inline }
}

fn component_name(name: Option<&str>) -> Option<&str> {
    let name = name?;
    name.chars().next().filter(|character| character.is_uppercase())?;
    Some(name)
}

fn props(
    attributes: &[AttributeContent],
    component: &str,
    issues: &mut Vec<MdxIssue>,
) -> (Map<String, Value>, bool) {
    let mut props = BTreeMap::new();
    let mut valid = true;

    for attribute in attributes {
        match attribute {
            AttributeContent::Expression(_) => {
                valid = false;
                issues.push(MdxIssue {
                    code: "mdx/non-static-props",
                    message: format!(
                        "`{component}` uses a spread prop, which cannot be serialised without executing JavaScript"
                    ),
                    help: "write each prop explicitly with a JSON value",
                });
            }
            AttributeContent::Property(property) => {
                let value = match &property.value {
                    None => Value::Bool(true),
                    Some(AttributeValue::Literal(value)) => Value::String(value.clone()),
                    Some(AttributeValue::Expression(expression)) => {
                        match serde_json::from_str(expression.value.trim()) {
                            Ok(value) => value,
                            Err(_) => {
                                valid = false;
                                issues.push(MdxIssue {
                                    code: "mdx/non-static-props",
                                    message: format!(
                                        "`{component}` prop `{}` is not a JSON value",
                                        property.name
                                    ),
                                    help: "use a string attribute or JSON number, boolean, null, array, or object",
                                });
                                Value::Null
                            }
                        }
                    }
                };
                props.insert(property.name.clone(), value);
            }
        }
    }

    (props.into_iter().collect(), valid)
}

fn children_source<'a>(children: &[Node], source: &'a str) -> &'a str {
    let Some(first) = children.first().and_then(Node::position) else {
        return "";
    };
    let Some(last) = children.last().and_then(Node::position) else {
        return "";
    };
    source.get(first.start.offset..last.end.offset).unwrap_or("")
}

fn unwrap_paragraph(rendered: String) -> String {
    rendered
        .strip_prefix("<p>")
        .and_then(|inner| inner.strip_suffix("</p>\n"))
        .unwrap_or(&rendered)
        .to_string()
}

fn escape_attribute(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

fn escape_text(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> MarkdownOptions {
        MarkdownOptions { mdx: true, highlight: false, ..MarkdownOptions::default() }
    }

    #[test]
    fn capitalised_components_become_static_first_islands() {
        let compiled = compile(
            "<Counter start={128} label=\"people\" enabled>\n\n**128 people**\n\n</Counter>",
            &options(),
        );

        assert!(compiled.issues.is_empty(), "{:?}", compiled.issues);
        assert!(compiled.source.contains("data-slidx-island=\"Counter\""));
        assert!(compiled.source.contains(
            "data-slidx-island-props=\"{&quot;enabled&quot;:true,&quot;label&quot;:&quot;people&quot;,&quot;start&quot;:128}\""
        ));
        assert!(compiled.source.contains("<strong>128 people</strong>"));
    }

    #[test]
    fn inline_components_remain_inline() {
        let compiled = compile("Now <Badge tone=\"good\">ready</Badge>.", &options());

        assert_eq!(
            compiled.source,
            "Now <span data-slidx-island=\"Badge\" data-slidx-island-props=\"{&quot;tone&quot;:&quot;good&quot;}\">ready</span>."
        );
    }

    #[test]
    fn lowercase_html_and_code_fences_are_never_components() {
        let source =
            "<section><strong>Static</strong></section>\n\n```mdx\n<Counter value={1} />\n```\n";
        let compiled = compile(source, &options());

        assert_eq!(compiled.source, source);
        assert!(compiled.issues.is_empty());
    }

    #[test]
    fn markdown_style_blocks_remain_opaque() {
        let source = "<style>\n.slidx-slide {\n  color: var(--slidx-ink);\n}\n</style>\n";
        let compiled = compile(source, &options());

        assert_eq!(compiled.source, source);
        assert!(compiled.issues.is_empty());
    }

    #[test]
    fn byte_offsets_survive_non_ascii_text() {
        let compiled = compile("観客: <Count value={3} />", &options());

        assert!(compiled.source.starts_with("観客: <span"));
        assert!(compiled.source.contains("data-slidx-island=\"Count\""));
    }

    #[test]
    fn dynamic_and_spread_props_block_mounting() {
        let compiled = compile("<Counter value={total} {...rest}>128</Counter>", &options());

        assert_eq!(compiled.issues.len(), 2);
        assert!(compiled.source.contains("data-slidx-mdx-error"));
        assert!(!compiled.source.contains("data-slidx-island=\""));
        assert_eq!(compiled.issues[0].code, "mdx/non-static-props");
    }

    #[test]
    fn standalone_expressions_are_static_or_blocking() {
        let static_value = compile("Attendees: {128}", &options());
        assert_eq!(static_value.source, "Attendees: 128");
        assert!(static_value.issues.is_empty());

        let dynamic = compile("Attendees: {window.total}", &options());
        assert_eq!(dynamic.issues[0].code, "mdx/non-static-expression");
        assert!(dynamic.source.contains("data-slidx-mdx-error"));
        assert!(dynamic.source.contains("{window.total}"));
    }

    #[test]
    fn mdx_syntax_errors_leave_the_source_available() {
        let source = "A broken expression: {";
        let compiled = compile(source, &options());

        assert_eq!(compiled.source, source);
        assert_eq!(compiled.issues[0].code, "mdx/syntax");
    }
}
