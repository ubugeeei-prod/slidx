//! Request dispatch and the session lifecycle.
//!
//! The server is a pure function of the messages it has seen: [`handle`] takes
//! one frame and returns the frames to send back, and [`flush`] returns the
//! diagnostics owed for edits made since the last one. Nothing here touches a
//! stream, a thread, or a clock, which is what makes a whole editing session
//! something a test can state.
//!
//! # Why publishing is deferred
//!
//! An author typing produces a `didChange` per keystroke. Publishing
//! diagnostics from inside the handler would parse the deck once per
//! character and send an editor a diagnostic set it is going to replace a
//! moment later. So a change only marks the document dirty, and the stdio loop
//! calls [`flush`] when the input queue is empty — once per burst of typing
//! rather than once per character, with no timer to tune and no work started
//! that is already known to be stale.
//!
//! [`handle`]: Server::handle
//! [`flush`]: Server::flush

use serde::Deserialize;
use serde_json::{json, Value};

use crate::completion::complete;
use crate::deck;
use crate::diagnostics::publish;
use crate::document::{ContentChange, DocumentStore};
use crate::formatting::format;
use crate::hover::hover;
use crate::position::{Position, PositionEncoding};
use crate::protocol::{error_code, Message, RequestId};
use crate::symbols::outline;
use crate::SERVER_NAME;

/// How far through the lifecycle the session is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Starting,
    Running,
    ShuttingDown,
    Exited,
}

/// One editing session.
#[derive(Debug, Default)]
pub struct Server {
    store: DocumentStore,
    encoding: PositionEncoding,
    /// Documents whose diagnostics are owed, in the order they were touched.
    dirty: Vec<String>,
    state: State,
}

#[derive(Debug, Deserialize)]
struct TextDocumentIdentifier {
    uri: String,
    #[serde(default)]
    version: i64,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentParams {
    text_document: TextDocumentIdentifier,
    #[serde(default)]
    content_changes: Vec<ContentChange>,
    #[serde(default)]
    position: Position,
}

impl Server {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> State {
        self.state
    }

    /// The encoding negotiated with the client. UTF-16 until one says
    /// otherwise, because that is what the protocol assumes.
    pub fn encoding(&self) -> PositionEncoding {
        self.encoding
    }

    /// Handles one frame and returns whatever should go back.
    pub fn handle(&mut self, message: Message) -> Vec<Message> {
        let Some(method) = message.method.clone() else {
            return Vec::new();
        };

        match message.id.clone() {
            Some(id) => vec![self.request(id, &method, message.params())],
            None => self.notification(&method, message.params()),
        }
    }

    /// Diagnostics owed for every document edited since the last call.
    pub fn flush(&mut self) -> Vec<Message> {
        let dirty = std::mem::take(&mut self.dirty);

        dirty.iter().filter_map(|uri| self.diagnostics_for(uri)).collect()
    }

    /// True once the client has said `exit`.
    pub fn should_exit(&self) -> bool {
        self.state == State::Exited
    }

