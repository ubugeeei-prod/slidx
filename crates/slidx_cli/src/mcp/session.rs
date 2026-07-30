//! Dispatch, and the lifecycle around it.
//!
//! The server is a pure function of the messages it has seen: [`handle`] takes
//! one frame and returns the frames to send back. Nothing here touches a stream,
//! a thread or a clock, which is what lets a whole session be a recorded
//! exchange in a test.
//!
//! ## The distinction this file exists to get right
//!
//! A notification has no id and **must not** be answered. It is the easiest
//! violation to commit and the hardest to notice: the extra frame goes out on a
//! stream nobody asserted on, the client cannot match it to a request, and most
//! clients log it and carry on. So [`handle`] decides request-or-notification
//! once, at the top, and the notification path returns no frames at all rather
//! than an empty result that something downstream could turn into one.
//!
//! ## There is no shutdown request
//!
//! MCP ends a session by closing the input stream. A server waiting for a
//! `shutdown` it will never be sent would hang on every disconnect, so the
//! lifecycle here has two states rather than four: before `initialize`, and
//! after it.
//!
//! [`handle`]: Session::handle

use serde_json::{json, Value};

use slidx_jsonrpc::{error_code, Message, RequestId};

use super::content;
use super::history::History;
use super::instructions;
use super::protocol::{negotiate, Negotiation, SUPPORTED};
use super::tool::{self, Context};
use super::workspace::Workspace;
use super::SERVER_NAME;

/// One connection.
#[derive(Debug)]
pub struct Session {
    workspace: Workspace,
    /// The revision agreed on `initialize`, and `None` until then.
    ///
    /// Holds the negotiation result rather than a boolean because it decides the
    /// shape of every later reply, not just whether there was one.
    version: Option<&'static str>,
    /// What this session has changed, so `undo` can take it back.
    history: History,
}

impl Session {
    pub fn new(workspace: Workspace) -> Self {
        Self { workspace, version: None, history: History::default() }
    }

    /// How many changes this session could still take back.
    pub fn undo_depth(&self) -> usize {
        self.history.len()
    }

