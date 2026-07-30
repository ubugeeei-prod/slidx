//! The operations that are about a whole slide.
//!
//! Seven tools, one [`EditOp`] each, and nothing in this file composes Markdown.
//! Each one is a schema saying what a slidx author means by the argument, and a
//! closure handing the operation to [`super::apply::operate`].
//!
//! The descriptions carry more than the argument names because a model choosing
//! between `set_body` and `set_heading` is choosing between rewriting a slide and
//! renaming it, and the second is almost always what was meant.

use serde_json::{json, Value};

use slidx_edit::EditOp;

use super::apply::{deck_arguments, operate};
use super::args;
use super::{Context, Tool};
use crate::mcp::content::Answer;

/// A schema over the deck arguments plus whatever else a tool takes.
pub fn schema(extra: Value, required: &[&str]) -> Value {
    let mut properties = deck_arguments();

    for (key, value) in extra.as_object().cloned().unwrap_or_default() {
        properties[key] = value;
    }

    let mut names = vec!["deck"];
    names.extend_from_slice(required);

    json!({ "type": "object", "properties": properties, "required": names })
}

/// Which slide, described once for the seven tools that take one.
pub fn slide_argument() -> Value {
    json!({
        "anyOf": [{ "type": "integer", "minimum": 0 }, { "type": "string" }],
        "description": "\
    The slide, by zero-based index or by slug. An index is what you get from reading \
    the deck and is stable for exactly one operation; a slug is the id in the slide's \
    URL and survives everything except retitling it.",
    })
}

pub const ALL: &[Tool] = &[
    Tool {
        name: "set_heading",
        title: "Retitle a slide",
        description: "\
Rewrites a slide's heading text, keeping the heading level the author chose and \
the spacing they wrote. A slide with no heading gains one above its body.

This is what you want for renaming a slide. `set_body` replaces everything, \
which loses whatever else was on it.",
        schema: heading_schema,
        output: super::apply::output,
        writes: true,
        run: set_heading,
    },
    Tool {
        name: "set_body",
        title: "Replace a slide's Markdown",
        description: "\
Replaces a slide's Markdown body, leaving its own frontmatter block and its \
speaker notes alone. Everything else on the slide goes, so reach for \
`set_heading`, `add_mark` or `set_notes` when only part of it should change — \
those splice, and this one replaces.",
        schema: body_schema,
        output: super::apply::output,
        writes: true,
        run: set_body,
    },
    Tool {
        name: "set_notes",
        title: "Write what the speaker says over a slide",
        description: "\
Replaces the speaker notes — the `<!-- notes: ... -->` comment. An empty string \
removes them.

Notes are what the speaker SAYS, not a second copy of what the audience reads. \
They also drive the spoken-length estimate the timing report compares against \
the slot, so notes that restate the slide make that estimate wrong.",
        schema: notes_schema,
        output: super::apply::output,
        writes: true,
        run: set_notes,
    },
    Tool {
        name: "set_field",
        title: "Write a frontmatter key",
        description: "\
Sets one key in a slide's frontmatter block, creating the block if it has none. \
The first slide's block is the DECK's, which is where `title`, `event`, \
`duration`, `theme` and `aspect` live.

Only the key named is rewritten. The order of the other keys, and the author's \
own spacing and quoting, are untouched — which is the difference between a \
reviewable diff and a reordered file.",
        schema: field_schema,
        output: super::apply::output,
        writes: true,
        run: set_field,
    },
    Tool {
        name: "insert_slide",
        title: "Add a slide",
        description: "\
Inserts a slide at a position, pushing the slide currently there down. `at` \
equal to the number of slides appends.

In a deck kept as one file per slide, the new slide joins the file it displaced \
rather than getting one of its own — renaming files is the author's business and \
not something an edit should do behind them.",
        schema: insert_schema,
        output: super::apply::output,
        writes: true,
        run: insert_slide,
    },
    Tool {
        name: "remove_slide",
        title: "Delete a slide",
        description: "\
Removes a slide and its notes. A slide file left holding no slides is deleted \
rather than emptied, because an empty slide file joins the deck as a blank slide.

The inverse comes back with the answer and goes on the undo stack, so this is \
recoverable within the session — but the file is gone from disk, so a deck under \
version control is the real safety net.",
        schema: slide_only_schema,
        output: super::apply::output,
        writes: true,
        run: remove_slide,
    },
    Tool {
        name: "move_slide",
        title: "Reorder a slide",
        description: "\
Moves a slide to a new position, counted AFTER the slide is lifted out — so \
moving slide 0 to 2 in a five-slide deck puts it third.

In a deck of one file per slide the bytes move between files, and the files keep \
their names. A reordered deck therefore has slide 3 in `0002.md`, which is what \
the author would have got by editing by hand.",
        schema: move_schema,
        output: super::apply::output,
        writes: true,
        run: move_slide,
    },
];

fn heading_schema() -> Value {
    schema(
        json!({
            "slide": slide_argument(),
            "text": {
                "type": "string",
                "description": "The heading's new text, without the leading `#` characters.",
            },
        }),
        &["slide", "text"],
    )
}

fn set_heading(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "set_heading", arguments, |_| {
        Ok(EditOp::SetHeading {
            slide: args::slide(arguments)?,
            text: args::required(arguments, "text", "the heading's new text.")?,
        })
    })
}

fn body_schema() -> Value {
    schema(
        json!({
            "slide": slide_argument(),
            "body": {
                "type": "string",
                "description": "\
        The slide's new Markdown, without its frontmatter block and without its notes \
        comment. Both of those survive on their own.",
            },
        }),
        &["slide", "body"],
    )
}

