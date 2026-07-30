//! The two tools that only look: the linter, and the machine.
//!
//! Neither of these re-implements anything. `lint_deck` calls
//! [`crate::lint::findings`], which is the same function `slidx lint` calls, and
//! renders with [`crate::lint::render`], which is the same report a person
//! reads. `check_machine` asks [`slidx_doctor::probe`] for one environment and
//! hands it to the same checks.
//!
//! That is the whole design of this module. A second opinion about a deck is the
//! failure the workspace is arranged to prevent: a model told a deck was clean
//! by a linter the build does not run has been told nothing.
//!
//! ## Why the text is the same text
//!
//! It would have been easy to answer with JSON alone and let the model write its
//! own summary. But the diagnostic wording is where the *remedy* lives — every
//! slidx finding carries a concrete next action — and a model paraphrasing that
//! is a model inventing advice. So the plain-text report goes back verbatim, and
//! the structured half is there for a client that wants to count.

use serde_json::{json, Value};

use slidx_doctor::probe::{self, Request};
use slidx_lint::LintOptions;

use crate::mcp::content::Answer;
use crate::mcp::workspace::Workspace;
use crate::style::Style;

pub fn lint_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "deck": {
                "type": "string",
                "description": "\
    The deck: a Markdown file, a directory of slide files, or the project directory \
    holding a `slides/` folder. Must be inside a directory this server was given.",
            },
            "theme": {
                "type": "string",
                "description": "\
    Theme to resolve colours against, overriding the deck's own `theme:`. One of the \
    built-in names. Linting one theme's colours and shipping another's is a green \
    run about a deck nobody is going to show, so leave this alone unless the author \
    is deciding between themes.",
                "enum": ["minimal", "editorial", "terminal", "contrast"],
            },
            "allow": {
                "type": "array",
                "items": { "type": "string" },
                "description": "\
    Rule codes to suppress. A group name suppresses everything under it, so \
    `contrast` covers `contrast/too-low` and `contrast/projector` alike.",
            },
            "strict": {
                "type": "boolean",
                "description": "\
    Also report advisory findings that are correct but not always worth acting on.",
            },
            "separator": {
                "type": "string",
                "description": "Slide separator, when the deck uses something other than `---`.",
            },
        },
        "required": ["deck"],
    })
}

pub fn lint_output() -> Value {
    json!({
        "type": "object",
        "properties": {
            "deck": { "type": "string", "description": "The deck that was read." },
            "slides": { "type": "integer", "description": "How many slides it has." },
            "blocking": {
                "type": "integer",
                "description": "\
    Findings that fail a build: content that was dropped, or an asset fetched over \
    the network.",
            },
            "diagnostics": {
                "type": "array",
                "description": "Every finding, worst first then in deck order.",
                "items": {
                    "type": "object",
                    "properties": {
                        "code": { "type": "string" },
                        "severity": { "type": "string" },
                        "message": { "type": "string" },
                        "help": { "type": "string" },
                        "span": { "type": "object" },
                    },
                },
            },
        },
        "required": ["deck", "slides", "blocking", "diagnostics"],
    })
}

pub fn lint(workspace: &Workspace, arguments: &Value) -> Result<Answer, String> {
    let path = text(arguments, "deck").ok_or_else(|| {
        "`deck` is required: the path to a Markdown deck, a directory of slide files, or the \
         project holding one."
            .to_string()
    })?;

    let reading = workspace.read_deck(path, text(arguments, "separator"))?;

    let options = LintOptions {
        allow: strings(arguments, "allow"),
        strict: arguments.get("strict").and_then(Value::as_bool).unwrap_or_default(),
        ..LintOptions::default()
    };

    let found = crate::lint::findings(&reading.deck, text(arguments, "theme"), &options);
    let blocking = found.iter().filter(|finding| finding.is_blocking()).count();

    // The report a person reads, verbatim. Every finding's remedy is written
    // into it, and a model paraphrasing a remedy is a model inventing advice.
    let text = crate::lint::render(&reading.deck, &found, &reading.label, &Style::plain());

    Ok(Answer::text(text).with_data(json!({
        "deck": reading.path.display().to_string(),
        "slides": reading.deck.slides.len(),
        "blocking": blocking,
        "diagnostics": found,
    })))
}

