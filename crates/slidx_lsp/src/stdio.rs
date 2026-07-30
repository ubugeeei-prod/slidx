//! The session loop, over standard input and standard output.
//!
//! Everything that decides anything lives in [`crate::server`] and is tested
//! there; this module owns the two things a test cannot hold, which are the
//! streams and the thread that reads one of them.
//!
//! It is a library function rather than a `main` because the server is reached
//! as `slidx lsp`. slidx ships one binary — the one the installer puts on a
//! PATH, the one the version manager pins, the one `npm i -g slidx` provides —
//! and an editor that had to find a second would be an editor that finds
//! neither on half the machines it runs on.
//!
//! # Why a reader thread
//!
//! Diagnostics are published when the input queue drains, so the loop has to be
//! able to ask whether another message is already waiting. Standard input
//! cannot answer that — a read either blocks or returns — so frames are read on
//! their own thread into a channel, and the channel can. That is the whole
//! reason the thread exists: no work happens on it beyond parsing a frame.

use std::io::{self, BufReader, Write};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use crate::protocol::{self, Message};
use crate::Server;

/// Runs a session to its end, and reports whether the client said goodbye.
///
/// False for a client that dropped the pipe without saying `exit`. It has gone
/// away and there is nobody left to report anything to, which is a different
/// ending from the one the protocol describes and worth the caller's exit code.
pub fn serve() -> bool {
    serve_on(io::stdin(), io::stdout().lock())
}

/// The same loop over any pair of streams, so a test can drive a whole session.
pub fn serve_on(input: impl io::Read + Send + 'static, mut output: impl Write) -> bool {
    let messages = spawn_reader(input);
    let mut server = Server::new();

    while let Ok(message) = messages.recv() {
        send(&mut output, &server.handle(message));

        // Drain whatever the editor has already sent before doing any
        // analysis: a fast typist's keystrokes are all in the pipe by the
        // time the first one is read, and each supersedes the last.
        loop {
            match messages.try_recv() {
                Ok(message) => send(&mut output, &server.handle(message)),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        send(&mut output, &server.flush());

        if server.should_exit() {
            break;
        }
    }

    server.should_exit()
}

/// Reads frames on their own thread so the main loop can tell an empty queue
/// from a blocked read.
fn spawn_reader(input: impl io::Read + Send + 'static) -> Receiver<Message> {
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut input = BufReader::new(input);

        loop {
            match protocol::read(&mut input) {
                Ok(Some(message)) => {
                    if sender.send(message).is_err() {
                        return;
                    }
                }
                Ok(None) => return,
                Err(error) => {
                    // A malformed frame desynchronises the stream, and every
                    // read after it is garbage. Saying so once and stopping is
                    // more useful than an endless run of parse errors.
                    let _ = writeln!(io::stderr(), "slidx lsp: {error}");
                    return;
                }
            }
        }
    });

    receiver
}

fn send(output: &mut impl Write, messages: &[Message]) {
    for message in messages {
        if let Err(error) = protocol::write(output, message) {
            let _ = writeln!(io::stderr(), "slidx lsp: {error}");
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::protocol::RequestId;

    /// Frames a run of messages the way a client writes them.
    fn wire(messages: &[Message]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for message in messages {
            protocol::write(&mut bytes, message).expect("framed");
        }
        bytes
    }

    fn request(id: i64, method: &str, params: serde_json::Value) -> Message {
        Message {
            id: Some(RequestId::Number(id)),
            method: Some(method.to_string()),
            params: Some(params),
            ..Message::default()
        }
    }

    /// Every frame the server wrote back.
    fn replies(bytes: &[u8]) -> Vec<Message> {
        let mut cursor = io::Cursor::new(bytes.to_vec());
        std::iter::from_fn(|| protocol::read(&mut cursor).ok().flatten()).collect()
    }

    #[test]
    fn a_whole_session_over_two_pipes_answers_and_then_ends() {
        // What an editor does, in order, through the streams a process really
        // gets. Nothing above this line is exercised by the unit tests, and it
        // is the half that has to work for anybody to see a diagnostic.
        let input = wire(&[
            request(1, "initialize", json!({})),
            Message::notification(
                "textDocument/didOpen",
                json!({
                    "textDocument": {
                        "uri": "file:///talks/slides/0001.md",
                        "version": 1,
                        "text": "# One\n\n![](./a.png)\n",
                    },
                }),
            ),
            request(2, "shutdown", serde_json::Value::Null),
            Message::notification("exit", serde_json::Value::Null),
        ]);

        let mut output = Vec::new();
        assert!(serve_on(io::Cursor::new(input), &mut output), "the client said exit");

        let answered = replies(&output);
        assert_eq!(answered[0].result.as_ref().unwrap()["serverInfo"]["name"], crate::SERVER_NAME);

        let published = answered
            .iter()
            .find(|message| message.method.as_deref() == Some("textDocument/publishDiagnostics"))
            .expect("diagnostics");
        assert_eq!(
            published.params.as_ref().unwrap()["diagnostics"][0]["code"],
            "structure/missing-alt"
        );
    }

    #[test]
    fn a_client_that_drops_the_pipe_without_saying_exit_is_reported_as_such() {
        // The editor was killed, or crashed. Nothing is left to report to, and
        // the process has to say that rather than exit as if it were asked to.
        let input = wire(&[request(1, "initialize", json!({}))]);

        let mut output = Vec::new();
        assert!(!serve_on(io::Cursor::new(input), &mut output));
    }

    #[test]
    fn a_malformed_frame_stops_the_session_rather_than_looping_on_garbage() {
        let mut input = b"Content-Length: nonsense\r\n\r\n{}".to_vec();
        input.extend(wire(&[request(1, "initialize", json!({}))]));

        let mut output = Vec::new();
        assert!(!serve_on(io::Cursor::new(input), &mut output));
        assert!(replies(&output).is_empty(), "and answers nothing it could not read");
    }
}
