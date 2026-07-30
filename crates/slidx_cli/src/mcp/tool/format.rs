//! Normalising the parts of a deck slidx owns.
//!
//! Its own module rather than an arm in [`super::slide`] because it is not an
//! operation: `slidx_fmt` computes the whole edit itself, over the whole deck,
//! and nothing here names a slide.
//!
//! What makes it belong beside the operations anyway is that it produces the
//! *same* [`slidx_edit::Edit`], so it goes on the undo stack and comes back with
//! its inverse like everything else. That is not a coincidence — it is what
//! having one representation of a change buys, and the reason an agent can
//! format a deck it does not own and reverse it exactly.

use serde_json::{json, Value};

use super::apply::deck_arguments;
use super::args;
use super::{Context, Tool};
use crate::mcp::content::Answer;
use crate::mcp::edit;
use crate::mcp::history::Entry;

pub const ALL: &[Tool] = &[Tool {
    name: "format_deck",
    title: "Normalise the parts of a deck slidx owns",
    description: "\
`slidx fmt`: frontmatter key order and indentation, the slide separator's \
spelling, step marker spelling, the attribute order inside a mark's braces, and \
the shape of a notes comment.

It is NOT a Markdown formatter and will not become one. Prose, line wrapping, \
bullet markers, table alignment and everything inside a fenced code block come \
out byte for byte, because slidx does not own them.

Reach for this instead of tidying Markdown by hand — hand-tidying touches bytes \
nobody asked you to change, which is what makes a diff unreadable. Like every \
other change here it is one `undo` away from being reversed exactly.",
    schema: arguments,
    output: super::apply::output,
    writes: true,
    run: format,
}];

fn arguments() -> Value {
    json!({
        "type": "object",
        "properties": deck_arguments(),
        "required": ["deck"],
    })
}

fn format(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    let path = args::required(arguments, "deck", "the deck to format.")?;
    let reading = context.workspace.edit_deck(&path, args::text(arguments, "separator"))?;
    let applied = edit::format(&reading)?;

    context.history.record(Entry {
        tool: "format_deck",
        deck: reading.path.clone(),
        separator: reading.separator.clone(),
        files: reading.files.clone(),
        inverse: applied.inverse.clone(),
    });

    let text = if applied.redundant {
        "`format_deck`: already formatted, so nothing was written.".to_string()
    } else {
        format!(
            "`format_deck`: normalised {}. Only the parts slidx owns changed — prose, \
             wrapping, bullet markers and code fences are byte for byte what they were. \
             {} change{} on this session's undo stack.",
            applied.changed.join(", "),
            context.history.len(),
            if context.history.len() == 1 { "" } else { "s" },
        )
    };

    Ok(Answer::text(text).with_data(json!({
        "changed": applied.changed,
        "inverse": applied.inverse,
        "undoDepth": context.history.len(),
        "slides": reading.deck.slides.len(),
        "redundant": applied.redundant,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_description_says_what_it_will_not_touch() {
        // A model that believed this was a Markdown formatter would reach for it
        // to rewrap a paragraph, and be surprised when nothing happened.
        assert!(ALL[0].description.contains("NOT a Markdown formatter"));
        assert!(ALL[0].description.contains("byte for byte"));
    }

    #[test]
    fn it_is_offered_as_the_alternative_to_tidying_by_hand() {
        assert!(ALL[0].description.contains("instead of tidying Markdown by hand"));
    }

    #[test]
    fn it_names_no_slide_because_it_is_not_an_operation() {
        let schema = arguments();

        assert!(schema["properties"]["slide"].is_null());
        assert_eq!(schema["required"], json!(["deck"]));
    }

    #[test]
    fn it_answers_in_the_same_shape_every_other_change_does() {
        // Including the inverse, which is what puts it on the undo stack.
        assert_eq!((ALL[0].output)(), super::super::apply::output());
    }
}
