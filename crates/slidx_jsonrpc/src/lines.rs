//! Newline-delimited framing, as an agent's client sends it over stdio.
//!
//! One JSON object per line, and the newline *is* the frame. So the rule that
//! matters here is the mirror of the header framing's: a message must not
//! contain an embedded newline. It is written compactly and followed by exactly
//! one `\n`, because a pretty-printed response would be read as several
//! truncated frames.
//!
//! ## A bad line is recoverable, and that is the difference
//!
//! A malformed `Content-Length` frame desynchronises the stream: the reader no
//! longer knows where the next body begins, and every read after it is
//! garbage. Here the next newline is the next frame whatever went wrong before
//! it, so a line that is not JSON is reported to the client as a parse error
//! and the session continues. [`Frame`] exists to say which of those happened,
//! rather than collapsing both into one error the caller cannot tell apart.

use std::io::{self, BufRead, Write};

use crate::Message;

/// What one line of the stream turned out to be.
#[derive(Debug)]
pub enum Frame {
    Message(Message),
    /// A line that was not a JSON-RPC frame. Carries what to tell the client.
    Malformed(String),
    /// An array of frames.
    ///
    /// A batch, which JSON-RPC 2.0 permits and this crate's callers do not
    /// implement. Named rather than reported as a syntax error, because the
    /// client's JSON was fine and "parse error" would send somebody looking for
    /// a bug in their serialiser.
    Batch,
    /// The client hung up.
    End,
}

/// Reads one line, skipping blank ones.
///
/// A blank line is a client that flushed an extra newline, and treating it as
/// end of stream would drop a session on a stray byte.
pub fn read(input: &mut impl BufRead) -> io::Result<Frame> {
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(Frame::End);
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') {
            return Ok(Frame::Batch);
        }

        return Ok(match serde_json::from_str::<Message>(line) {
            Ok(message) => Frame::Message(message),
            Err(error) => Frame::Malformed(error.to_string()),
        });
    }
}

/// Writes one frame, one line, and flushes it.
///
/// The flush is not optional: a client waiting on a response it will never be
/// sent looks exactly like a server that has crashed.
pub fn write(output: &mut impl Write, message: &Message) -> io::Result<()> {
    // Compact rather than pretty, because the frame is the newline. This is the
    // one place that decides it, so nothing downstream can undo it.
    let body = serde_json::to_string(message)?;

    debug_assert!(!body.contains('\n'), "a frame may not contain the byte that ends it: {body}");

    output.write_all(body.as_bytes())?;
    output.write_all(b"\n")?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RequestId;
    use serde_json::json;

    fn read_one(text: &str) -> Frame {
        read(&mut text.as_bytes()).unwrap()
    }

    #[test]
    fn one_object_on_one_line_is_one_frame() {
        let frame = read_one("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n");

        let Frame::Message(message) = frame else { panic!("expected a message, got {frame:?}") };
        assert!(message.is_request());
        assert_eq!(message.method.as_deref(), Some("initialize"));
    }

    #[test]
    fn two_frames_in_one_buffer_are_read_in_order() {
        let text =
            "{\"jsonrpc\":\"2.0\",\"method\":\"one\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"two\"}\n";
        let mut cursor = text.as_bytes();

        let mut methods = Vec::new();
        while let Frame::Message(message) = read(&mut cursor).unwrap() {
            methods.push(message.method.unwrap_or_default());
        }

        assert_eq!(methods, ["one", "two"]);
    }

    #[test]
    fn a_blank_line_is_not_the_end_of_the_stream() {
        // A client that flushed an extra newline. Dropping the session on a
        // stray byte would look exactly like a crash.
        let frame = read_one("\n\n{\"jsonrpc\":\"2.0\",\"method\":\"ping\"}\n");

        assert!(matches!(frame, Frame::Message(_)), "{frame:?}");
    }

    #[test]
    fn a_line_that_is_not_json_is_reported_rather_than_desynchronising_the_stream() {
        // The whole reason this framing is easier to live with than the
        // header one: the next newline is the next frame regardless.
        let mut cursor = "not json at all\n{\"jsonrpc\":\"2.0\",\"method\":\"ping\"}\n".as_bytes();

        assert!(matches!(read(&mut cursor).unwrap(), Frame::Malformed(_)));
        assert!(matches!(read(&mut cursor).unwrap(), Frame::Message(_)), "and the session goes on");
    }

    #[test]
    fn a_batch_is_named_rather_than_called_a_syntax_error() {
        // The client's JSON was fine. Reporting a parse error would send
        // somebody looking for a bug in their serialiser.
        let frame = read_one("[{\"jsonrpc\":\"2.0\",\"method\":\"ping\"}]\n");

        assert!(matches!(frame, Frame::Batch), "{frame:?}");
    }

    #[test]
    fn an_empty_stream_ends_cleanly() {
        assert!(matches!(read_one(""), Frame::End));
    }

    #[test]
    fn a_last_line_without_a_newline_is_still_a_frame() {
        // A client that closed the pipe immediately after writing.
        assert!(matches!(read_one("{\"jsonrpc\":\"2.0\",\"method\":\"ping\"}"), Frame::Message(_)));
    }

    #[test]
    fn a_written_frame_occupies_exactly_one_line() {
        // Two would be read as two frames, the second of them truncated.
        let mut wire = Vec::new();
        let result = json!({ "instructions": "line one\nline two", "nested": { "a": [1, 2] } });
        write(&mut wire, &Message::response(RequestId::Number(1), result)).unwrap();

        let text = String::from_utf8(wire).unwrap();
        assert_eq!(text.matches('\n').count(), 1, "{text}");
        assert!(text.ends_with('\n'));
        assert!(text.contains(r"line one\nline two"), "escaped, not embedded: {text}");
    }

    #[test]
    fn japanese_content_survives_the_round_trip() {
        // Nothing here counts bytes, but the encoding still has to be UTF-8
        // end to end: a deck's notes are the payload this carries.
        let mut wire = Vec::new();
        write(
            &mut wire,
            &Message::response(RequestId::Number(1), json!({ "title": "高速なデッキ" })),
        )
        .unwrap();

        let Frame::Message(echoed) = read(&mut wire.as_slice()).unwrap() else { panic!("a frame") };
        assert_eq!(echoed.result.unwrap()["title"], "高速なデッキ");
    }
}