    fn request(&mut self, id: RequestId, method: &str, params: &Value) -> Message {
        if self.state == State::ShuttingDown {
            return Message::error(id, error_code::INVALID_REQUEST, "server is shutting down");
        }

        match method {
            "initialize" => {
                self.encoding = negotiate(params);
                self.state = State::Running;
                Message::response(id, self.capabilities())
            }
            "shutdown" => {
                self.state = State::ShuttingDown;
                Message::response(id, Value::Null)
            }
            "textDocument/completion" => self.feature(id, params, |server, params, uri| {
                let encoding = server.encoding;
                let document = server.store.get_mut(uri)?;
                let analysis = document.analysis();

                Some(json!(complete(
                    &analysis,
                    document.text(),
                    document.index(),
                    params.position,
                    encoding
                )))
            }),
            "textDocument/hover" => self.feature(id, params, |server, params, uri| {
                let encoding = server.encoding;
                let document = server.store.get_mut(uri)?;
                let analysis = document.analysis();

                Some(
                    hover(&analysis, document.text(), document.index(), params.position, encoding)
                        .map_or(Value::Null, |hovered| json!(hovered)),
                )
            }),
            // Reads no analysis: the formatter works from the source, and a
            // document with a half-typed fence in it is exactly the one an
            // author is most likely to save.
            "textDocument/formatting" => self.feature(id, params, |server, _, uri| {
                let encoding = server.encoding;
                let document = server.store.get(uri)?;

                Some(json!(format(document.text(), document.index(), encoding)))
            }),
            "textDocument/documentSymbol" => self.feature(id, params, |server, _, uri| {
                let encoding = server.encoding;
                let document = server.store.get_mut(uri)?;
                // The outline may come from an earlier analysis while a fence
                // is half typed, so it is built against the current index.
                let analysis = document.outline_analysis();

                Some(json!(outline(&analysis, document.text(), document.index(), encoding)))
            }),
            _ => {
                Message::error(id, error_code::METHOD_NOT_FOUND, format!("unknown method {method}"))
            }
        }
    }

    fn notification(&mut self, method: &str, params: &Value) -> Vec<Message> {
        let Ok(params) = serde_json::from_value::<DocumentParams>(params.clone()) else {
            if method == "exit" {
                self.state = State::Exited;
            }
            return Vec::new();
        };

        let uri = params.text_document.uri;

        match method {
            // A file that is not a deck is never opened, so every later
            // request for it answers with nothing and no diagnostic is ever
            // published against it. That is the whole enforcement: a client
            // that filters is saving traffic rather than deciding anything.
            "textDocument/didOpen" if !deck::is_deck(&uri) => {}
            "textDocument/didOpen" => {
                self.store.open(&uri, params.text_document.version, params.text_document.text);
                self.mark_dirty(uri);
            }
            "textDocument/didChange" => {
                let version = params.text_document.version;
                if self.store.change(&uri, version, &params.content_changes, self.encoding) {
                    self.mark_dirty(uri);
                }
            }
            "textDocument/didClose" => {
                self.store.close(&uri);
                self.dirty.retain(|dirty| dirty != &uri);
                // An editor keeps whatever it was last told, so a closed file
                // has to be told explicitly that it now has no problems.
                return vec![publication(&uri, Vec::new())];
            }
            _ => {}
        }

        Vec::new()
    }

    /// Runs a feature over an open document.
    fn feature(
        &mut self,
        id: RequestId,
        params: &Value,
        run: impl Fn(&mut Self, &DocumentParams, &str) -> Option<Value>,
    ) -> Message {
        let Ok(params) = serde_json::from_value::<DocumentParams>(params.clone()) else {
            return Message::error(id, error_code::INVALID_PARAMS, "expected a text document");
        };

        let uri = params.text_document.uri.clone();
        match run(self, &params, &uri) {
            Some(result) => Message::response(id, result),
            // A request for a document the client never opened is a client
            // bug, and answering with nothing is kinder than an error dialog.
            None => Message::response(id, Value::Null),
        }
    }

    fn mark_dirty(&mut self, uri: String) {
        if !self.dirty.contains(&uri) {
            self.dirty.push(uri);
        }
    }

    fn diagnostics_for(&mut self, uri: &str) -> Option<Message> {
        let encoding = self.encoding;
        let document = self.store.get_mut(uri)?;
        let analysis = document.analysis();
        let found = publish(&analysis, document.text(), document.index(), encoding);

        Some(publication(uri, found.iter().map(|found| json!(found)).collect()))
    }

