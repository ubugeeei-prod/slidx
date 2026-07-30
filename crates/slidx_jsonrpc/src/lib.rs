//! # slidx jsonrpc
//!
//! JSON-RPC 2.0 over stdio, framed two ways.
//!
//! ## Why one crate and two framings
//!
//! This workspace speaks JSON-RPC to two clients over standard input and
//! output, and they disagree about the frame and about nothing else. An editor
//! sends a header block and a body of exactly the declared number of bytes
//! ([`headers`]). An agent sends one JSON object per line ([`lines`]).
//! Everything above the frame — what a request is, which field decides whether
//! a frame expects an answer, the error codes — is the same.
//!
//! So [`Message`] is declared once. A second reader would eventually disagree
//! with the first about a partial read, and the failure would be a stream that
//! desynchronises on a message nobody can reproduce.
//!
//! ## What is not here
//!
//! No dispatch, no lifecycle, no session. Both servers are pure functions from
//! a message to the messages they answer with, and neither of them needs a
//! framework to be one.
//!
//! ```
//! use slidx_jsonrpc::{lines, Message, RequestId};
//!
//! let mut wire = Vec::new();
//! lines::write(&mut wire, &Message::response(RequestId::Number(1), serde_json::json!({}))).unwrap();
//!
//! // One line, and the newline is the frame.
//! assert_eq!(wire.iter().filter(|byte| **byte == b'\n').count(), 1);
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod headers;
pub mod lines;
pub mod message;

pub use message::{error_code, Message, RequestId, ResponseError};