fn set_body(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "set_body", arguments, |_| {
        Ok(EditOp::SetBody {
            slide: args::slide(arguments)?,
            body: args::string(arguments, "body")
                .ok_or_else(|| "`body` is required: the slide's new Markdown.".to_string())?,
        })
    })
}

fn notes_schema() -> Value {
    schema(
        json!({
            "slide": slide_argument(),
            "notes": {
                "type": "string",
                "description": "\
        What the speaker says over this slide. An empty string removes the notes \
        entirely.",
            },
        }),
        &["slide", "notes"],
    )
}

fn set_notes(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "set_notes", arguments, |_| {
        Ok(EditOp::SetNotes {
            slide: args::slide(arguments)?,
            // An empty string is meaningful here — it is how notes are removed —
            // so absent and empty have to be told apart.
            notes: args::string(arguments, "notes").ok_or_else(|| {
                "`notes` is required: what the speaker says, or an empty string to remove them."
                    .to_string()
            })?,
        })
    })
}

fn field_schema() -> Value {
    schema(
        json!({
            "slide": slide_argument(),
            "key": {
                "type": "string",
                "description": "\
        The frontmatter key. Deck-level keys live on slide 0: `title`, `description`, \
        `author`, `event`, `date`, `venue`, `hashtag`, `url`, `repo`, `theme`, \
        `transition`, `aspect` (16:9, 16:10 or 4:3) and `duration` (`20m`). Slide-level \
        keys include `layout`, `budget` (`90s`), `optional`, `autoSteps` and `steps`.",
            },
            "value": {
                "description": "\
        The value, as JSON. A string, a number, a boolean, or a list — whatever the key \
        takes. `null` removes the key.",
            },
        }),
        &["slide", "key", "value"],
    )
}

fn set_field(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "set_field", arguments, |_| {
        Ok(EditOp::SetField {
            slide: args::slide(arguments)?,
            key: args::required(arguments, "key", "the frontmatter key to write.")?,
            value: arguments
                .get("value")
                .cloned()
                .ok_or_else(|| "`value` is required: what the key should say.".to_string())?,
        })
    })
}

fn insert_schema() -> Value {
    schema(
        json!({
            "at": {
                "type": "integer",
                "minimum": 0,
                "description": "\
        Where the slide goes, counting from zero. Equal to the number of slides appends.",
            },
            "body": {
                "type": "string",
                "description": "The new slide's Markdown, heading included.",
            },
        }),
        &["at", "body"],
    )
}

fn insert_slide(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "insert_slide", arguments, |_| {
        Ok(EditOp::InsertSlide {
            at: args::number(arguments, "at").ok_or_else(|| {
                "`at` is required: where the slide goes, counting from zero.".to_string()
            })?,
            body: args::string(arguments, "body")
                .ok_or_else(|| "`body` is required: the new slide's Markdown.".to_string())?,
        })
    })
}

fn slide_only_schema() -> Value {
    schema(json!({ "slide": slide_argument() }), &["slide"])
}

fn remove_slide(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "remove_slide", arguments, |_| {
        Ok(EditOp::RemoveSlide { slide: args::slide(arguments)? })
    })
}

fn move_schema() -> Value {
    schema(
        json!({
            "slide": slide_argument(),
            "to": {
                "type": "integer",
                "minimum": 0,
                "description": "\
        The new position, counted after the slide is lifted out. Moving slide 0 to 2 in \
        a five-slide deck puts it third.",
            },
        }),
        &["slide", "to"],
    )
}

fn move_slide(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    operate(context, "move_slide", arguments, |_| {
        Ok(EditOp::MoveSlide {
            slide: args::slide(arguments)?,
            to: args::number(arguments, "to")
                .ok_or_else(|| "`to` is required: the slide's new position.".to_string())?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slide_tool_takes_a_deck_and_says_why_it_has_to_be_inside_a_root() {
        for tool in ALL {
            let schema = (tool.schema)();

            assert_eq!(schema["properties"]["deck"]["type"], "string", "{}", tool.name);
            assert!(
                schema["required"].as_array().expect("required").contains(&json!("deck")),
                "{} does not require a deck",
                tool.name
            );
            assert!(schema["properties"]["separator"].is_object(), "{}", tool.name);
        }
    }

    #[test]
    fn every_slide_tool_writes_and_says_so() {
        // The flag the read-only gate reads. A mutating tool that claimed
        // otherwise would be listed to a client that cannot call it.
        assert!(ALL.iter().all(|tool| tool.writes));
    }

    #[test]
    fn a_slide_can_always_be_named_either_way() {
        // An index is what reading a deck gives you; a slug survives a reorder.
        // A tool that took only one of them would force a re-read.
        for tool in ALL.iter().filter(|tool| tool.name != "insert_slide") {
            let named = (tool.schema)()["properties"]["slide"]["anyOf"].clone();

            assert_eq!(
                named,
                json!([{ "type": "integer", "minimum": 0 }, { "type": "string" }]),
                "{}",
                tool.name
            );
        }
    }

    #[test]
    fn the_deck_arguments_are_spliced_in_rather_than_restated() {
        // Twelve copies of one description is twelve places for it to go stale.
        let one = schema(json!({}), &[]);
        let two = (ALL[0].schema)();

        assert_eq!(one["properties"]["deck"], two["properties"]["deck"]);
    }

    #[test]
    fn the_body_and_the_heading_tools_say_which_one_a_rename_wants() {
        // The choice a model gets wrong: replacing a whole slide to change its
        // title, which silently drops everything else on it.
        assert!(ALL[0].description.contains("renaming"), "set_heading");
        assert!(ALL[1].description.contains("set_heading"), "set_body");
    }
}