    fn capabilities(&self) -> Value {
        json!({
            "capabilities": {
                "positionEncoding": self.encoding.as_token(),
                "textDocumentSync": { "openClose": true, "change": 2 },
                // `:` opens a frontmatter value, `{`, `.` and `#` open a mark
                // attribute list. Everything else completes on a word.
                "completionProvider": { "triggerCharacters": [":", "{", ".", "#"] },
                "documentSymbolProvider": true,
                "hoverProvider": true,
                // Whole-document only. A range format would have to decide what
                // a partly-selected construct means, and there is no honest
                // answer for half a frontmatter block.
                "documentFormattingProvider": true,
            },
            "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
        })
    }
}

/// Picks an encoding from what the client says it supports.
///
/// UTF-8 is preferred when offered because it is what the source already is,
/// so no conversion happens at all. UTF-16 is the fallback and the protocol's
/// default, and every client supports it.
fn negotiate(params: &Value) -> PositionEncoding {
    let offered: Vec<PositionEncoding> = params
        .get("general")
        .and_then(|general| general.get("positionEncodings"))
        .and_then(Value::as_array)
        .map(|encodings| {
            encodings.iter().filter_map(Value::as_str).filter_map(PositionEncoding::parse).collect()
        })
        .unwrap_or_default();

    if offered.contains(&PositionEncoding::Utf8) {
        return PositionEncoding::Utf8;
    }

    PositionEncoding::Utf16
}

fn publication(uri: &str, diagnostics: Vec<Value>) -> Message {
    Message::notification(
        "textDocument/publishDiagnostics",
        json!({ "uri": uri, "diagnostics": diagnostics }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const URI: &str = "file:///talks/slides/0001.md";

    fn request(id: i64, method: &str, params: Value) -> Message {
        Message {
            id: Some(RequestId::Number(id)),
            method: Some(method.to_string()),
            params: Some(params),
            ..Message::default()
        }
    }

    fn open(text: &str) -> Message {
        Message::notification(
            "textDocument/didOpen",
            json!({ "textDocument": { "uri": URI, "version": 1, "text": text } }),
        )
    }

    fn change(version: i64, text: &str) -> Message {
        Message::notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": URI, "version": version },
                "contentChanges": [{ "text": text }],
            }),
        )
    }

    /// A server that has been initialised and has one deck open.
    fn session(text: &str) -> Server {
        let mut server = Server::new();
        server.handle(request(1, "initialize", json!({})));
        server.handle(open(text));
        server
    }

    fn ask(server: &mut Server, method: &str, params: Value) -> Value {
        let replies = server.handle(request(9, method, params));
        replies[0].result.clone().expect("a result")
    }

    fn at(line: u32, character: u32) -> Value {
        json!({ "textDocument": { "uri": URI }, "position": { "line": line, "character": character } })
    }

