//! The editor's side of `slidx lsp`, driven through a real process.
//!
//! The language server's own suite proves the server answers. It proved that
//! before anything could start it: nothing outside `slidx_lsp` spawned a
//! process, so every diagnostic, completion and hover in that crate reached
//! nobody. This file is the missing half — the binary an editor extension
//! actually launches, launched the way that extension launches it, answering
//! over a real pipe.
//!
//! # The argv is read from the extension rather than typed here
//!
//! `packages/vscode/src/server.ts` decides what VS Code spawns, and nothing in
//! Rust can read a TypeScript constant. So this test reads that file and runs
//! what it found: change the extension's subcommand and this fails, which is
//! the only way two languages can be held to one answer.
//!
//! The same trick pins the deck glob and the install-directory order. Each is
//! stated twice because a `DocumentSelector` and a `PATH` walk both live on the
//! other side of a boundary Rust cannot cross — and each restatement is worth
//! having only while something checks it.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

use slidx_cli::home::{Env, Home};

/// A deck file, in the layout the server serves and nothing else.
const DECK_URI: &str = "file:///talks/vueconf/slides/0001.md";

/// A deck with one thing wrong that the author can act on.
const DECK: &str = "# 導入\n\n![](./図.png)\n";

// ---------------------------------------------------------------- the session

/// A running `slidx lsp`, with its pipes.
struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl Session {
    /// Starts the server the way the VS Code extension starts it.
    fn start() -> Self {
        let mut child = Command::new(binary())
            .args(server_arguments())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("slidx starts");

        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));

        Self { child, stdin, stdout, next_id: 0 }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.write(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Sends a request and reads until its answer comes back.
    ///
    /// Notifications arriving in between are skipped rather than dropped —
    /// diagnostics are published whenever the queue drains, so one may well
    /// land between a request and its response.
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));

        loop {
            let message = self.read();
            if message.get("id").and_then(Value::as_i64) == Some(id) {
                return message.get("result").cloned().unwrap_or(Value::Null);
            }
        }
    }

    /// Reads until a notification of this method arrives.
    fn wait_for(&mut self, method: &str) -> Value {
        loop {
            let message = self.read();
            if message.get("method").and_then(Value::as_str) == Some(method) {
                return message.get("params").cloned().unwrap_or(Value::Null);
            }
        }
    }

    fn write(&mut self, message: &Value) {
        let body = serde_json::to_vec(message).expect("serialisable");
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).expect("header");
        self.stdin.write_all(&body).expect("body");
        self.stdin.flush().expect("flush");
    }

    /// One frame off the wire, framed exactly as the protocol frames it.
    fn read(&mut self) -> Value {
        let mut length = None;

        loop {
            let mut header = String::new();
            let read = self.stdout.read_line(&mut header).expect("a header");
            assert_ne!(read, 0, "the server closed the stream");

            let header = header.trim_end_matches(['\r', '\n']);
            if header.is_empty() {
                break;
            }
            if let Some(value) = header.strip_prefix("Content-Length:") {
                length = value.trim().parse::<usize>().ok();
            }
        }

        let mut body = vec![0u8; length.expect("a Content-Length")];
        self.stdout.read_exact(&mut body).expect("a body");

        serde_json::from_slice(&body).expect("json")
    }

    /// Ends the session the way a client ends it, and reports the exit code.
    fn finish(mut self) -> i32 {
        self.request("shutdown", Value::Null);
        self.notify("exit", Value::Null);
        drop(self.stdin);

        self.child.wait().expect("the server exits").code().unwrap_or(-1)
    }
}

fn binary() -> PathBuf {
    // `cargo test` puts integration binaries next to the ones they test.
    let mut path = std::env::current_exe().expect("test binary");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join(if cfg!(windows) { "slidx.exe" } else { "slidx" })
}

fn open(uri: &str, text: &str) -> Value {
    json!({ "textDocument": { "uri": uri, "languageId": "markdown", "version": 1, "text": text } })
}

// ----------------------------------------------------------------- the tests

