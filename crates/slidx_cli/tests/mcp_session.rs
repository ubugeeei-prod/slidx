//! `slidx mcp`, stated as recorded JSON-RPC.
//!
//! Every test here writes the lines a client writes and reads the lines the
//! server writes back. Nothing reaches inside the server, and that is the point:
//! the unit tests beside each module exercise slidx's own abstractions, and a
//! protocol is not one of those. What a client sees is the contract, so the
//! contract is what is written down.
//!
//! Two things this catches that a unit test cannot:
//!
//! **A response to a notification.** It is a protocol violation and it looks
//! like nothing at all from inside a dispatcher — the frame is simply written to
//! a stream nobody asserted on. Counting the lines on the wire catches it.
//!
//! **Anything on standard output that is not a frame.** A single stray `println!`
//! desynchronises the client for the rest of the session, and a server that
//! logged one line at startup would pass every test that inspected its return
//! values.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use slidx_cli::mcp::{self, Session, Workspace};

/// A scratch project that cleans up after itself.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("slidx-mcp-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("slides")).expect("a scratch project");
        Self(path)
    }

    /// Writes one slide file and returns the project directory.
    fn slide(&self, name: &str, body: &str) -> &Path {
        fs::write(self.0.join("slides").join(name), body).expect("write");
        &self.0
    }

    fn deck(&self) -> String {
        self.0.join("slides").display().to_string()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs a recorded exchange and returns the raw bytes the server wrote.
fn wire(root: &Path, script: &[&str]) -> String {
    let input: String = script.iter().map(|line| format!("{line}\n")).collect();
    let mut output = Vec::new();
    let mut session = Session::new(Workspace::new(vec![root.to_path_buf()]));

    mcp::serve(&mut input.as_bytes(), &mut output, &mut session).expect("the session ran");

    String::from_utf8(output).expect("frames are UTF-8")
}

/// The frames the server wrote, parsed.
fn exchange(root: &Path, script: &[&str]) -> Vec<Value> {
    wire(root, script)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|_| panic!("not a frame: {line}")))
        .collect()
}

/// An exchange in a directory the server is allowed to read but which holds no
/// deck, for the tests that never mention one.
fn talk(script: &[&str]) -> Vec<Value> {
    exchange(&std::env::temp_dir(), script)
}

fn initialize(version: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": version,
            "capabilities": {},
            "clientInfo": { "name": "a-client", "version": "1.0.0" },
        },
    })
    .to_string()
}

/// The current revision, as a client that is up to date sends it.
fn hello() -> String {
    initialize(mcp::PROTOCOL_VERSION)
}

fn call(id: i64, name: &str, arguments: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    })
    .to_string()
}

const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
const LIST_TOOLS: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;

#[test]
fn the_version_a_client_offers_is_the_version_it_is_answered_in() {
    let frames = talk(&[&hello()]);

    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["id"], 1);
    assert_eq!(frames[0]["result"]["protocolVersion"], mcp::PROTOCOL_VERSION);
    assert_eq!(frames[0]["result"]["serverInfo"]["name"], "slidx");
}

#[test]
fn an_older_revision_this_server_still_speaks_is_agreed_to_rather_than_upgraded() {
    // Answering with a newer revision than the client asked for would leave it
    // reading fields it has no idea about.
    let frames = talk(&[&initialize("2024-11-05")]);

    assert_eq!(frames[0]["result"]["protocolVersion"], "2024-11-05");
}

#[test]
fn a_revision_this_server_does_not_speak_is_refused_by_naming_the_ones_it_does() {
    // A client that is only told no cannot retry. The list is what lets it.
    let frames = talk(&[&initialize("1.0.0")]);

    let error = &frames[0]["error"];
    assert_eq!(error["code"], -32602);
    assert!(error["message"].as_str().expect("a message").contains("1.0.0"), "{error}");
    assert_eq!(error["data"]["supported"], json!(mcp::SUPPORTED));
}