    #[test]
    fn initialize_advertises_everything_the_server_actually_serves() {
        let mut server = Server::new();
        let result = ask(&mut server, "initialize", json!({}));
        let capabilities = &result["capabilities"];

        assert_eq!(capabilities["textDocumentSync"]["change"], 2, "incremental");
        assert_eq!(capabilities["documentSymbolProvider"], true);
        assert_eq!(capabilities["hoverProvider"], true);
        assert_eq!(capabilities["documentFormattingProvider"], true);
        assert!(capabilities["completionProvider"].is_object());
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn formatting_answers_with_the_edits_the_document_needs() {
        let mut server = session("---\ntheme: minimal\ntitle: T\n---\n\n- a <!--step-->\n");
        let edits = ask(&mut server, "textDocument/formatting", at(0, 0));

        assert_eq!(edits.as_array().unwrap().len(), 2, "{edits}");
        assert!(edits.as_array().unwrap().iter().any(|edit| edit["newText"] == "<!-- step -->"));
    }

    #[test]
    fn formatting_a_document_that_is_already_formatted_answers_with_nothing_to_do() {
        // An empty list, not a whole-file replacement. A client handed one of
        // those on every save moves the cursor and collapses the undo stack.
        let mut server = session("---\ntitle: T\n---\n\n# One\n");

        assert_eq!(ask(&mut server, "textDocument/formatting", at(0, 0)), json!([]));
    }

    #[test]
    fn formatting_reads_the_edits_a_burst_of_typing_left_behind() {
        // Formatting must not depend on `flush` having run: an editor formats on
        // save, which is one keystroke after the last change.
        let mut server = session("# One\n");
        server.handle(change(2, "# One\n\n- a <!--step-->\n"));

        let edits = ask(&mut server, "textDocument/formatting", at(0, 0));
        assert_eq!(edits[0]["newText"], "<!-- step -->");
    }

    #[test]
    fn an_unnegotiated_session_counts_columns_in_utf16() {
        // The protocol's default, and the only encoding every client has.
        let mut server = Server::new();
        server.handle(request(1, "initialize", json!({})));

        assert_eq!(server.encoding(), PositionEncoding::Utf16);
    }

    #[test]
    fn a_client_offering_utf8_gets_utf8_and_is_told_so() {
        // Free correctness: the source is already UTF-8, so nothing converts.
        let mut server = Server::new();
        let result = ask(
            &mut server,
            "initialize",
            json!({ "general": { "positionEncodings": ["utf-8", "utf-16"] } }),
        );

        assert_eq!(server.encoding(), PositionEncoding::Utf8);
        assert_eq!(result["capabilities"]["positionEncoding"], "utf-8");
    }

    #[test]
    fn opening_a_deck_owes_its_diagnostics() {
        let mut server = session("# One\n\n![](./a.png)\n");
        let published = server.flush();

        assert_eq!(published.len(), 1);
        assert_eq!(published[0].method.as_deref(), Some("textDocument/publishDiagnostics"));
        let params = published[0].params.clone().unwrap();
        assert_eq!(params["uri"], URI);
        assert_eq!(params["diagnostics"][0]["source"], "slidx");
    }

    #[test]
    fn a_burst_of_edits_publishes_once_and_publishes_the_last_one() {
        // What typing looks like: several changes arrive before anything is
        // asked of the server, and only the final text is worth reporting on.
        let mut server = session("# One\n");
        server.flush();

        server.handle(change(2, "# One\n\n![](./a.png)\n"));
        server.handle(change(3, "# One\n\n![alt](./a.png)\n"));

        let published = server.flush();
        assert_eq!(published.len(), 1, "one publication for two edits");
        assert_eq!(
            published[0].params.clone().unwrap()["diagnostics"].as_array().unwrap().len(),
            0
        );
    }

    #[test]
    fn nothing_is_owed_when_nothing_has_changed() {
        let mut server = session("# One\n");
        server.flush();

        assert!(server.flush().is_empty());
    }

    #[test]
    fn closing_a_deck_clears_the_problems_the_editor_is_still_showing() {
        let mut server = session("# One\n\n![](./a.png)\n");
        server.flush();

        let published = server.handle(Message::notification(
            "textDocument/didClose",
            json!({ "textDocument": { "uri": URI } }),
        ));

        assert_eq!(published.len(), 1);
        assert_eq!(
            published[0].params.clone().unwrap()["diagnostics"].as_array().unwrap().len(),
            0
        );
        assert!(server.flush().is_empty(), "and it is no longer owed anything");
    }

    #[test]
    fn an_incremental_edit_is_applied_to_the_open_document() {
        let mut server = session("# One\n");
        server.handle(Message::notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": URI, "version": 2 },
                "contentChanges": [{
                    "range": {
                        "start": { "line": 0, "character": 2 },
                        "end": { "line": 0, "character": 5 },
                    },
                    "text": "Two",
                }],
            }),
        ));

        let symbols = ask(&mut server, "textDocument/documentSymbol", at(0, 0));
        assert_eq!(symbols[0]["name"], "Two");
    }

    #[test]
    fn the_outline_survives_a_fence_being_opened() {
        // The failure this is built to avoid: an outline pane that empties
        // every time an author starts a code block.
        let mut server = session("# One\n\n---\n\n# Two\n");
        assert_eq!(
            ask(&mut server, "textDocument/documentSymbol", at(0, 0)).as_array().unwrap().len(),
            2
        );

        server.handle(change(2, "```rust\n\n# One\n\n---\n\n# Two\n"));
        let symbols = ask(&mut server, "textDocument/documentSymbol", at(0, 0));

        assert_eq!(symbols.as_array().unwrap().len(), 2, "still two, from the last good parse");
    }

    #[test]
    fn completion_answers_from_the_document_the_request_names() {
        let mut server = session("---\ntheme: \n---\n\n# One\n");
        let items = ask(&mut server, "textDocument/completion", at(1, 7));

        assert!(items.as_array().unwrap().iter().any(|item| item["label"] == "terminal"));
    }

    #[test]
    fn hover_answers_null_rather_than_an_error_when_there_is_nothing_to_say() {
        let mut server = session("# One\n\nprose\n");
        assert_eq!(ask(&mut server, "textDocument/hover", at(2, 2)), Value::Null);
    }

    #[test]
    fn a_request_for_a_document_that_was_never_opened_answers_nothing() {
        let mut server = Server::new();
        server.handle(request(1, "initialize", json!({})));

        let replies = server.handle(request(
            9,
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": "file:///talks/slides/gone.md" } }),
        ));

        assert_eq!(replies[0].result, Some(Value::Null));
        assert!(replies[0].error.is_none(), "not an error dialog for a client bug");
    }

    #[test]
    fn a_markdown_file_that_is_not_a_deck_is_never_opened() {
        // A client that cannot scope by path — a Zed extension binds to a
        // language and nothing finer — sends every Markdown file it has. None
        // of them may come back with a slidx finding on it.
        let readme = "file:///talks/vueconf/README.md";
        let mut server = Server::new();
        server.handle(request(1, "initialize", json!({})));
        server.handle(Message::notification(
            "textDocument/didOpen",
            json!({
                "textDocument": { "uri": readme, "version": 1, "text": "# Build\n\n![](./a.png)\n" },
            }),
        ));

        assert!(server.flush().is_empty(), "nothing is owed for a file that is not a deck");

        let replies = server.handle(request(
            9,
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": readme } }),
        ));
        assert_eq!(replies[0].result, Some(Value::Null), "and it has no outline either");
    }

    #[test]
    fn an_unknown_method_is_refused_by_name() {
        let mut server = session("# One\n");
        let replies = server.handle(request(9, "textDocument/rename", json!({})));

        assert_eq!(replies[0].error.as_ref().unwrap().code, error_code::METHOD_NOT_FOUND);
    }

    #[test]
    fn an_unknown_notification_is_ignored_rather_than_answered() {
        let mut server = session("# One\n");
        assert!(server
            .handle(Message::notification("$/setTrace", json!({ "value": "off" })))
            .is_empty());
    }

    #[test]
    fn shutdown_then_exit_ends_the_session() {
        let mut server = session("# One\n");

        let replies = server.handle(request(9, "shutdown", Value::Null));
        assert_eq!(replies[0].result, Some(Value::Null));
        assert_eq!(server.state(), State::ShuttingDown);
        assert!(!server.should_exit());

        server.handle(Message::notification("exit", Value::Null));
        assert!(server.should_exit());
    }

    #[test]
    fn a_request_after_shutdown_is_refused() {
        // Required by the protocol, and the alternative is answering with
        // state the client has already discarded.
        let mut server = session("# One\n");
        server.handle(request(9, "shutdown", Value::Null));

        let replies = server.handle(request(10, "textDocument/hover", at(0, 0)));
        assert_eq!(replies[0].error.as_ref().unwrap().code, error_code::INVALID_REQUEST);
    }

    #[test]
    fn a_japanese_deck_is_edited_and_reported_in_code_units() {
        let mut server = session("---\ntitle: 高速なデッキ\naspect: 21x9\n---\n\n# 導入\n");
        let published = server.flush();
        let diagnostics = &published[0].params.clone().unwrap()["diagnostics"];

        assert_eq!(diagnostics[0]["range"]["start"]["line"], 1);
        assert_eq!(diagnostics[0]["range"]["end"]["character"], 12, "`aspect: 21x9`");
    }
}