#[test]
fn an_editor_starts_the_server_and_is_told_what_it_can_ask_for() {
    let mut session = Session::start();
    let capabilities = session.request("initialize", json!({}))["capabilities"].clone();

    assert_eq!(capabilities["textDocumentSync"]["change"], 2, "incremental");
    assert_eq!(capabilities["documentSymbolProvider"], true);
    assert_eq!(capabilities["hoverProvider"], true);
    assert_eq!(capabilities["documentFormattingProvider"], true);
    assert!(capabilities["completionProvider"].is_object());

    assert_eq!(session.finish(), 0, "and the session ends when the client says so");
}

#[test]
fn opening_a_deck_publishes_the_findings_an_author_can_act_on() {
    // The whole reason this item exists. Everything below has been true and
    // untested from outside the crate that implements it: until something
    // spawned this process, no author had seen one of these.
    let mut session = Session::start();
    session.request("initialize", json!({}));
    session.notify("textDocument/didOpen", open(DECK_URI, DECK));

    let published = session.wait_for("textDocument/publishDiagnostics");

    assert_eq!(published["uri"], DECK_URI);
    assert_eq!(published["diagnostics"][0]["code"], "structure/missing-alt");
    assert_eq!(published["diagnostics"][0]["source"], "slidx");
    assert!(
        published["diagnostics"][0]["message"]
            .as_str()
            .expect("a message")
            .contains("describe what the image shows"),
        "the remedy travels with the finding: {published}",
    );

    assert_eq!(session.finish(), 0);
}

#[test]
fn the_outline_the_editor_draws_comes_back_over_the_same_pipe() {
    let mut session = Session::start();
    session.request("initialize", json!({}));
    session.notify("textDocument/didOpen", open(DECK_URI, "# 導入\n\n---\n\n# まとめ\n"));

    let symbols = session
        .request("textDocument/documentSymbol", json!({ "textDocument": { "uri": DECK_URI } }));

    assert_eq!(symbols[0]["name"], "導入");
    assert_eq!(symbols[1]["name"], "まとめ");
    assert_eq!(session.finish(), 0);
}

#[test]
fn completion_offers_the_theme_names_the_binary_was_built_with() {
    // The derived-from-Rust half, reaching an editor: a theme added to
    // `builtin::all` is offered here without anyone editing a client.
    let mut session = Session::start();
    session.request("initialize", json!({}));
    session.notify("textDocument/didOpen", open(DECK_URI, "---\ntheme: \n---\n\n# One\n"));

    let items = session.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": DECK_URI },
            "position": { "line": 1, "character": 7 },
        }),
    );
    let labels: Vec<&str> =
        items.as_array().expect("a list").iter().filter_map(|i| i["label"].as_str()).collect();

    assert!(labels.contains(&"terminal"), "{labels:?}");
    assert_eq!(session.finish(), 0);
}

#[test]
fn a_readme_in_the_same_workspace_is_never_given_a_slide_diagnostic() {
    // A client that cannot scope by path sends every Markdown file it has, and
    // an editor that answered would put deck findings on somebody's
    // documentation. The rule is the server's, so every editor gets it.
    let mut session = Session::start();
    session.request("initialize", json!({}));
    session.notify("textDocument/didOpen", open("file:///talks/vueconf/README.md", DECK));
    session.notify("textDocument/didOpen", open(DECK_URI, DECK));

    // The deck was opened second, so anything published before its own
    // diagnostics would have to be the README's.
    let published = session.wait_for("textDocument/publishDiagnostics");

    assert_eq!(published["uri"], DECK_URI, "the first thing published is the deck");
    assert_eq!(session.finish(), 0);
}

#[test]
fn formatting_on_save_answers_with_the_edits_and_nothing_more() {
    let mut session = Session::start();
    session.request("initialize", json!({}));
    session.notify("textDocument/didOpen", open(DECK_URI, "# One\n\n- a <!--step-->\n"));

    let edits = session.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": DECK_URI }, "options": { "tabSize": 2 } }),
    );

    assert_eq!(edits.as_array().expect("a list").len(), 1, "{edits}");
    assert_eq!(edits[0]["newText"], "<!-- step -->");
    assert_eq!(session.finish(), 0);
}

// -------------------------------------------------- what the clients restate