pub fn machine_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "dir": {
                "type": "string",
                "description": "\
    Directory whose volume the disk check measures. Defaults to the first directory \
    this server was given.",
            },
            "offline": {
                "type": "boolean",
                "description": "\
    Take no network readings, and say so in the report. Use this when the machine is \
    deliberately offline; the network check reports unknown rather than failing.",
            },
            "explain": {
                "type": "boolean",
                "description": "Add what each check exists to catch.",
            },
        },
    })
}

pub fn machine_output() -> Value {
    json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "description": "The worst status in the report: pass, warn, fail, or unknown.",
            },
            "findings": {
                "type": "array",
                "description": "Every check, worst first. A check is always present.",
                "items": {
                    "type": "object",
                    "properties": {
                        "check": { "type": "string" },
                        "status": { "type": "string" },
                        "detail": { "type": "string" },
                        "remedy": { "type": "string" },
                    },
                },
            },
        },
        "required": ["status", "findings"],
    })
}

pub fn machine(workspace: &Workspace, arguments: &Value) -> Result<Answer, String> {
    let base = if arguments.get("offline").and_then(Value::as_bool).unwrap_or_default() {
        Request::offline()
    } else {
        Request::default()
    };

    // The disk check measures a volume, and the one worth measuring is the one
    // the deck is on. Defaulting to a root rather than to the process's working
    // directory, which is wherever the client happened to spawn this server.
    let workspace_dir = match text(arguments, "dir") {
        Some(path) => Some(workspace.readable(path)?),
        None => workspace.roots().first().cloned(),
    };

    let request = match workspace_dir {
        Some(dir) => base.in_workspace(dir),
        None => base,
    };

    let report = slidx_doctor::run(&probe::read(&request));
    let explain = arguments.get("explain").and_then(Value::as_bool).unwrap_or_default();

    Ok(Answer::text(crate::doctor::render(&report, &Style::plain(), explain)).with_data(json!({
        "status": report.status().as_token(),
        "findings": report.findings(),
    })))
}

fn text<'a>(arguments: &'a Value, key: &str) -> Option<&'a str> {
    arguments.get(key).and_then(Value::as_str).filter(|value| !value.is_empty())
}

