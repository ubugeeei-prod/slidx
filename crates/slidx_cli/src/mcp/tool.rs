//! Which tools there are, spelled out once.
//!
//! The same shape [`crate::command::table`] takes, and for the same reason: a
//! list a client reads and a dispatch that runs it are two lists that drift, and
//! the failure is quiet — a tool that is advertised and unreachable, or one that
//! works and is invisible. So a tool is declared here and there is a test that
//! fails if the two halves disagree.
//!
//! ## No tool writes raw text to a deck file
//!
//! Not one, and that is the design rather than an omission. Every mutation is a
//! [`slidx_edit::EditOp`] that splices a byte range and hands back its own
//! inverse, so an agent working through this server is *structurally* unable to
//! reflow a paragraph or reorder frontmatter it did not name. A gesture the
//! operation set cannot express has no tool here — the answer is a new operation
//! in Rust with tests, which is the same rule the visual editor lives under.
//!
//! Every mutating tool is marked [`Tool::writes`] and is neither listed nor
//! runnable unless `slidx mcp --write` was asked for.
//!
//! ## Why the schema is written by hand
//!
//! A JSON Schema derived from a Rust type describes the type. What a tool needs
//! is a description of the *argument*, including which spellings are meaningful
//! to a speaker — a theme is one of four names, a rule code suppresses a whole
//! group when it names one, a step is a snapshot rather than a click handler. A
//! derived schema says `string` to all of that, and a model then guesses.
//!
//! What is *not* restated by hand is any closed set slidx already defines. The
//! effect presets in [`step`] are read from `EffectPreset::ALL`, so a preset
//! added to `slidx_core` reaches an agent without anybody remembering to.

pub mod apply;
pub mod args;
pub mod check;
pub mod find;
pub mod format;
pub mod mark;
pub mod slide;
pub mod step;
pub mod undo;

use serde_json::Value;

use super::content::Answer;
use super::history::History;
use super::protocol::has_structured_output;
use super::workspace::Workspace;

/// Everything a tool is allowed to touch.
///
/// Passed rather than owned by each tool so that the authority check happens in
/// one place: a tool cannot reach a filesystem except through
/// [`Workspace::edit_deck`], which refuses on a read-only server.
#[derive(Debug)]
pub struct Context<'a> {
    pub workspace: &'a Workspace,
    pub history: &'a mut History,
}

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
    /// True when it changes a file. Gated on `--write`, and not even listed
    /// without it — advertising a tool a client cannot call wastes a model's
    /// attention on a call it will be refused.
    pub writes: bool,
    pub run: fn(&mut Context<'_>, &Value) -> Result<Answer, String>,
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

/// The read-only tools, declared here; the mutating ones live beside the
/// operations they run.
const READING: &[Tool] = &[
    Tool {
        name: "lint_deck",
        title: "Lint a deck for the room it will be shown in",
        description: "\
Runs every slidx rule over a deck on disk and returns what a conference room \
will do to it: contrast through a model of projector washout, rendered font size \
by the angular size a glyph subtends from the back row, images blown up past \
their own pixels, heading order, bullet load, animation cost, and per-slide time \
budgets summed against the declared slot. An asset fetched from another origin \
is an error rather than advice, because a built deck asking nothing of anywhere \
but itself is the guarantee slidx makes out loud.

Reach for this before telling an author a slide is fine. None of what it checks \
is visible in the Markdown, and the rules that need a laid-out page — whether \
content actually fits — run in the build instead and are absent here rather \
than approximated.",
        schema: check::lint_schema,
        output: check::lint_output,
        writes: false,
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
        writes: false,
        run: check::machine,
    },
];

/// Every tool, whatever this server is allowed to do.
///
/// One list rather than two, so `find` cannot disagree with what was listed and
/// so the authority check lives in exactly one place — see [`listed`].
pub fn all() -> Vec<&'static Tool> {
    READING
        .iter()
        .chain(find::ALL)
        .chain(slide::ALL)
        .chain(mark::ALL)
        .chain(step::ALL)
        .chain(format::ALL)
        .chain(undo::ALL)
        .collect()
}

