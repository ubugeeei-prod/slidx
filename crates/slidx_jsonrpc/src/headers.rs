//! `Content-Length`-framed streams, as an editor sends them.
//!
//! A header block, a blank line, and a body of exactly the declared number of
//! *bytes*. That last word is the one rule that is easy to get wrong: a deck
//! full of Japanese sends three bytes per kanji, and a reader that counted
//! characters would desynchronise the stream on the first message that
//! mentioned one — after which nothing works and nothing says why.
//!
//! Written by hand rather than taken as a dependency. The framing is three
//! lines of rules and the workspace already has `serde_json` for everything
//! below it.

use std::io::{self, BufRead, Write};

use crate::Message;

/// Reads one frame, or `None` at a clean end of stream.
///
/// Unknown headers are skipped rather than rejected: the protocol permits
/// `Content-Type`, and a client that adds one must not be able to hang the
/// server.
pub fn read(input: &mut impl BufRead) -> io::Result<Option<Message>> {
    let mut length: Option<usize> = None;

    loop {
        let mut header = String::new();
        if input.read_line(&mut header)? == 0 {
            return Ok(None);
        }

        let header = header.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }

        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = value.trim().parse().ok();
        }
    }

    let Some(length) = length else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame has no Content-Length"));
    };

    let mut body = vec![0u8; length];
    input.read_exact(&mut body)?;

    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Writes one frame and flushes it.
///
/// The flush is not optional: an editor waiting on a response it will never be
/// sent looks exactly like a server that has crashed.
pub fn write(output: &mut impl Write, message: &Message) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;

    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RequestId;
    use serde_json::{json, Value};

    fn frame(body: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
    }

    #[test]
    fn a_framed_request_is_read_back_whole() {
        let input = frame(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        let message = read(&mut input.as_slice()).unwrap().unwrap();

        assert!(message.is_request());
        assert_eq!(message.method.as_deref(), Some("initialize"));
        assert_eq!(message.id, Some(RequestId::Number(1)));
    }

    #[test]
    fn a_request_id_may_be_a_string() {
        // Some clients number requests with UUIDs, and a response that echoed
        // back a number would never be matched to its request.
        let input = frame(r#"{"jsonrpc":"2.0","id":"a-1","method":"shutdown"}"#);
        let message = read(&mut input.as_slice()).unwrap().unwrap();

        assert_eq!(message.id, Some(RequestId::Text("a-1".into())));
    }

    #[test]
    fn a_notification_has_no_id_and_expects_no_response() {
        let input = frame(r#"{"jsonrpc":"2.0","method":"exit"}"#);
        let message = read(&mut input.as_slice()).unwrap().unwrap();

        assert!(!message.is_request());
        assert_eq!(message.params(), &Value::Null);
    }

    #[test]
    fn japanese_content_is_framed_by_byte_length_not_character_count() {
        // Three bytes per kanji. A length in characters would leave the tail
        // of every message in the buffer and desynchronise the stream.
        let mut buffer = Vec::new();
        let message = Message::notification("window/logMessage", json!({ "message": "高速" }));
        write(&mut buffer, &message).unwrap();

        let text = String::from_utf8(buffer.clone()).unwrap();
        let declared: usize = text
            .split("\r\n\r\n")
            .next()
            .unwrap()
            .trim_start_matches("Content-Length:")
            .trim()
            .parse()
            .unwrap();

        assert_eq!(declared, text.split("\r\n\r\n").nth(1).unwrap().len());
        assert!(read(&mut buffer.as_slice()).unwrap().is_some(), "and it reads back");
    }

    #[test]
    fn two_frames_in_one_buffer_are_read_in_order() {
        // A fast typist's edits arrive batched in a single read.
        let mut input = frame(r#"{"jsonrpc":"2.0","method":"one"}"#);
        input.extend(frame(r#"{"jsonrpc":"2.0","method":"two"}"#));
        let mut cursor = input.as_slice();

        assert_eq!(read(&mut cursor).unwrap().unwrap().method.as_deref(), Some("one"));
        assert_eq!(read(&mut cursor).unwrap().unwrap().method.as_deref(), Some("two"));
        assert!(read(&mut cursor).unwrap().is_none(), "and then the stream ends");
    }

    #[test]
    fn an_unknown_header_is_skipped_rather_than_rejected() {
        let body = r#"{"jsonrpc":"2.0","method":"one"}"#;
        let input =
            format!("Content-Type: application/vscode-jsonrpc; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}", body.len());

        assert!(read(&mut input.as_bytes()).unwrap().is_some());
    }

    #[test]
    fn a_frame_without_a_length_is_an_error_rather_than_a_hang() {
        let input = "X-Nonsense: 1\r\n\r\n{}";
        assert!(read(&mut input.as_bytes()).is_err());
    }

    #[test]
    fn an_empty_stream_ends_cleanly() {
        assert!(read(&mut "".as_bytes()).unwrap().is_none());
    }

    #[test]
    fn a_response_survives_a_round_trip_through_the_frame() {
        let mut buffer = Vec::new();
        write(&mut buffer, &Message::response(RequestId::Number(7), json!({ "ok": true })))
            .unwrap();
        let echoed = read(&mut buffer.as_slice()).unwrap().unwrap();

        assert_eq!(echoed.id, Some(RequestId::Number(7)));
        assert_eq!(echoed.result, Some(json!({ "ok": true })));
        assert_eq!(echoed.jsonrpc, "2.0");
    }
}
