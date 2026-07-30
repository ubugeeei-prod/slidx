//! The one path every mutation takes.
//!
//! Read the deck, ask [`slidx_edit`] for the splice, write the files it names,
//! remember the inverse. Twelve tools go through this function, which is what
//! makes "every mutation returns the edit that takes it back" a property of the
//! server rather than something each tool has to remember.
//!
//! It also means a thirteenth operation in `slidx_edit` needs a schema and a
//! closure here, and nothing else.

use serde_json::{json, Value};

use slidx_edit::EditOp;

use crate::mcp::content::Answer;
use crate::mcp::edit;
use crate::mcp::history::Entry;
use crate::mcp::tool::args;
use crate::mcp::tool::Context;
use crate::mcp::workspace::Reading;

/// The arguments every mutating tool takes, and their descriptions.
///
/// Spliced into each tool's schema rather than restated, so a change to how a
/// deck is named reaches all twelve.
pub fn deck_arguments() -> Value {
    json!({
        "deck": {
            "type": "string",
            "description": "\
    The deck: a Markdown file, a directory of slide files, or the project directory \
    holding a `slides/` folder. Must be inside a directory this server was started \
    in or pointed at with --root.",
        },
        "separator": {
            "type": "string",
            "description": "Slide separator, when the deck uses something other than `---`.",
        },
    })
}

/// What every mutating tool answers with.
pub fn output() -> Value {
    json!({
        "type": "object",
        "properties": {
            "changed": {
                "type": "array",
                "items": { "type": "string" },
                "description": "\
    The files whose bytes changed. Empty when the deck already said this, which is \
    not a failure — slidx operations are idempotent, so asking for what is already \
    there plans no edit at all and leaves the modification time alone.",
            },
            "inverse": {
                "type": "array",
                "description": "\
    The edit that takes this change back: a list of byte ranges and their \
    replacements, measured against the deck as it now stands. Also on the undo \
    stack, so `undo` needs no argument.",
            },
            "undoDepth": {
                "type": "integer",
                "description": "How many changes this session can still take back.",
            },
            "slides": { "type": "integer", "description": "How many slides the deck now has." },
            "redundant": {
                "type": "boolean",
                "description": "True when the deck already said this and nothing was written.",
            },
        },
        "required": ["changed", "inverse", "undoDepth", "slides", "redundant"],
    })
}

/// Runs one operation against the deck the arguments name.
///
/// The operation is built from a [`Reading`] rather than from the arguments
/// alone, because a mark's byte range is only meaningful against the source it
/// was measured in.
pub fn operate(
    context: &mut Context<'_>,
    tool: &'static str,
    arguments: &Value,
    build: impl FnOnce(&Reading) -> Result<EditOp, String>,
) -> Result<Answer, String> {
    let path = args::required(
        arguments,
        "deck",
        "the deck to change — a Markdown file, a directory of slide files, or the project \
         holding one.",
    )?;

    let reading = context.workspace.edit_deck(&path, args::text(arguments, "separator"))?;
    let op = build(&reading)?;
    let applied = edit::apply(&reading, &op)?;

    context.history.record(Entry {
        tool,
        deck: reading.path.clone(),
        separator: reading.separator.clone(),
        // The layout *before* the change, because a removal deletes a file and
        // the undo has to know the place it left.
        files: reading.files.clone(),
        inverse: applied.inverse.clone(),
    });

    let after = context
        .workspace
        .read_deck(&reading.path.display().to_string(), Some(&reading.separator))?;

    Ok(Answer::text(report(tool, &applied, after.deck.slides.len(), context.history.len()))
        .with_data(json!({
            "changed": applied.changed,
            "inverse": applied.inverse,
            "undoDepth": context.history.len(),
            "slides": after.deck.slides.len(),
            "redundant": applied.redundant,
        })))
}

/// What a person reads in the client's transcript.
///
/// Names the files rather than the operation's own arguments, because the thing
/// worth checking is which of the author's files this touched.
fn report(tool: &str, applied: &edit::Applied, slides: usize, depth: usize) -> String {
    if applied.redundant {
        return format!(
            "`{tool}`: the deck already said this, so nothing was written. \
             {slides} slide{} unchanged.",
            plural(slides)
        );
    }

    format!(
        "`{tool}`: {}. The deck now has {slides} slide{}.\n\n\
         Every byte outside those ranges is untouched — this was a splice, not a rewrite. \
         {depth} change{} on this session's undo stack; `undo` takes back the last one.",
        changed(&applied.changed),
        plural(slides),
        plural(depth),
    )
}

fn changed(files: &[String]) -> String {
    match files {
        [] => "no file changed".to_string(),
        [one] => format!("spliced {one}"),
        many => format!("spliced {}", many.join(", ")),
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(changed: &[&str], redundant: bool) -> edit::Applied {
        edit::Applied {
            inverse: Default::default(),
            changed: changed.iter().map(|name| name.to_string()).collect(),
            source: String::new(),
            redundant,
        }
    }

    #[test]
    fn the_report_names_the_authors_files_rather_than_the_operation() {
        // The thing worth checking is which files this touched.
        let text = report("set_heading", &applied(&["0002.md"], false), 3, 1);

        assert!(text.contains("0002.md"), "{text}");
        assert!(text.contains("3 slides"), "{text}");
        assert!(text.contains("undo"), "{text}");
    }

    #[test]
    fn a_redundant_operation_says_so_rather_than_claiming_a_write() {
        // Idempotence is a property of slidx_edit, and a tool that reported a
        // write here would make a model believe a file's timestamp had moved.
        let text = report("set_heading", &applied(&[], true), 1, 0);

        assert!(text.contains("already said this"), "{text}");
        assert!(text.contains("1 slide unchanged"), "{text}");
    }

    #[test]
    fn the_report_counts_its_own_nouns() {
        assert!(report("undo", &applied(&["a.md"], false), 1, 1).contains("1 slide."));
        assert!(report("undo", &applied(&["a.md"], false), 2, 1).contains("2 slides."));
        assert!(report("undo", &applied(&["a.md"], false), 2, 1).contains("1 change on"));
        assert!(report("undo", &applied(&["a.md"], false), 2, 2).contains("2 changes on"));
    }

    #[test]
    fn every_mutating_tool_promises_the_same_four_things() {
        // The property that makes undo real rather than per-tool bookkeeping.
        let required = output()["required"].clone();

        assert_eq!(required, json!(["changed", "inverse", "undoDepth", "slides", "redundant"]));
    }
}