/// The tools a client is told about.
///
/// A read-only server does not list what it will refuse: advertising a tool a
/// client cannot call spends a model's attention on a call that is going to come
/// back as an error, and reads as a server that is broken rather than one that
/// was started carefully.
pub fn listed(writing: bool) -> Vec<&'static Tool> {
    all().into_iter().filter(|tool| writing || !tool.writes).collect()
}

/// One tool by name, listed or not.
///
/// Unfiltered on purpose: a mutating tool called on a read-only server has to be
/// answered with *why* rather than with "no such tool", which would send a model
/// looking for a different name for something that exists.
pub fn find(name: &str) -> Option<&'static Tool> {
    all().into_iter().find(|tool| tool.name == name)
}

#[cfg(test)]
mod tests {
    use super::super::protocol::PROTOCOL_VERSION;
    use super::*;

    #[test]
    fn every_tool_is_declared_exactly_once() {
        let mut names: Vec<&str> = all().iter().map(|tool| tool.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), total, "a tool name is declared twice");
    }

    #[test]
    fn every_tool_is_reachable_by_the_name_it_is_listed_under() {
        for tool in all() {
            assert!(find(tool.name).is_some(), "{} is listed and not dispatched", tool.name);
        }
    }

    #[test]
    fn every_tool_says_what_slidx_knows_that_a_model_does_not() {
        // A description that only restates the name is a description a model
        // will ignore, and then it does the job itself and gets it wrong.
        for tool in all() {
            assert!(tool.description.len() > 120, "{} says too little", tool.name);
            assert!(!tool.title.is_empty(), "{} has no title", tool.name);
        }
    }

    #[test]
    fn every_schema_is_an_object_with_documented_properties() {
        // A client validates against this before it ever reaches the tool, so
        // an argument that is not described here is one a model cannot send.
        for tool in all() {
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
        let tool = all()[0];

        assert!(tool.describe(PROTOCOL_VERSION)["outputSchema"].is_object());
        assert!(tool.describe("2024-11-05")["outputSchema"].is_null());
    }

    #[test]
    fn a_read_only_server_lists_nothing_that_writes() {
        assert!(listed(false).iter().all(|tool| !tool.writes));
        assert!(!listed(false).is_empty(), "and still has something to offer");
    }

    #[test]
    fn a_writing_server_lists_everything() {
        assert_eq!(listed(true).len(), all().len());
    }

    #[test]
    fn a_mutating_tool_is_still_findable_on_a_read_only_server() {
        // So calling it is answered with *why*, rather than with "no such tool"
        // — which would send a model looking for another name for something that
        // exists.
        assert!(find("set_heading").is_some_and(|tool| tool.writes));
    }

    #[test]
    fn every_operation_slidx_edit_defines_has_a_tool() {
        // The closed set is the whole point: an operation nothing can reach is a
        // gesture an agent will do by rewriting the file instead. This test is
        // what makes adding a variant in slidx_edit fail here until it is served.
        let served: Vec<&str> = all().iter().map(|tool| tool.name).collect();

        for expected in [
            "set_body",
            "set_heading",
            "insert_slide",
            "remove_slide",
            "move_slide",
            "set_field",
            "add_mark",
            "set_mark",
            "remove_mark",
            "add_step",
            "remove_step",
            "move_step",
            "set_step",
            "adopt_steps",
            "set_notes",
        ] {
            assert!(served.contains(&expected), "no tool runs EditOp `{expected}`");
        }
    }

    #[test]
    fn no_tool_takes_raw_file_content() {
        // The property that makes this server worth using: there is no call that
        // writes bytes at a path. `set_body` replaces one slide's Markdown
        // through an operation, which is a splice; nothing names a file.
        for tool in all() {
            let properties = (tool.schema)()["properties"].clone();

            for forbidden in ["path", "file", "contents", "source"] {
                assert!(
                    properties[forbidden].is_null(),
                    "{} takes `{forbidden}`, which is a file writer wearing a hat",
                    tool.name
                );
            }
        }
    }
}