fn strings(arguments: &Value, key: &str) -> Vec<String> {
    arguments
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-check-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("slides")).expect("scratch");
            Self(path)
        }

        fn slide(&self, body: &str) {
            fs::write(self.0.join("slides").join("0001.md"), body).expect("write");
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn workspace(scratch: &Scratch) -> Workspace {
        Workspace::new(vec![scratch.path().to_path_buf()])
            .with_index(scratch.path().join("no-index.json"))
    }

    fn lint_deck(scratch: &Scratch, extra: Value) -> Result<Answer, String> {
        let mut arguments = json!({ "deck": scratch.path().display().to_string() });
        for (key, value) in extra.as_object().cloned().unwrap_or_default() {
            arguments[key] = value;
        }

        lint(&workspace(scratch), &arguments)
    }

    /// The structured half of whatever a tool answered.
    fn data(answer: Result<Answer, String>) -> Value {
        crate::mcp::content::result(answer, crate::mcp::PROTOCOL_VERSION)["structuredContent"]
            .clone()
    }

    #[test]
    fn a_deck_that_reaches_the_network_for_an_asset_is_reported_as_blocking() {
        // The one guarantee slidx makes out loud, so the one rule that blocks.
        let scratch = Scratch::new("remote");
        scratch.slide("# One\n\n![a diagram](https://cdn.example.com/a.png)\n");

        let structured = data(lint_deck(&scratch, json!({})));

        assert_eq!(structured["blocking"], 1);
        assert_eq!(structured["diagnostics"][0]["code"], "offline/remote-asset");
    }

    #[test]
    fn the_report_a_model_reads_is_the_report_a_person_reads() {
        // The remedy lives in the wording. A model paraphrasing it is a model
        // inventing advice.
        let scratch = Scratch::new("wording");
        scratch.slide("# One\n\n![](./a.png)\n");

        let answer = lint_deck(&scratch, json!({})).expect("a report");
        let rendered = crate::mcp::content::result(Ok(answer), crate::mcp::PROTOCOL_VERSION);
        let text = rendered["content"][0]["text"].as_str().expect("text").to_string();

        assert!(text.contains("slidx lint"), "{text}");
        assert!(text.contains("[structure/missing-alt]"), "{text}");
        assert!(!text.contains('\u{1b}'), "no terminal escapes reach a client: {text}");
    }

    #[test]
    fn a_clean_deck_is_told_so_in_a_sentence_rather_than_by_an_empty_answer() {
        let scratch = Scratch::new("clean");
        scratch.slide("# One\n\n- a\n- b\n");

        let structured = data(lint_deck(&scratch, json!({})));
        assert_eq!(structured["diagnostics"], json!([]));
        assert_eq!(structured["slides"], 1);
    }

    #[test]
    fn allow_suppresses_a_rule_and_a_whole_group_alike() {
        let scratch = Scratch::new("allow");
        scratch.slide("# One\n\n![a](https://cdn.example.com/a.png)\n");

        assert_eq!(data(lint_deck(&scratch, json!({ "allow": ["offline"] })))["blocking"], 0);
        assert_eq!(
            data(lint_deck(&scratch, json!({ "allow": ["offline/remote-asset"] })))["blocking"],
            0
        );
    }

    #[test]
    fn the_theme_argument_decides_which_colours_are_checked() {
        // Linting one theme's colours and shipping another's is a green run
        // about a deck nobody is going to show.
        let scratch = Scratch::new("theme");
        scratch.slide("# One\n");

        assert!(lint_deck(&scratch, json!({ "theme": "contrast" })).is_ok());
    }

    #[test]
    fn a_missing_deck_argument_says_what_the_argument_is_for() {
        let scratch = Scratch::new("missing");
        let refusal = lint(&workspace(&scratch), &json!({})).expect_err("no deck");

        assert!(refusal.contains("`deck` is required"), "{refusal}");
    }

    #[test]
    fn the_machine_check_reports_every_check_even_where_nothing_could_be_read() {
        // An unavailable reading is unknown, never a pass. This runs on a
        // continuous integration machine with no battery and no window server.
        let scratch = Scratch::new("machine");
        let structured = data(machine(&workspace(&scratch), &json!({ "offline": true })));

        let checks = structured["findings"].as_array().expect("findings");
        assert!(!checks.is_empty());
        for finding in checks {
            assert!(finding["check"].as_str().is_some_and(|check| !check.is_empty()));
            assert!(
                finding["status"] == "pass"
                    || finding["remedy"].as_str().is_some_and(|remedy| !remedy.is_empty()),
                "nothing but a pass may be left without a next action: {finding}"
            );
        }
    }

    #[test]
    fn the_disk_check_measures_a_root_rather_than_wherever_the_client_spawned_us() {
        // The working directory of an MCP server is whatever the client chose.
        // The volume worth measuring is the one the deck is on.
        let scratch = Scratch::new("disk");
        let answer = machine(&workspace(&scratch), &json!({ "offline": true })).expect("a report");
        let text = crate::mcp::content::result(Ok(answer), crate::mcp::PROTOCOL_VERSION);

        assert!(text["content"][0]["text"].as_str().expect("a report").contains("slidx doctor"));
    }

    #[test]
    fn a_directory_outside_the_roots_cannot_be_measured_either() {
        let scratch = Scratch::new("disk-outside");
        let above = std::env::temp_dir().display().to_string();
        let refusal = machine(&workspace(&scratch), &json!({ "dir": above })).expect_err("outside");

        assert!(refusal.contains("outside"), "{refusal}");
    }
}
