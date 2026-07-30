//! Taking the last change back.
//!
//! This is the tool that only exists because slidx's edits are already
//! invertible values. It needs no arguments and holds no copy of the file: the
//! inverse was computed when the change was made, and applying it is the same
//! splice machinery in the other direction.
//!
//! No other editing surface an agent has can offer this. A tool that wrote whole
//! files would have to keep a copy of every file it touched, and would then be
//! restoring a snapshot rather than reversing a change — which loses anything
//! else that happened to the file in between.

use serde_json::{json, Value};

use super::{Context, Tool};
use crate::mcp::content::Answer;
use crate::mcp::edit;

pub const ALL: &[Tool] = &[Tool {
    name: "undo",
    title: "Take back the last change",
    description: "\
Reverses the most recent change this session made, exactly — byte for byte, not \
by rewriting the file from a copy. Call it repeatedly to walk back a whole run of \
edits; the answer says how many remain.

This works because a slidx edit is a value that knows its own inverse, computed \
when the change was made. It is the last change of THIS session only: nothing is \
remembered across restarts, and a file changed by anything else in the meantime is \
not reverted by it. Version control is still the real safety net.",
    schema: arguments,
    output,
    writes: true,
    run: undo,
}];

/// No arguments: what to reverse is a fact about the session, not the call.
fn arguments() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "description": "\
    Takes no arguments: the change to reverse is the last one this session made.",
    })
}

fn output() -> Value {
    json!({
        "type": "object",
        "properties": {
            "undone": {
                "type": "string",
                "description": "The tool whose change was taken back.",
            },
            "deck": { "type": "string", "description": "The deck it was taken back in." },
            "changed": {
                "type": "array",
                "items": { "type": "string" },
                "description": "The files whose bytes changed, going back.",
            },
            "undoDepth": {
                "type": "integer",
                "description": "How many changes this session can still take back.",
            },
        },
        "required": ["undone", "deck", "changed", "undoDepth"],
    })
}

fn undo(context: &mut Context<'_>, _arguments: &Value) -> Result<Answer, String> {
    let Some(entry) = context.history.take_last() else {
        return Err(
            "There is nothing to undo: this session has not changed anything yet. Undo does not \
             reach back past the server starting — for that, use version control."
                .to_string(),
        );
    };

    let deck = entry.deck.display().to_string();
    let mut reading = context.workspace.edit_deck(&deck, Some(&entry.separator))?;

    // The file list the change was measured against, not the one on disk now.
    // They differ exactly when the change deleted a file, which is the case an
    // undo has to get right.
    reading.files = entry.files.clone();

    let applied = edit::revert(&reading, &entry.inverse)?;

    // The inverse of the inverse is the change again, so a redo would be free.
    // It is not offered: an agent that wants the change back can make it again,
    // and a stack with two ends is two more states to explain than it is worth.
    Ok(Answer::text(format!(
        "Took back `{}` in {deck}: {}. {} change{} left on this session's stack.",
        entry.tool,
        changed(&applied.changed),
        context.history.len(),
        if context.history.len() == 1 { "" } else { "s" },
    ))
    .with_data(json!({
        "undone": entry.tool,
        "deck": deck,
        "changed": applied.changed,
        "undoDepth": context.history.len(),
    })))
}

fn changed(files: &[String]) -> String {
    match files {
        [] => "no file needed changing".to_string(),
        many => format!("restored {}", many.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::history::History;
    use crate::mcp::workspace::Workspace;

    #[test]
    fn undoing_nothing_says_it_does_not_reach_past_the_server_starting() {
        // A model that believed otherwise would ask for an undo instead of
        // reading the file, and get a confident wrong answer.
        let mut history = History::default();
        let workspace = Workspace::new(vec![std::env::temp_dir()]).writing();
        let mut held = Context { workspace: &workspace, history: &mut history };

        let refusal = undo(&mut held, &json!({})).expect_err("nothing to undo");

        assert!(refusal.contains("nothing to undo"), "{refusal}");
        assert!(refusal.contains("version control"), "{refusal}");
    }

    #[test]
    fn the_tool_takes_no_arguments_and_says_so() {
        // So a client does not invent one and a model does not look for a deck
        // to name.
        assert_eq!(arguments()["properties"], json!({}));
        assert!(arguments()["description"].as_str().expect("a text").contains("no arguments"));
    }

    #[test]
    fn undo_is_a_write_so_a_read_only_server_does_not_offer_it() {
        assert!(ALL[0].writes);
    }

    #[test]
    fn the_description_says_what_makes_this_possible_and_what_it_does_not_cover() {
        assert!(ALL[0].description.contains("knows its own inverse"));
        assert!(ALL[0].description.contains("THIS session only"));
    }
}