/// Everything the VS Code extension decides, read out of its own source.
fn extension_constant(file: &str, name: &str) -> String {
    let source = fs::read_to_string(repository().join("packages/vscode/src").join(file))
        .unwrap_or_else(|error| panic!("{file}: {error}"));
    let needle = format!("export const {name} = \"");

    let after = source
        .split_once(&needle)
        .unwrap_or_else(|| panic!("{file} declares no {name}"))
        .1;

    after.split_once('"').expect("a closing quote").0.to_string()
}

/// A string list the extension exports, as its members in order.
fn extension_list(file: &str, name: &str) -> Vec<String> {
    let source = fs::read_to_string(repository().join("packages/vscode/src").join(file))
        .unwrap_or_else(|error| panic!("{file}: {error}"));
    let needle = format!("export const {name} = [");

    let after =
        source.split_once(&needle).unwrap_or_else(|| panic!("{file} declares no {name}")).1;
    let inside = after.split_once(']').expect("a closing bracket").0;

    inside
        .split(',')
        .map(|member| member.trim().trim_matches('"').to_string())
        .filter(|member| !member.is_empty())
        .collect()
}

/// The arguments the extension spawns `slidx` with.
fn server_arguments() -> Vec<String> {
    vec![extension_constant("server.ts", "SERVER_COMMAND")]
}

fn repository() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `crates/slidx_cli`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_extension_spawns_the_subcommand_this_binary_actually_has() {
    // Which is also what every test above ran, so a rename that missed the
    // extension fails six tests rather than passing silently and shipping an
    // extension that starts nothing.
    assert_eq!(server_arguments(), ["lsp"]);
}

#[test]
fn the_glob_the_extension_filters_on_is_the_rule_the_server_enforces() {
    // Two statements of one rule, in two languages. The client's saves the
    // traffic; the server's is what decides — and a client filtering more
    // narrowly than the server would hide files the server would have served.
    assert_eq!(extension_constant("server.ts", "DECK_GLOB"), slidx_lsp::deck::DECK_GLOB);
}

#[test]
fn the_extension_looks_for_slidx_where_the_installer_puts_it() {
    // slidx installs into $SLIDX_HOME, else $XDG_DATA_HOME/slidx, else
    // ~/.slidx — and %LOCALAPPDATA%\slidx on Windows, where XDG is not a
    // convention that platform has. An extension that resolved that order
    // differently would run a different binary from the one the author's own
    // terminal runs, which is the failure `slidx version current` exists to
    // report and the one nobody thinks to check.
    assert_eq!(extension_list("binary.ts", "HOME_VARIABLES"), ["SLIDX_HOME", "XDG_DATA_HOME", "HOME"]);
    assert_eq!(
        extension_list("binary.ts", "WINDOWS_HOME_VARIABLES"),
        ["SLIDX_HOME", "LOCALAPPDATA", "USERPROFILE"]
    );
}

#[test]
fn and_that_order_is_the_order_this_binary_resolves() {
    // The other half of the pin: the list above is only worth checking against
    // if it describes what `Home` does, so each variable is shown to outrank
    // the ones after it.
    let unix = Env {
        slidx_home: Some("/opt/slidx".into()),
        xdg_data_home: Some("/data".into()),
        home: Some("/home/somebody".into()),
        ..Env::default()
    };

    assert_eq!(Home::from_env(&unix).root(), PathBuf::from("/opt/slidx"));
    assert_eq!(
        Home::from_env(&Env { slidx_home: None, ..unix.clone() }).root(),
        PathBuf::from("/data/slidx")
    );
    assert_eq!(
        Home::from_env(&Env { slidx_home: None, xdg_data_home: None, ..unix }).root(),
        PathBuf::from("/home/somebody/.slidx")
    );

    let windows = Env {
        local_app_data: Some("C:\\Local".into()),
        user_profile: Some("C:\\Users\\somebody".into()),
        xdg_data_home: Some("/data".into()),
        windows: true,
        ..Env::default()
    };

    assert!(Home::from_env(&windows).root().starts_with("C:\\Local"));
    assert!(Home::from_env(&Env { local_app_data: None, ..windows })
        .root()
        .starts_with("C:\\Users\\somebody"));
}
