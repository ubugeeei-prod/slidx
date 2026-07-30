//! One frame, in either direction, whichever framing carried it.
//!
//! The wire has no discriminator: which of request, response and notification
//! a frame is depends on which fields are present. So there is one shape rather
//! than three, and [`Message::is_request`] is the question a dispatcher asks.
//!
//! That question is the one it is easiest to get wrong, and both protocols
//! built on this crate say the same thing about it: a notification has no `id`
//! and **must** not be answered. A response to a notification is a frame the
//! client has no request to match it against.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request identifiers may be numbers or strings, and a response has to echo
/// back the form it was given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    Text(String),
    /// The id of a frame whose own id could not be read.
    ///
    /// Required rather than tidy: JSON-RPC says a response to an unparseable
    /// request carries a *null* id, and an omitted field is not the same thing
    /// on the wire. An incoming `"id": null` still reads as no id at all,
    /// because a frame nobody can answer is not a request.
    Null,
}

/// One JSON-RPC frame in either direction.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Message {
    #[serde(default = "jsonrpc_version")]
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
}

fn jsonrpc_version() -> String {
    "2.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: i64,
    pub message: String,
    /// Whatever the client needs to act on the failure.
    ///
    /// The case this exists for is a protocol version it does not share: the
    /// message says one is unsupported and this says which ones are, so the
    /// client can retry rather than only report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// The subset of JSON-RPC error codes the servers in this workspace produce.
pub mod error_code {
    /// A frame that was not JSON at all.
    pub const PARSE_ERROR: i64 = -32700;
    /// Well-formed JSON that is not a request this server will take: one that
    /// arrived before initialisation, or after shutdown.
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    /// The handler itself failed. Distinct from a *tool* failing, which is a
    /// successful call reporting a bad result — see `slidx mcp`.
    pub const INTERNAL_ERROR: i64 = -32603;
}

impl Message {
    pub fn notification(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: jsonrpc_version(),
            method: Some(method.into()),
            params: Some(params),
            ..Self::default()
        }
    }

    pub fn response(id: RequestId, result: Value) -> Self {
        Self { jsonrpc: jsonrpc_version(), id: Some(id), result: Some(result), ..Self::default() }
    }

    pub fn error(id: RequestId, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: jsonrpc_version(),
            id: Some(id),
            error: Some(ResponseError { code, message: message.into(), data: None }),
            ..Self::default()
        }
    }

    /// An error the client can act on rather than only report.
    pub fn error_with_data(
        id: RequestId,
        code: i64,
        message: impl Into<String>,
        data: Value,
    ) -> Self {
        let mut frame = Self::error(id, code, message);
        if let Some(error) = frame.error.as_mut() {
            error.data = Some(data);
        }

        frame
    }

    /// An error that belongs to no request.
    ///
    /// A frame that could not be parsed has no id to echo, and the protocol
    /// says to answer with a null one rather than to say nothing — silence
    /// leaves a client waiting on a response it will never get.
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self {
            jsonrpc: jsonrpc_version(),
            id: Some(RequestId::Null),
            error: Some(ResponseError {
                code: error_code::PARSE_ERROR,
                message: message.into(),
                data: None,
            }),
            ..Self::default()
        }
    }

    /// True when the frame expects a response.
    pub fn is_request(&self) -> bool {
        self.id.is_some() && self.method.is_some()
    }

    /// True when the frame must never be answered.
    ///
    /// Stated as its own question rather than as `!is_request()`, because the
    /// two are not complements: a response arriving at a server is neither.
    pub fn is_notification(&self) -> bool {
        self.id.is_none() && self.method.is_some()
    }

    /// Params as an object, or JSON null when the client sent none.
    pub fn params(&self) -> &Value {
        self.params.as_ref().unwrap_or(&Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_frame_with_an_id_and_a_method_is_a_request() {
        let message: Message =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#).unwrap();

        assert!(message.is_request());
        assert!(!message.is_notification());
    }

    #[test]
    fn a_frame_with_a_method_and_no_id_is_a_notification_and_expects_no_response() {
        let message: Message =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .unwrap();

        assert!(message.is_notification());
        assert!(!message.is_request());
        assert_eq!(message.params(), &Value::Null);
    }

    #[test]
    fn a_response_arriving_at_a_server_is_neither_a_request_nor_a_notification() {
        // Which is why the two questions are asked separately. A dispatcher
        // that treated "not a request" as "notification" would try to handle a
        // reply to something it never sent.
        let message: Message =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":4,"result":{}}"#).unwrap();

        assert!(!message.is_request());
        assert!(!message.is_notification());
    }

    #[test]
    fn a_request_id_may_be_a_string() {
        // Some clients number requests with UUIDs, and a response that echoed
        // back a number would never be matched to its request.
        let message: Message =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":"a-1","method":"ping"}"#).unwrap();

        assert_eq!(message.id, Some(RequestId::Text("a-1".into())));
    }

    #[test]
    fn a_response_carries_the_id_it_was_asked_with() {
        let frame = Message::response(RequestId::Number(7), json!({ "ok": true }));

        assert_eq!(frame.id, Some(RequestId::Number(7)));
        assert_eq!(frame.jsonrpc, "2.0");
    }

    #[test]
    fn an_error_response_omits_a_result() {
        let frame = Message::error(RequestId::Number(1), error_code::METHOD_NOT_FOUND, "no such");
        let text = serde_json::to_string(&frame).unwrap();

        assert!(!text.contains("\"result\""), "a frame may not carry both");
        assert!(text.contains("-32601"));
        assert!(!text.contains("\"data\""), "and says nothing it was not given");
    }

    #[test]
    fn an_error_can_carry_what_the_client_needs_to_retry() {
        let frame = Message::error_with_data(
            RequestId::Number(1),
            error_code::INVALID_PARAMS,
            "unsupported protocol version",
            json!({ "supported": ["2025-06-18"] }),
        );

        assert_eq!(
            frame.error.unwrap().data,
            Some(json!({ "supported": ["2025-06-18"] })),
            "a client that is only told no cannot retry"
        );
    }

    #[test]
    fn a_frame_that_could_not_be_parsed_is_answered_with_a_null_id() {
        // There is no id to echo, and silence would leave the client waiting.
        // Null rather than absent: those are different frames on the wire.
        let text = serde_json::to_string(&Message::parse_error("not json")).unwrap();

        assert!(text.contains("\"id\":null"), "{text}");
        assert!(text.contains("-32700"));
    }

    #[test]
    fn an_incoming_null_id_is_no_id_at_all() {
        // A frame nobody can answer is not a request, whichever way the client
        // spelled the absence.
        let message: Message =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#).unwrap();

        assert!(!message.is_request());
        assert!(message.is_notification());
    }
}
