//! A whole editing session, over the wire.
//!
//! The unit tests hold each piece: framing in `protocol`, dispatch in
//! `server`, arithmetic in `position`. This holds them together — every
//! message is really framed, really parsed, and really answered, in the order
//! an editor sends them. It is the test that would fail if the pieces were
//! individually right and jointly wrong.
//!
//! The deck is in Japanese on purpose. A column counted in bytes passes every
//! ASCII test there is.

use std::io::Cursor;

use serde_json::{json, Value};

use slidx_lsp::protocol::{self, Message, RequestId};
use slidx_lsp::Server;

const URI: &str = "file:///デッキ.md";

const DECK: &str = "---\ntitle: 高速なデッキ\ntheme: terminal\n---\n\n# 導入\n\n\
                    - 速い <!-- step -->\n\n---\n\n# まとめ\n";

/// Frames a message, reads it back, and runs it through the server, framing
/// whatever comes out. Nothing here shortcuts the wire.
fn exchange(server: &mut Server, message: Message) -> Vec<Message> {
    let mut wire = Vec::new();
    protocol::write(&mut wire, &message).expect("framed");

    let mut input = Cursor::new(wire);
    let received = protocol::read(&mut input).expect("readable").expect("a message");

    let replies = server.handle(received);

    let mut out = Vec::new();
    for reply in &replies {
        protocol::write(&mut out, reply).expect("framed");
    }

    let mut cursor = Cursor::new(out);
    std::iter::from_fn(|| protocol::read(&mut cursor).ok().flatten()).collect()
}

fn request(id: i64, method: &str, params: Value) -> Message {
    Message {
        id: Some(RequestId::Number(id)),
        method: Some(method.to_string()),
        params: Some(params),
        ..Message::default()
    }
}

fn result(replies: &[Message]) -> Value {
    replies[0].result.clone().expect("a result")
}

fn at(line: u32, character: u32) -> Value {
    json!({
        "textDocument": { "uri": URI },
        "position": { "line": line, "character": character },
    })
}

fn opened() -> Server {
    let mut server = Server::new();
    exchange(&mut server, request(1, "initialize", json!({})));
    exchange(
        &mut server,
        Message::notification(
            "textDocument/didOpen",
            json!({ "textDocument": { "uri": URI, "version": 1, "text": DECK } }),
        ),
    );
    // The stdio loop flushes after every burst of input, so a document is
    // always analysed once before anything is asked of it. A helper that
    // skipped this would be testing a state the server is never in.
    server.flush();
    server
}

#[test]
fn a_session_starts_by_agreeing_on_what_a_column_is() {
    let mut server = Server::new();
    let replies = exchange(&mut server, request(1, "initialize", json!({})));

    assert_eq!(result(&replies)["capabilities"]["positionEncoding"], "utf-16");
}

#[test]
fn a_japanese_deck_outlines_by_slide_and_by_step() {
    let mut server = opened();
    let replies = exchange(&mut server, request(2, "textDocument/documentSymbol", at(0, 0)));
    let symbols = result(&replies);

    assert_eq!(symbols[0]["name"], "導入");
    assert_eq!(symbols[0]["detail"], "2 stops");
    assert_eq!(symbols[0]["children"][0]["name"], "reveal step 1");
    assert_eq!(symbols[1]["name"], "まとめ");
}

#[test]
fn completion_after_japanese_text_offers_the_themes_that_exist() {
    let mut server = opened();
    // Line three, past `theme: `, which is column seven in any encoding — but
    // the line above it is not, and the index had to walk over it.
    let replies = exchange(&mut server, request(2, "textDocument/completion", at(2, 7)));
    let items = result(&replies);
    let labels: Vec<&str> =
        items.as_array().unwrap().iter().map(|item| item["label"].as_str().unwrap()).collect();

    assert!(labels.contains(&"terminal"), "{labels:?}");
}

#[test]
fn hover_over_a_title_written_in_japanese_highlights_the_right_span() {
    let mut server = opened();
    let replies = exchange(&mut server, request(2, "textDocument/hover", at(1, 2)));
    let hovered = result(&replies);

    assert!(hovered["contents"]["value"].as_str().unwrap().contains("**title**"));
    assert_eq!(hovered["range"]["start"]["character"], 0);
    assert_eq!(hovered["range"]["end"]["character"], 5, "`title`, not its bytes");
}

#[test]
fn an_edit_expressed_in_utf16_columns_lands_where_the_editor_meant_it() {
    let mut server = opened();

    // Replace 導入 with 概要 — columns two to four of a line whose kanji are
    // three bytes each.
    exchange(
        &mut server,
        Message::notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": URI, "version": 2 },
                "contentChanges": [{
                    "range": {
                        "start": { "line": 5, "character": 2 },
                        "end": { "line": 5, "character": 4 },
                    },
                    "text": "概要",
                }],
            }),
        ),
    );

    let replies = exchange(&mut server, request(3, "textDocument/documentSymbol", at(0, 0)));
    assert_eq!(result(&replies)[0]["name"], "概要");
}

#[test]
fn a_deck_with_a_problem_publishes_it_once_the_typing_stops() {
    let mut server = Server::new();
    exchange(&mut server, request(1, "initialize", json!({})));
    exchange(
        &mut server,
        Message::notification(
            "textDocument/didOpen",
            json!({
                "textDocument": { "uri": URI, "version": 1, "text": "# 導入\n\n![](./図.png)\n" },
            }),
        ),
    );

    let published = server.flush();
    let diagnostics = published[0].params.clone().unwrap()["diagnostics"].clone();

    assert_eq!(diagnostics[0]["code"], "structure/missing-alt");
    assert_eq!(diagnostics[0]["source"], "slidx");
    assert!(
        diagnostics[0]["message"].as_str().unwrap().contains("describe what the image shows"),
        "the remedy travels with the finding",
    );
    assert!(server.flush().is_empty(), "and is not published twice");
}

#[test]
fn the_outline_holds_while_a_fence_is_being_typed() {
    let mut server = opened();

    exchange(
        &mut server,
        Message::notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": URI, "version": 2 },
                "contentChanges": [{ "text": format!("```rust\n{DECK}") }],
            }),
        ),
    );

    let replies = exchange(&mut server, request(3, "textDocument/documentSymbol", at(0, 0)));
    assert_eq!(
        result(&replies).as_array().unwrap().len(),
        2,
        "an outline that empties mid-fence is worse than no outline at all",
    );
}

#[test]
fn a_session_ends_when_the_client_says_so() {
    let mut server = opened();

    exchange(&mut server, request(2, "shutdown", Value::Null));
    exchange(&mut server, Message::notification("exit", Value::Null));

    assert!(server.should_exit());
}
