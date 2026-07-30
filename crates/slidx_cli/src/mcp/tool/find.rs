//! Searching every deck this machine has seen.
//!
//! The one tool here that is not about the deck in front of you. A speaker does
//! not give one talk: they keep five decks in five repositories, revisit one a
//! year later, and reuse a third — so the question "have I said this before, and
//! where" is a real one, and it is not answerable by looking at a directory.
//!
//! Wired to [`crate::grep::search`], which is what `slidx grep` calls. A hit
//! names the SLIDE it is on rather than a line number, because a line number in
//! a Markdown file is not where a speaker keeps their content and "slide 7 of
//! the VueConf deck" is.

use serde_json::{json, Value};

use crate::grep;
use crate::home::Home;
use crate::index::Index;
use crate::mcp::content::Answer;
use crate::mcp::tool::{args, Context, Tool};

/// Enough to answer a question, few enough to read.
///
/// A search that returned everything would be a search whose answer is another
/// search, and the query is cheap to narrow.
const DEFAULT_LIMIT: usize = 30;

pub const ALL: &[Tool] = &[Tool {
    name: "search_decks",
    title: "Search every deck this machine has seen",
    description: "\
Plain-text search across the deck sources of every project in the slidx index — \
not just the one in front of you. The index fills itself: running any slidx \
command on a deck is what puts it in the list.

A hit names the SLIDE it is on, not just the line. A line number in a Markdown \
file is not where a speaker keeps their content; \"slide 7 of the VueConf deck\" \
is, and it is what they can open.

There is no pattern syntax to learn and none to escape. A query in all lowercase \
matches either case; a query with a capital in it is matched exactly, so `Vue` \
finds the framework and not `revue`.

Reach for this when the question is \"have I explained this before\", \"which talk \
had the diagram about X\", or \"where else did I claim that number\" — before \
writing a slide from scratch that already exists in another deck.",
    schema: arguments,
    output,
    writes: false,
    run: search,
}];

fn arguments() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "\
    The text to look for, matched anywhere in a line. Lowercase matches either case; \
    a capital makes it exact.",
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "description":
                    format!("How many hits at most. Default {DEFAULT_LIMIT}."),
            },
        },
        "required": ["query"],
    })
}

fn output() -> Value {
    json!({
        "type": "object",
        "properties": {
            "hits": {
                "type": "array",
                "description": "Matches, most recently touched project first.",
                "items": {
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "The project directory, which is what a deck argument takes.",
                        },
                        "deck": { "type": "string" },
                        "slide": {
                            "type": "integer",
                            "description": "\
    One-based, the way a speaker counts slides — so subtract one before passing it to \
    an editing tool, which counts from zero.",
                        },
                        "slideTitle": { "type": "string" },
                        "slideId": { "type": "string" },
                        "line": { "type": "integer" },
                        "text": { "type": "string" },
                    },
                },
            },
            "truncated": {
                "type": "boolean",
                "description": "True when the limit was reached and there may be more.",
            },
        },
        "required": ["hits", "truncated"],
    })
}

