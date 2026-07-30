//! What a tool call answers with, shaped for the revision that was agreed.
//!
//! Two distinctions live here and both are easy to get wrong.
//!
//! **A tool that failed is not a protocol error.** An unknown tool is the
//! client's mistake and belongs in a JSON-RPC `error`. A tool that ran and could
//! not do the job — a deck that is not there, a slide that has been deleted — is
//! a *successful* call whose result says so, because the model is the one that
//! has to read it and try something else. An error frame would be reported to
//! the user as a broken server instead.
//!
//! **Structured output belongs to one revision.** `structuredContent` arrived in
//! 2025-06-18. A client that negotiated 2024-11-05 has no way to read it, so the
//! same answer is always in a text block as well — which is also what makes the
//! text the thing worth writing well rather than a label on the JSON.

use serde_json::{json, Value};

use super::protocol::has_structured_output;

/// What a tool produced.
#[derive(Debug, Clone)]
pub struct Answer {
    text: String,
    data: Option<Value>,
}

impl Answer {
    /// What a model reads. Written for a reader, not as a label on the JSON.
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), data: None }
    }

    /// The same answer as data, for a client that can take it.
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// The `tools/call` result for whatever a tool did.
pub fn result(answer: Result<Answer, String>, version: &str) -> Value {
    let answer = match answer {
        Ok(answer) => answer,
        // The reason goes in the content rather than an error frame: the model
        // is the one that has to act on it.
        Err(reason) => {
            return json!({
                "content": [text_block(&reason)],
                "isError": true,
            })
        }
    };

    let mut result = json!({
        "content": [text_block(&answer.text)],
        "isError": false,
    });

    if let (Some(data), true) = (answer.data, has_structured_output(version)) {
        result["structuredContent"] = data;
    }

    result
}

fn text_block(text: &str) -> Value {
    json!({ "type": "text", "text": text })
}

#[cfg(test)]
mod tests {
    use super::super::protocol::PROTOCOL_VERSION;
    use super::*;

    #[test]
    fn an_answer_always_arrives_as_text_whichever_revision_was_agreed() {
        // The text is the part every client can read, so it is the part that
        // has to carry the answer.
        for version in [PROTOCOL_VERSION, "2024-11-05"] {
            let result = result(Ok(Answer::text("2 slides, all clear.")), version);

            assert_eq!(result["content"][0]["type"], "text");
            assert_eq!(result["content"][0]["text"], "2 slides, all clear.");
            assert_eq!(result["isError"], false);
        }
    }

    #[test]
    fn structured_output_is_withheld_from_a_client_that_cannot_read_it() {
        let answer = || Ok(Answer::text("x").with_data(json!({ "slides": 2 })));

        assert_eq!(result(answer(), PROTOCOL_VERSION)["structuredContent"], json!({ "slides": 2 }));
        assert!(result(answer(), "2024-11-05")["structuredContent"].is_null());
    }

    #[test]
    fn a_tool_that_could_not_do_the_job_says_so_in_a_successful_call() {
        // An error frame would be reported to the user as a broken server. The
        // model is the one that has to read this and try something else.
        let refused = result(Err("There is nothing at ./slides.".into()), PROTOCOL_VERSION);

        assert_eq!(refused["isError"], true);
        assert_eq!(refused["content"][0]["text"], "There is nothing at ./slides.");
        assert!(refused["structuredContent"].is_null(), "there is no answer to structure");
    }
}