    /// The revision this session agreed on, once it has.
    pub fn version(&self) -> Option<&'static str> {
        self.version
    }

    /// Handles one frame and returns whatever should go back.
    pub fn handle(&mut self, message: Message) -> Vec<Message> {
        if message.is_notification() {
            // Nothing to answer, ever. `notifications/initialized` and
            // `notifications/cancelled` are the two a client actually sends,
            // and neither changes anything this server does.
            return Vec::new();
        }

        let (Some(id), Some(method)) = (message.id.clone(), message.method.clone()) else {
            // A response to a request this server never sent, or a frame with
            // neither id nor method. There is nobody to reply to.
            return Vec::new();
        };

        vec![self.request(id, &method, message.params())]
    }

    /// A batch, refused.
    ///
    /// Its own method because a batch has no single id to answer under, and
    /// because the refusal has to say what was wrong: the client's JSON was
    /// fine, so a parse error would send somebody looking for a bug in their
    /// serialiser.
    pub fn refuse_batch() -> Message {
        Message::error(
            RequestId::Null,
            error_code::INVALID_REQUEST,
            format!(
                "This server does not accept a JSON-RPC batch. Send one request per line. \
                 It speaks {}, neither of which requires batching.",
                SUPPORTED.join(" and ")
            ),
        )
    }

    fn request(&mut self, id: RequestId, method: &str, params: &Value) -> Message {
        if method == "initialize" {
            return self.initialize(id, params);
        }

        // A client checks a server is alive with `ping`, and it is allowed to do
        // that before anything is negotiated. Everything else needs a revision
        // agreed, because the shape of the answer depends on it.
        if method == "ping" {
            return Message::response(id, json!({}));
        }

        let Some(version) = self.version else {
            return Message::error(
                id,
                error_code::INVALID_REQUEST,
                format!(
                    "`{method}` arrived before `initialize`. Negotiate a protocol version first."
                ),
            );
        };

        match method {
            "tools/list" => {
                let writing = self.workspace.authority().writes();
                let listed: Vec<Value> =
                    tool::listed(writing).iter().map(|tool| tool.describe(version)).collect();

                Message::response(id, json!({ "tools": listed }))
            }
            "tools/call" => self.call(id, params, version),
            _ => Message::error(
                id,
                error_code::METHOD_NOT_FOUND,
                format!("`{method}` is not something this server serves."),
            ),
        }
    }

    fn initialize(&mut self, id: RequestId, params: &Value) -> Message {
        if self.version.is_some() {
            // Renegotiating mid-session would change the shape of every reply
            // after it, including the ones already in flight.
            return Message::error(
                id,
                error_code::INVALID_REQUEST,
                "This session is already initialised.",
            );
        }

        let requested = params.get("protocolVersion").and_then(Value::as_str);

        let agreed = match negotiate(requested) {
            Negotiation::Agreed(version) => version,
            Negotiation::Unsupported(asked) => {
                let asked = if asked.is_empty() { "nothing".to_string() } else { asked };

                return Message::error_with_data(
                    id,
                    error_code::INVALID_PARAMS,
                    format!(
                        "Unsupported protocol version: this server speaks {}, and was asked for {asked}.",
                        SUPPORTED.join(" and ")
                    ),
                    json!({ "supported": SUPPORTED, "requested": asked }),
                );
            }
        };

        self.version = Some(agreed);

        Message::response(
            id,
            json!({
                "protocolVersion": agreed,
                // No `listChanged` under tools: the set is compiled in, so a
                // client that subscribed would be waiting for a notification
                // that cannot happen.
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "title": "slidx",
                    "version": crate::version(),
                },
                "instructions": instructions::for_authority(self.workspace.authority()),
            }),
        )
    }

    /// Runs one tool.
    ///
    /// An unknown tool is a protocol error and a tool that failed is not: the
    /// first is the client's mistake, and the second is an answer the model has
    /// to read and act on. See [`super::content`].
    fn call(&mut self, id: RequestId, params: &Value, version: &str) -> Message {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Message::error(
                id,
                error_code::INVALID_PARAMS,
                "`tools/call` needs the `name` of a tool.",
            );
        };

        let Some(tool) = tool::find(name) else {
            return Message::error(
                id,
                error_code::INVALID_PARAMS,
                format!(
                    "There is no tool called `{name}`. Ask `tools/list` for the ones there are."
                ),
            );
        };

        let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

        // Refused here rather than deeper down, so the reason is about authority
        // rather than about whichever argument happened to be checked first.
        let answered = if tool.writes && !self.workspace.authority().writes() {
            Err(format!(
                "`{name}` changes a deck, and this server was started read-only. Nothing was \
                 written. Whoever runs it can allow writes with `slidx mcp --write`; that is \
                 their decision and not one a deck or a tool call can make for them.",
            ))
        } else {
            let mut context = Context { workspace: &self.workspace, history: &mut self.history };
            (tool.run)(&mut context, &arguments)
        };

        Message::response(id, content::result(answered, version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::PROTOCOL_VERSION;

    fn session() -> Session {
        Session::new(
            Workspace::new(vec![std::env::temp_dir()])
                .with_index(std::env::temp_dir().join("slidx-mcp-no-index.json")),
        )
    }

    fn request(id: i64, method: &str, params: Value) -> Message {
        Message {
            id: Some(RequestId::Number(id)),
            method: Some(method.to_string()),
            params: Some(params),
            ..Message::default()
        }
    }

    fn started() -> Session {
        let mut session = session();
        session.handle(request(1, "initialize", json!({ "protocolVersion": PROTOCOL_VERSION })));
        session
    }

    #[test]
    fn a_notification_produces_no_frames_at_all() {
        // Not an empty response — no frame. A response to a notification is a
        // violation the client cannot match to anything.
        let mut session = started();

        assert!(session
            .handle(Message::notification("notifications/initialized", json!({})))
            .is_empty());
        assert!(session
            .handle(Message::notification("notifications/cancelled", json!({ "requestId": 1 })))
            .is_empty());
    }

    #[test]
    fn a_response_arriving_at_this_server_is_not_answered() {
        // This server sends no requests, so a response is either a confused
        // client or a crossed pipe. Answering it would make a loop.
        let mut session = started();
        let reply = Message {
            id: Some(RequestId::Number(4)),
            result: Some(json!({})),
            ..Message::default()
        };

        assert!(session.handle(reply).is_empty());
    }

    #[test]
    fn the_agreed_revision_is_remembered_because_it_shapes_every_later_reply() {
        let mut session = session();
        assert_eq!(session.version(), None);

        session.handle(request(1, "initialize", json!({ "protocolVersion": "2024-11-05" })));
        assert_eq!(session.version(), Some("2024-11-05"));
    }

    #[test]
    fn a_revision_nobody_here_speaks_leaves_the_session_uninitialised() {
        // So the client can retry with one from the list rather than having to
        // start a new process.
        let mut session = session();
        session.handle(request(1, "initialize", json!({ "protocolVersion": "1.0.0" })));

        assert_eq!(session.version(), None);

        let replies = session.handle(request(
            2,
            "initialize",
            json!({ "protocolVersion": PROTOCOL_VERSION }),
        ));
        assert!(replies[0].result.is_some(), "and the retry works");
    }

    #[test]
    fn a_read_only_session_lists_only_the_tools_it_will_run() {
        // A tool advertised and refused spends a model's attention on a call
        // that comes back as an error, and reads as a broken server.
        let mut session = started();
        let replies = session.handle(request(2, "tools/list", Value::Null));
        let listed = replies[0].result.clone().expect("a result");

        assert_eq!(listed["tools"].as_array().expect("tools").len(), tool::listed(false).len());
        assert!(listed["tools"].as_array().expect("tools").len() < tool::all().len());
    }

    #[test]
    fn a_writing_session_lists_the_operations_too() {
        let workspace = Workspace::new(vec![std::env::temp_dir()])
            .with_index(std::env::temp_dir().join("slidx-mcp-no-index.json"))
            .writing();
        let mut session = Session::new(workspace);
        session.handle(request(1, "initialize", json!({ "protocolVersion": PROTOCOL_VERSION })));

        let replies = session.handle(request(2, "tools/list", Value::Null));
        let listed = replies[0].result.clone().expect("a result");

        assert_eq!(listed["tools"].as_array().expect("tools").len(), tool::all().len());
    }

    #[test]
    fn a_mutating_tool_on_a_read_only_session_is_refused_by_authority_not_by_argument() {
        // The reason has to be about what the server was allowed to do. A
        // complaint about a missing argument would send a model round again
        // with a better-formed call that is refused for the same reason.
        let mut session = started();
        let replies = session.handle(request(
            3,
            "tools/call",
            json!({ "name": "set_heading", "arguments": {} }),
        ));
        let result = replies[0].result.clone().expect("a result");

        assert_eq!(result["isError"], true);
        let reason = result["content"][0]["text"].as_str().expect("a reason");
        assert!(reason.contains("read-only"), "{reason}");
        assert!(reason.contains("--write"), "{reason}");
    }

    #[test]
    fn nothing_is_on_the_undo_stack_until_something_changes() {
        assert_eq!(started().undo_depth(), 0);
    }

    #[test]
    fn a_call_with_no_tool_name_is_a_protocol_error_rather_than_a_failing_tool() {
        // There was no tool to fail. The client sent a malformed request.
        let mut session = started();
        let replies = session.handle(request(3, "tools/call", json!({ "arguments": {} })));

        assert_eq!(replies[0].error.as_ref().expect("an error").code, error_code::INVALID_PARAMS);
    }

    #[test]
    fn a_call_with_no_arguments_at_all_reaches_the_tool() {
        // A tool with only optional arguments has to be callable as
        // `{"name": "..."}`, which is what a client sends when it has nothing
        // to pass.
        let mut session = started();
        let replies = session.handle(request(3, "tools/call", json!({ "name": "check_machine" })));

        assert!(replies[0].error.is_none(), "{:?}", replies[0].error);
    }

    #[test]
    fn a_batch_is_refused_under_a_null_id_because_it_has_no_single_one() {
        let refusal = Session::refuse_batch();

        assert_eq!(refusal.id, Some(RequestId::Null));
        assert!(refusal.error.expect("an error").message.contains("batch"));
    }

    #[test]
    fn the_capabilities_declare_no_notification_this_server_cannot_send() {
        // A client that subscribed to `listChanged` would wait for a
        // notification that cannot happen: the tool set is compiled in.
        let mut session = session();
        let replies = session.handle(request(
            1,
            "initialize",
            json!({ "protocolVersion": PROTOCOL_VERSION }),
        ));
        let capabilities = replies[0].result.clone().expect("a result")["capabilities"].clone();

        assert_eq!(capabilities["tools"], json!({}));
    }
}