fn search(context: &mut Context<'_>, arguments: &Value) -> Result<Answer, String> {
    let query = args::required(arguments, "query", "the text to look for.")?;
    let limit =
        args::number(arguments, "limit").filter(|limit| *limit > 0).unwrap_or(DEFAULT_LIMIT);

    let index = Index::load(&context.workspace.index_path());
    let hits = grep::search(&index, Home::discover().root(), &query, limit);

    // Not "0 results": a speaker whose index is empty and a query that genuinely
    // matches nothing are different problems, and only one of them is about the
    // query.
    if hits.is_empty() && index.is_empty() {
        return Ok(Answer::text(
            "This machine's deck index is empty, so there was nothing to search. It fills \
             itself — running any slidx command on a deck is what puts it in the list.",
        )
        .with_data(json!({ "hits": [], "truncated": false })));
    }

    let text = if hits.is_empty() {
        format!("No deck contains {query:?}.")
    } else {
        let mut text = format!("{} hit(s) for {query:?}:\n", hits.len());
        for hit in &hits {
            text.push_str(&format!(
                "\n  {} — slide {}{}\n    {}\n    {}\n",
                hit.deck,
                hit.slide,
                hit.slide_title.as_deref().map(|t| format!(" ({t})")).unwrap_or_default(),
                hit.text.trim(),
                hit.project.display(),
            ));
        }
        text
    };

    Ok(Answer::text(text).with_data(json!({ "hits": hits, "truncated": hits.len() >= limit })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::history::History;
    use crate::mcp::workspace::Workspace;
    use std::fs;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-find-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("slides")).expect("scratch");
            Self(path)
        }

        fn slide(&self, body: &str) {
            fs::write(self.0.join("slides/0001.md"), body).expect("write");
        }

        fn index(&self) -> PathBuf {
            self.0.join("index.json")
        }

        fn remembered(&self) -> &Self {
            crate::index::remember(&self.index(), crate::index::Entry::new(&self.0));
            self
        }

        fn ran(&self, arguments: Value) -> Result<Answer, String> {
            let workspace = Workspace::new(vec![self.0.clone()]).with_index(self.index());
            let mut history = History::default();

            search(&mut Context { workspace: &workspace, history: &mut history }, &arguments)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn data(answer: Result<Answer, String>) -> Value {
        crate::mcp::content::result(answer, crate::mcp::PROTOCOL_VERSION)["structuredContent"]
            .clone()
    }

    #[test]
    fn a_hit_names_the_slide_rather_than_only_the_line() {
        // A line number in a Markdown file is not where a speaker keeps their
        // content. "Slide 2 of the VueConf deck" is what they can open.
        let scratch = Scratch::new("slide");
        scratch.slide("# One\n\n---\n\n# Results\n\nLatency dropped to 38ms.\n");
        scratch.remembered();

        let found = data(scratch.ran(json!({ "query": "Latency" })));
        let hit = &found["hits"][0];

        assert_eq!(hit["slide"], 2, "one-based, the way a speaker counts");
        assert_eq!(hit["slideTitle"], "Results");
        assert!(hit["text"].as_str().expect("a line").contains("38ms"));
    }

    #[test]
    fn an_empty_index_is_a_different_answer_from_no_match() {
        // Only one of the two is about the query, and a model told "0 results"
        // would go on refining a search over nothing.
        let scratch = Scratch::new("empty");
        let answer = scratch.ran(json!({ "query": "anything" })).expect("an answer");
        let text = crate::mcp::content::result(Ok(answer), crate::mcp::PROTOCOL_VERSION);

        assert!(text["content"][0]["text"].as_str().expect("a text").contains("index is empty"));
    }

    #[test]
    fn a_query_that_matches_nothing_says_so_plainly() {
        let scratch = Scratch::new("nothing");
        scratch.slide("# One\n");
        scratch.remembered();

        let answer = scratch.ran(json!({ "query": "zzz-nowhere" })).expect("an answer");
        let text = crate::mcp::content::result(Ok(answer), crate::mcp::PROTOCOL_VERSION);

        assert!(text["content"][0]["text"].as_str().expect("a text").contains("No deck contains"));
    }

    #[test]
    fn the_limit_is_reported_so_a_model_knows_there_may_be_more() {
        let scratch = Scratch::new("limit");
        scratch.slide("# a\n\n---\n\n# a\n\n---\n\n# a\n");
        scratch.remembered();

        let found = data(scratch.ran(json!({ "query": "a", "limit": 1 })));

        assert_eq!(found["hits"].as_array().expect("hits").len(), 1);
        assert_eq!(found["truncated"], true);
    }

    #[test]
    fn a_missing_query_says_what_the_argument_is_for() {
        let scratch = Scratch::new("no-query");
        let refusal = scratch.ran(json!({})).expect_err("no query");

        assert!(refusal.contains("`query` is required"), "{refusal}");
    }

    #[test]
    fn searching_reads_and_never_writes() {
        assert!(!ALL[0].writes);
    }

    #[test]
    fn the_description_says_when_to_reach_for_it() {
        // Otherwise a model writes a slide from scratch that already exists in
        // another deck, which is the whole thing this is for.
        assert!(ALL[0].description.contains("have I explained this before"));
    }
}
