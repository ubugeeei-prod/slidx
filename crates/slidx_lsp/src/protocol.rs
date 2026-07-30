//! The base protocol: JSON-RPC 2.0 over `Content-Length`-framed streams.
//!
//! Which framing a language server uses, and nothing else. The frame and the
//! message shape live in [`slidx_jsonrpc`] because this workspace now speaks
//! JSON-RPC to two clients that disagree about the frame and about nothing
//! above it — an editor sends header blocks, `slidx mcp` sends one object per
//! line. Two readers would eventually disagree about a partial read, and the
//! symptom would be a stream that desynchronises on a message nobody can
//! reproduce.
//!
//! Kept as a module rather than folded into the imports at each call site so
//! that this file is the one place saying which framing an editor gets.

pub use slidx_jsonrpc::headers::{read, write};
pub use slidx_jsonrpc::{error_code, Message, RequestId, ResponseError};
