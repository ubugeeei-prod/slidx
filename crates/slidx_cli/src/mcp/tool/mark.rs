//! The operations that address part of a line.
//!
//! A mark is the smallest thing anything in slidx can point at, and the reason
//! colouring three words has somewhere to go in the file. These three tools are
//! how an agent reaches one.
//!
//! The description of `add_mark` carries the take rule, because this is where an
//! agent acts on it: a value that *changes* is a second mark with the same key
//! next to the first, and an agent that reaches for a reveal and a hide instead
//! ends up with two elements that swap.

use serde_json::{json, Value};

use slidx_edit::EditOp;

use super::apply::operate;
use super::args;
use super::slide::{schema, slide_argument};
use super::{Context, Tool};
use crate::mcp::content::Answer;

pub const ALL: &[Tool] = &[
    Tool {
        name: "add_mark",
        title: "Mark a range inside a slide",
        description: "\
Wraps some of a slide's text in a mark: `[3.2x faster]{#result .accent}`. A mark \
is the smallest thing a step, a style or the visual editor can point at, so \
anything an animation targets needs one.

Say the text to wrap, not a byte range. Where that text appears more than once \
this refuses rather than marking the first, because marking the wrong three words \
leaves nothing in the file saying which was meant.

A VALUE THAT CHANGES is not two marks that swap. Write the second take next to \
the first with the same key — `[120ms]{#latency}[38ms]{#latency}` — and slidx \
compiles one element with successive states. Reveal and hide are for content that \
is not there yet, or is gone.",
        schema: add_schema,
        output: super::apply::output,
        writes: true,
        run: add_mark,
    },
    Tool {
        name: "set_mark",
        title: "Change a mark's attributes",
        description: "\
Rewrites a mark's key, classes and properties, leaving its text alone. Passing \
none of them unwraps the mark back to plain text, because `[text]{}` is not \
something anybody meant to write.",
        schema: set_schema,
        output: super::apply::output,
        writes: true,
        run: set_mark,
    },
    Tool {
        name: "remove_mark",
        title: "Unwrap a mark",
        description: "\
Removes the mark and keeps its text. Anything targeting its key — a step, a \
theme rule — stops matching, so check the slide's `steps:` before removing a mark \
an animation points at.",
        schema: mark_only_schema,
        output: super::apply::output,
        writes: true,
        run: remove_mark,
    },
];

/// A mark's attributes, described once for the two tools that set them.
fn attribute_arguments() -> Value {
    json!({
        "key": {
            "type": "string",
            "description": "\
    The mark's identifier, which a step targets as `#key`. A leading `#` is accepted. \
    Two adjacent marks sharing a key are takes of one element, not two elements.",
        },
        "classes": {
            "type": "array",
            "items": { "type": "string" },
            "description": "\
    Classes on the mark, such as `accent`. A leading `.` is accepted. What a class \
    looks like is the theme's business, so a class no theme styles renders as plain \
    text rather than failing.",
        },
        "properties": {
            "type": "object",
            "additionalProperties": { "type": "string" },
            "description": "Data properties, written as `name=value` in the mark.",
        },
    })
}

fn add_schema() -> Value {
    let mut extra = attribute_arguments();
    extra["slide"] = slide_argument();
    extra["text"] = json!({
        "type": "string",
        "description": "\
    The exact text the mark should wrap, as it appears in the slide's Markdown body. \
    Not a byte range — slidx finds it.",
    });
    extra["occurrence"] = json!({
        "type": "integer",
        "minimum": 1,
        "description": "\
    Which occurrence to mark, counting from one, when the text appears more than \
    once. Without it, repeated text is refused rather than guessed at.",
    });

    schema(extra, &["slide", "text"])
}

fn add_mark(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "add_mark", arguments, |reading| {
        let slide = args::slide(arguments)?;
        let range = args::range(reading, &slide, arguments)?;
        let attributes = args::attributes(arguments);

        if attributes.is_empty() {
            return Err(
                "A mark with no key, classes or properties would render as plain text. Give it a \
                 `key` if a step needs to target it, or a class if a theme should style it."
                    .to_string(),
            );
        }

        Ok(EditOp::AddMark { slide, range, attributes })
    })
}

/// Which mark, described once for the two tools that name one.
fn mark_argument() -> Value {
    json!({
        "anyOf": [{ "type": "integer", "minimum": 0 }, { "type": "string" }],
        "description": "\
    The mark, by position in the slide counting from zero, or by its `#key`. Only \
    marks something refers to have a key, so position is the general way to name one.",
    })
}

fn set_schema() -> Value {
    let mut extra = attribute_arguments();
    extra["slide"] = slide_argument();
    extra["mark"] = mark_argument();

    schema(extra, &["slide", "mark"])
}

fn set_mark(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "set_mark", arguments, |_| {
        Ok(EditOp::SetMark {
            slide: args::slide(arguments)?,
            mark: args::mark(arguments)?,
            attributes: args::attributes(arguments),
        })
    })
}

fn mark_only_schema() -> Value {
    schema(json!({ "slide": slide_argument(), "mark": mark_argument() }), &["slide", "mark"])
}

fn remove_mark(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "remove_mark", arguments, |_| {
        Ok(EditOp::RemoveMark { slide: args::slide(arguments)?, mark: args::mark(arguments)? })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_a_mark_takes_text_rather_than_a_byte_range() {
        // The arithmetic a model gets wrong, especially through a paragraph
        // whose characters are three bytes each.
        let schema = add_schema();

        assert_eq!(schema["properties"]["text"]["type"], "string");
        assert!(schema["properties"]["range"].is_null(), "a range is slidx's to compute");
    }

    #[test]
    fn the_take_rule_is_stated_where_an_agent_would_get_it_wrong() {
        // Not only in the server's instructions: this is the call being made at
        // the moment the choice is live.
        assert!(ALL[0].description.contains("[120ms]{#latency}[38ms]{#latency}"));
        assert!(ALL[0].description.contains("not two marks that swap"));
    }

    #[test]
    fn unwrapping_a_mark_is_documented_as_what_empty_attributes_mean() {
        assert!(ALL[1].description.contains("unwraps"), "set_mark");
    }

    #[test]
    fn removing_a_mark_warns_about_the_steps_that_pointed_at_it() {
        // A step whose target no longer matches is an animation that silently
        // does nothing, on stage.
        assert!(ALL[2].description.contains("steps:"), "remove_mark");
    }

    #[test]
    fn a_mark_can_be_named_by_position_or_by_key() {
        for tool in [&ALL[1], &ALL[2]] {
            assert!((tool.schema)()["properties"]["mark"]["anyOf"].is_array(), "{}", tool.name);
        }
    }
}
