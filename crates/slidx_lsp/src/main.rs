//! `slidx-lsp` — the language server, speaking the base protocol over stdio.
//!
//! The binary is deliberately thin. Everything that decides anything lives in
//! the library and is tested there; this file owns the two things a test
//! cannot hold, which are the streams and the thread that reads one of them.
//!
//! # Why a reader thread
//!
//! Diagnostics are published when the input queue drains, so the loop has to
//! be able to ask whether another message is already waiting. Standard input
//! cannot answer that — a read either blocks or returns — so frames are read
//! on their own thread into a channel, and the channel can. That is the whole
//! reason the thread exists: no work happens on it beyond parsing a frame.

use std::io::{self, BufReader, Write};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use slidx_lsp::protocol::{self, Message};
use slidx_lsp::Server;

fn main() {
    let messages = spawn_reader();
    let mut server = Server::new();
    let mut output = io::stdout().lock();

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

    // A client that dropped the pipe without saying `exit` has gone away, and
    // there is nobody left to report anything to.
    if !server.should_exit() {
        std::process::exit(1);
    }
}

/// Reads frames on their own thread so the main loop can tell an empty queue
/// from a blocked read.
fn spawn_reader() -> Receiver<Message> {
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut input = BufReader::new(io::stdin());

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
                    let _ = writeln!(io::stderr(), "slidx-lsp: {error}");
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
            let _ = writeln!(io::stderr(), "slidx-lsp: {error}");
            return;
        }
    }
}
