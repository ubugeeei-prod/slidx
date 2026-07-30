//! The operations that stage a slide.
//!
//! Two tools over the `steps:` list. The interesting part is the schema rather
//! than the code: a step is a *snapshot*, and the vocabulary an agent is offered
//! has to make that hard to misread. So the effect presets are enumerated from
//! the Rust enum rather than restated as a list here, and the description says
//! what each action kind is for and which gesture is not one of them.

use serde_json::{json, Value};

use slidx_core::EffectPreset;
use slidx_edit::EditOp;

use super::apply::operate;
use super::args;
use super::slide::{schema, slide_argument};
use super::{Context, Tool};
use crate::mcp::content::Answer;

pub const ALL: &[Tool] = &[
    Tool {
        name: "add_step",
        title: "Stage part of a slide",
        description: "\
Appends one intent to a slide's `steps:` list, creating it if there is none. Each \
intent costs the speaker one advance.

Steps are SNAPSHOTS, not deltas: slidx compiles the list into a vector of complete \
states, so advancing, going back, deep-linking to `?step=7` and printing all index \
into the same vector and cannot drift. Describe what a stop looks like, never what \
to do on a click.

  reveal     content that is not on screen yet
  hide       content that has been and is now gone
  emphasize  content already on screen, drawn attention to
  set        content already on screen, changed in place
  group      several of the above landing on one advance

A value that CHANGES is usually none of these: write two takes of one mark instead \
— `[120ms]{#latency}[38ms]{#latency}` — and slidx compiles one element with \
successive states. Reach for `set` when a PROPERTY changes rather than the text.

Anything targeted needs a mark with that key first; `add_mark` makes one.",
        schema: add_schema,
        output: super::apply::output,
        writes: true,
        run: add_step,
    },
    Tool {
        name: "remove_step",
        title: "Take a stop out of a slide",
        description: "\
Removes one intent from a slide's `steps:` list by position. Every later step \
moves up, so removing several means working backwards or re-reading the list \
between calls.

An element whose only mention was the removed step goes back to being part of the \
slide from the start, which is what it was before anybody staged it.",
        schema: remove_schema,
        output: super::apply::output,
        writes: true,
        run: remove_step,
    },
];

fn add_schema() -> Value {
    schema(
        json!({
            "slide": slide_argument(),
            "action": {
                "type": "string",
                "enum": ["reveal", "hide", "emphasize", "set", "group"],
                "description": "What happens at this stop.",
            },
            "target": {
                "type": "string",
                "description": "\
        What the step acts on: a mark's key as `#latency`, or a CSS selector into the \
        slide. Required for everything except a group.",
            },
            "content": {
                "type": "string",
                "description": "For `set`: the element's new text.",
            },
            "properties": {
                "type": "object",
                "additionalProperties": { "type": "string" },
                "description": "For `set`: data properties to write on the element.",
            },
            "actions": {
                "type": "array",
                "items": { "type": "object" },
                "description": "\
        For `group`: the intents that land on the same advance, each shaped like this \
        object.",
            },
            "preset": {
                "type": "string",
                "enum": presets(),
                "description": "\
        The animation. Left out, slidx picks the one that suits the action, and nothing \
        is written into the deck — an editor that spells out every key turns a one-word \
        change into a diff across the whole slide.",
            },
            "durationMs": {
                "type": "integer",
                "minimum": 0,
                "description": "How long the animation runs. Default 400.",
            },
            "afterMs": {
                "type": "integer",
                "minimum": 0,
                "description": "\
        Play this many milliseconds after the stop it belongs to, INSTEAD of consuming an \
        advance of its own. This is how two things happen on one click without a group.",
            },
        }),
        &["slide", "action"],
    )
}

/// Every preset, read from the Rust enum that defines them.
///
/// Not restated as a list. A preset added to `slidx_core` has to appear here
/// without anybody remembering to add it, or the vocabulary an agent is offered
/// is a stale copy of slidx's own.
fn presets() -> Vec<&'static str> {
    EffectPreset::ALL.iter().map(|preset| preset.as_token()).collect()
}

fn add_step(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "add_step", arguments, |_| {
        Ok(EditOp::AddStep { slide: args::slide(arguments)?, action: args::action(arguments)? })
    })
}

fn remove_schema() -> Value {
    schema(
        json!({
            "slide": slide_argument(),
            "index": {
                "type": "integer",
                "minimum": 0,
                "description": "Which step, counting from zero in the order they were written.",
            },
        }),
        &["slide", "index"],
    )
}

fn remove_step(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "remove_step", arguments, |_| {
        Ok(EditOp::RemoveStep {
            slide: args::slide(arguments)?,
            index: args::number(arguments, "index").ok_or_else(|| {
                "`index` is required: which step, counting from zero.".to_string()
            })?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_offered_are_the_ones_slidx_has() {
        // Read from the enum, so a preset added in slidx_core cannot be missing
        // here and a preset removed cannot be offered.
        let offered = add_schema()["properties"]["preset"]["enum"].clone();

        assert_eq!(offered.as_array().expect("presets").len(), EffectPreset::ALL.len());
        assert!(offered.as_array().expect("presets").contains(&json!("fly-in")));
    }

    #[test]
    fn the_snapshot_rule_is_stated_where_a_step_is_written() {
        assert!(ALL[0].description.contains("SNAPSHOTS, not deltas"));
        assert!(ALL[0].description.contains("never what"), "{}", ALL[0].description);
    }

    #[test]
    fn each_action_kind_says_what_it_is_for() {
        for kind in ["reveal", "hide", "emphasize", "set", "group"] {
            assert!(ALL[0].description.contains(kind), "add_step never mentions {kind}");
        }
    }

    #[test]
    fn the_take_alternative_is_offered_beside_the_actions_it_is_mistaken_for() {
        // The failure: reaching for hide-and-reveal when the gesture is a value
        // that changes.
        assert!(ALL[0].description.contains("[120ms]{#latency}[38ms]{#latency}"));
    }

    #[test]
    fn removing_a_step_warns_that_the_later_ones_move() {
        // Removing steps 1 and 2 by index in that order removes 1 and 3.
        assert!(ALL[1].description.contains("moves up"), "{}", ALL[1].description);
    }

    #[test]
    fn playing_after_a_stop_is_documented_as_the_alternative_to_a_group() {
        let schema = add_schema();
        let after = schema["properties"]["afterMs"]["description"].as_str().expect("a text");

        assert!(after.contains("INSTEAD of consuming an advance"), "{after}");
    }
}