#[test]
fn a_client_that_names_no_revision_at_all_is_told_which_ones_exist() {
    // Rather than assuming the newest and failing later on a field it cannot
    // read. The version is negotiated, so there is nothing to assume.
    let frames = talk(&[r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#]);

    assert_eq!(frames[0]["error"]["code"], -32602);
    assert_eq!(frames[0]["error"]["data"]["supported"], json!(mcp::SUPPORTED));
}

#[test]
fn the_server_teaches_the_dialect_before_a_client_can_get_it_wrong() {
    // An agent that has not been told about takes writes two marks that swap
    // instead of one that changes. There is nowhere later to say so.
    let frames = talk(&[&hello()]);
    let instructions = frames[0]["result"]["instructions"].as_str().expect("instructions");

    for subject in ["take", "snapshot", "mark", "notes:"] {
        assert!(instructions.contains(subject), "instructions never mention {subject}");
    }
}

#[test]
fn a_read_only_server_says_so_in_the_instructions_a_client_reads_first() {
    let instructions =
        talk(&[&hello()])[0]["result"]["instructions"].as_str().expect("instructions").to_string();

    assert!(instructions.to_lowercase().contains("read-only"), "{instructions}");
}

#[test]
fn a_notification_is_never_answered() {
    // A response to a notification is a protocol violation: the client has no
    // request to match it against, and most clients log it and carry on, so
    // nothing would ever tell us.
    let frames = talk(&[
        &hello(),
        INITIALIZED,
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#,
    ]);

    assert_eq!(frames.len(), 1, "only initialize is answered: {frames:?}");
}

#[test]
fn an_unknown_notification_is_ignored_rather_than_refused() {
    let frames = talk(&[&hello(), r#"{"jsonrpc":"2.0","method":"notifications/whatever"}"#]);

    assert_eq!(frames.len(), 1, "{frames:?}");
}

#[test]
fn a_request_before_initialize_is_refused_and_says_what_to_send_first() {
    let frames = talk(&[LIST_TOOLS]);

    assert_eq!(frames[0]["error"]["code"], -32600);
    assert!(frames[0]["error"]["message"].as_str().expect("a message").contains("initialize"));
}

#[test]
fn ping_is_answered_before_initialize_because_it_is_how_a_client_checks_us() {
    let frames = talk(&[r#"{"jsonrpc":"2.0","id":9,"method":"ping"}"#]);

    assert_eq!(frames[0]["result"], json!({}));
}

#[test]
fn initialising_twice_is_refused_rather_than_renegotiated() {
    // The second one would change the shape of every reply mid-session.
    let frames = talk(&[&hello(), INITIALIZED, &hello()]);

    assert!(frames[0]["result"].is_object());
    assert_eq!(frames[1]["error"]["code"], -32600);
}

#[test]
fn an_unknown_method_is_refused_by_name() {
    let frames = talk(&[&hello(), r#"{"jsonrpc":"2.0","id":3,"method":"resources/subscribe"}"#]);

    assert_eq!(frames[1]["error"]["code"], -32601);
    assert!(frames[1]["error"]["message"]
        .as_str()
        .expect("a message")
        .contains("resources/subscribe"));
}

#[test]
fn a_line_that_is_not_json_is_answered_with_a_null_id_and_the_session_continues() {
    // Newline framing resynchronises at the next newline, so one bad line is
    // not the end of the session. It is with header framing, and that
    // difference is why the two are separate modules.
    let frames = talk(&["not json at all", &hello()]);

    assert_eq!(frames[0]["error"]["code"], -32700);
    assert_eq!(frames[0]["id"], Value::Null, "there was no id to echo");
    assert!(frames[1]["result"].is_object(), "and initialize still worked");
}

#[test]
fn a_batch_is_refused_as_a_request_rather_than_as_a_syntax_error() {
    // The client's JSON was fine. Calling it a parse error would send somebody
    // looking for a bug in their serialiser.
    let frames = talk(&[r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#]);

    assert_eq!(frames[0]["error"]["code"], -32600);
    assert!(frames[0]["error"]["message"].as_str().expect("a message").contains("batch"));
}

#[test]
fn the_client_closing_the_stream_ends_the_session() {
    // MCP has no shutdown request: the client closes stdin. A server waiting
    // for one would hang on every disconnect.
    assert_eq!(talk(&[&hello()]).len(), 1);
}

#[test]
fn every_tool_is_listed_with_a_name_a_description_and_a_schema() {
    let frames = talk(&[&hello(), LIST_TOOLS]);
    let tools = frames[1]["result"]["tools"].as_array().expect("tools");

    assert!(!tools.is_empty());
    for tool in tools {
        assert!(tool["name"].as_str().is_some_and(|name| !name.is_empty()), "{tool}");
        assert!(tool["description"].as_str().is_some_and(|text| text.len() > 20), "{tool}");
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
    }
}

#[test]
fn a_tool_that_does_not_exist_is_a_protocol_error_and_a_tool_that_fails_is_not() {
    // The distinction the specification draws and clients act on: an unknown
    // tool is the client's mistake, and a tool that ran and could not do the
    // job is a result the model has to read and act on.
    let frames =
        talk(&[&hello(), &call(3, "no_such_tool", json!({})), &call(4, "lint_deck", json!({}))]);

    assert_eq!(frames[1]["error"]["code"], -32602);
    assert!(frames[2]["error"].is_null(), "a failing tool is not a protocol error: {}", frames[2]);
    assert_eq!(frames[2]["result"]["isError"], true);
    assert!(frames[2]["result"]["content"][0]["text"].as_str().is_some());
}

#[test]
fn structured_output_is_sent_only_to_a_client_that_negotiated_a_revision_with_it() {
    // `structuredContent` and a tool's `outputSchema` arrived in 2025-06-18.
    // Sending either to a 2024-11-05 client would be inventing a field it has
    // no way to read, so the shape follows the negotiation.
    let scratch = Scratch::new("structured");
    let root = scratch.slide("0001.md", "# One\n\n- a\n- b\n").to_path_buf();
    let arguments = json!({ "deck": scratch.deck() });

    let current =
        exchange(&root, &[&hello(), &call(3, "lint_deck", arguments.clone()), LIST_TOOLS]);
    assert!(current[1]["result"]["structuredContent"].is_object(), "{}", current[1]);
    assert!(current[2]["result"]["tools"][0]["outputSchema"].is_object());

    let older =
        exchange(&root, &[&initialize("2024-11-05"), &call(3, "lint_deck", arguments), LIST_TOOLS]);
    assert!(older[1]["result"]["structuredContent"].is_null(), "{}", older[1]);
    assert!(older[2]["result"]["tools"][0]["outputSchema"].is_null());
    assert!(
        older[1]["result"]["content"][0]["text"].as_str().is_some(),
        "and the answer still arrives, as text"
    );
}

#[test]
fn linting_a_deck_over_the_wire_finds_what_the_command_finds() {
    // The one rule that blocks: a deck that fetches an asset over the network.
    let scratch = Scratch::new("lint");
    let root = scratch
        .slide("0001.md", "# One\n\n![a diagram](https://cdn.example.com/a.png)\n")
        .to_path_buf();

    let frames =
        exchange(&root, &[&hello(), &call(3, "lint_deck", json!({ "deck": scratch.deck() }))]);
    let result = &frames[1]["result"];

    assert_eq!(result["isError"], false);
    let codes: Vec<&str> = result["structuredContent"]["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .map(|found| found["code"].as_str().unwrap_or_default())
        .collect();

    assert!(codes.contains(&"offline/remote-asset"), "{codes:?}");
    assert_eq!(result["structuredContent"]["blocking"], 1);
}

#[test]
fn a_deck_outside_the_directories_the_server_was_given_is_refused() {
    // The server is pointed at a project, and a deck's own content is
    // untrusted input. A path that arrived in an argument does not widen that.
    let scratch = Scratch::new("outside");
    let root = scratch.slide("0001.md", "# One\n").to_path_buf();

    // The directory the scratch sits in: it exists on every platform this ships
    // to, and it is not under the one root this server was given.
    let above = std::env::temp_dir().display().to_string();
    let frames = exchange(&root, &[&hello(), &call(3, "lint_deck", json!({ "deck": above }))]);

    assert_eq!(frames[1]["result"]["isError"], true);
    assert!(frames[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("a reason")
        .contains("outside"));
}

#[test]
fn the_machine_check_answers_without_being_given_a_deck() {
    // `slidx doctor` is about the room, not the talk. Every reading it could
    // not take is reported as unknown, never as a pass — so this passes on a
    // continuous integration runner with no battery and no window server.
    let frames = talk(&[&hello(), &call(3, "check_machine", json!({ "offline": true }))]);
    let result = &frames[1]["result"];

    assert!(result["structuredContent"]["findings"].as_array().is_some_and(|f| !f.is_empty()));
    assert!(result["content"][0]["text"].as_str().expect("a report").contains("slidx doctor"));
}

#[test]
fn nothing_reaches_standard_output_that_is_not_a_frame() {
    // One stray line of logging desynchronises the client for the rest of the
    // session, and every test that read a return value would still pass.
    let scratch = Scratch::new("stdout");
    let root = scratch.slide("0001.md", "# One\n").to_path_buf();

    let raw = wire(
        &root,
        &[
            "not json",
            &hello(),
            LIST_TOOLS,
            &call(3, "lint_deck", json!({ "deck": scratch.deck() })),
            &call(4, "check_machine", json!({ "offline": true })),
        ],
    );

    for line in raw.lines() {
        let frame: Value =
            serde_json::from_str(line).unwrap_or_else(|_| panic!("not JSON: {line}"));
        assert_eq!(frame["jsonrpc"], "2.0", "{line}");
        assert!(frame["result"].is_object() || frame["error"].is_object(), "{line}");
    }

    assert!(raw.ends_with('\n'), "a frame ends with its newline");
}
