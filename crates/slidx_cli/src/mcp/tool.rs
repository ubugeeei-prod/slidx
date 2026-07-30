//! Which tools there are, spelled out once.
//!
//! The same shape [`crate::command::table`] takes, and for the same reason: a
//! list a client reads and a dispatch that runs it are two lists that drift, and
//! the failure is quiet — a tool that is advertised and unreachable, or one that
//! works and is invisible. So a tool is declared here and there is a test that
//! fails if the two halves disagree.
//!
//! ## What a tool is allowed to be
//!
//! Read-only, in this crate's current form. Nothing under [`self`] opens a file
//! for writing.
//!
//! ## Why the schema is written by hand
//!
//! A JSON Schema derived from a Rust type describes the type. What a tool needs
//! is a description of the *argument*, including which spellings are meaningful
//! to a speaker — a theme is one of four names, a rule code suppresses a whole
//! group when it names one. A derived schema says `string` to all of that, and a
//! model then guesses.

pub mod check;

use serde_json::Value;

use super::content::Answer;
use super::protocol::has_structured_output;
use super::workspace::Workspace;

/// One tool, as a client lists it and as the server runs it.
#[derive(Debug, Clone, Copy)]
pub struct Tool {
    pub name: &'static str,
    /// Shown to a person choosing whether to allow the call.
    pub title: &'static str,
    /// What the tool does and when to reach for it. Read by a model deciding
    /// between this and doing the job itself, so it says what slidx knows that
    /// the model does not.
    pub description: &'static str,
    /// The arguments, as JSON Schema.
    pub schema: fn() -> Value,
    /// The shape of the structured half of the answer, for a client that
    /// negotiated a revision with one.
    pub output: fn() -> Value,
    pub run: fn(&Workspace, &Value) -> Result<Answer, String>,
}

impl Tool {
    /// The descriptor a client lists.
    pub fn describe(&self, version: &str) -> Value {
        let mut described = serde_json::json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": (self.schema)(),
        });

        // `outputSchema` arrived with `structuredContent`, and promising one to
        // a client that will never be sent the other is a promise to nobody.
        if has_structured_output(version) {
            described["outputSchema"] = (self.output)();
        }

        described
    }
}

pub const ALL: &[Tool] = &[
    Tool {
        name: "lint_deck",
        title: "Lint a deck for the room it will be shown in",
        description: "\
Runs every slidx rule over a deck on disk and returns what a conference room \
will do to it: contrast through a model of projector washout, rendered font size \
by the angular size a glyph subtends from the back row, images blown up past \
their own pixels, heading order, bullet load, animation cost, and per-slide time \
budgets summed against the declared slot. An asset fetched over the network is \
an error rather than advice, because a built deck making zero network requests \
is the guarantee slidx makes out loud.

Reach for this before telling an author a slide is fine. None of what it checks \
is visible in the Markdown, and the rules that need a laid-out page — whether \
content actually fits — run in the build instead and are absent here rather \
than approximated.",
        schema: check::lint_schema,
        output: check::lint_output,
        run: check::lint,
    },
    Tool {
        name: "check_machine",
        title: "Check the machine a talk is about to be given from",
        description: "\
`slidx doctor`: power, disk, clock skew against a reference, the fonts a theme \
names, whether anything running could grab the screen, and whether the network \
works. Worst first, each with what to do about it.

Every reading that could not be taken is reported as unknown, never as a pass — \
a green light for something nobody measured is the one failure that would make \
the report worse than useless. Nothing here reads the deck.",
        schema: check::machine_schema,
        output: check::machine_output,
        run: check::machine,
    },
];

/// One tool by name.
pub fn find(name: &str) -> Option<&'static Tool> {
    ALL.iter().find(|tool| tool.name == name)
}

#[cfg(test)]
mod tests {
    use super::super::protocol::PROTOCOL_VERSION;
    use super::*;

    #[test]
    fn every_tool_is_declared_exactly_once() {
        let mut names: Vec<&str> = ALL.iter().map(|tool| tool.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), total, "a tool name is declared twice");
    }

    #[test]
    fn every_tool_is_reachable_by_the_name_it_is_listed_under() {
        for tool in ALL {
            assert!(find(tool.name).is_some(), "{} is listed and not dispatched", tool.name);
        }
    }

    #[test]
    fn every_tool_says_what_slidx_knows_that_a_model_does_not() {
        // A description that only restates the name is a description a model
        // will ignore, and then it does the job itself and gets it wrong.
        for tool in ALL {
            assert!(tool.description.len() > 120, "{} says too little", tool.name);
            assert!(!tool.title.is_empty(), "{} has no title", tool.name);
        }
    }

    #[test]
    fn every_schema_is_an_object_with_documented_properties() {
        // A client validates against this before it ever reaches the tool, so
        // an argument that is not described here is one a model cannot send.
        for tool in ALL {
            let schema = (tool.schema)();

            assert_eq!(schema["type"], "object", "{}", tool.name);
            let properties = schema["properties"].as_object().expect(tool.name);
            for (argument, described) in properties {
                assert!(
                    described["description"].as_str().is_some_and(|text| !text.is_empty()),
                    "{}.{argument} is undescribed",
                    tool.name
                );
            }
        }
    }

    #[test]
    fn a_descriptor_carries_an_output_schema_only_for_a_revision_that_has_one() {
        let tool = &ALL[0];

        assert!(tool.describe(PROTOCOL_VERSION)["outputSchema"].is_object());
        assert!(tool.describe("2024-11-05")["outputSchema"].is_null());
    }
}
