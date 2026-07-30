//! Which revision of the Model Context Protocol this server speaks.
//!
//! One module because the version decides more than a string in one reply. It
//! decides the *shape* of every tool result for the rest of the session, and
//! the negotiation is the only moment either side gets to find out. A server
//! that assumed a revision would send fields a client has no way to read, and
//! the symptom is a tool whose answer silently arrives empty.

/// The revision this server implements.
///
/// Named rather than inferred from whatever the client sent, because a server
/// that echoed back any version at all would claim to speak revisions that had
/// not been written when it was built.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// Every revision this server will agree to, newest first.
///
/// `2025-03-26` sits between these two and is deliberately absent: it requires
/// a server to accept JSON-RPC *batches* — an array of frames answered by an
/// array of responses — and this one does not. Both revisions listed here
/// either removed that requirement or never had it, and everything this server
/// puts on the wire is spelled the same way in both.
pub const SUPPORTED: &[&str] = &[PROTOCOL_VERSION, "2024-11-05"];

/// What came of a client's `protocolVersion`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Negotiation {
    /// Agreed, at the revision the client asked for.
    Agreed(&'static str),
    /// Nothing in common. Carries what the client said, for the message.
    Unsupported(String),
}

/// Agrees a revision, or refuses.
///
/// The client's own version is echoed back when this server speaks it, rather
/// than the newest one it speaks: answering with something newer than was asked
/// for leaves a client reading fields it knows nothing about.
///
/// A client naming nothing is refused rather than defaulted. The version is
/// negotiated precisely so that neither side has to assume, and a default here
/// would move the failure to the first reply whose shape did not match.
pub fn negotiate(requested: Option<&str>) -> Negotiation {
    let requested = requested.unwrap_or_default();

    match SUPPORTED.iter().find(|supported| **supported == requested) {
        Some(agreed) => Negotiation::Agreed(agreed),
        None => Negotiation::Unsupported(requested.to_string()),
    }
}

/// True when the agreed revision has structured tool output.
///
/// A tool's `outputSchema` and a result's `structuredContent` arrived in
/// 2025-06-18. Revisions are dated, so they sort as strings — which is what
/// makes this a comparison rather than a list to maintain.
pub fn has_structured_output(version: &str) -> bool {
    version >= "2025-06-18"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_revision_a_client_asks_for_is_the_one_it_gets() {
        assert_eq!(negotiate(Some("2024-11-05")), Negotiation::Agreed("2024-11-05"));
        assert_eq!(negotiate(Some(PROTOCOL_VERSION)), Negotiation::Agreed(PROTOCOL_VERSION));
    }

    #[test]
    fn a_revision_nobody_here_speaks_carries_what_was_asked_for_into_the_refusal() {
        // So the message can name it. "Unsupported protocol version" without
        // saying which one leaves a client with nothing to fix.
        assert_eq!(negotiate(Some("1.0.0")), Negotiation::Unsupported("1.0.0".into()));
    }

    #[test]
    fn a_client_that_names_no_revision_is_refused_rather_than_defaulted() {
        // A default would move the failure to the first reply whose shape did
        // not match, which is much further from the cause.
        assert!(matches!(negotiate(None), Negotiation::Unsupported(_)));
    }

    #[test]
    fn the_newest_revision_this_server_speaks_is_the_one_it_declares() {
        assert_eq!(SUPPORTED[0], PROTOCOL_VERSION);
    }

    #[test]
    fn the_batching_revision_is_not_in_the_list() {
        // It requires a server to accept an array of frames, and this one
        // refuses one by name. Claiming it would be claiming batching.
        assert!(!SUPPORTED.contains(&"2025-03-26"));
    }

    #[test]
    fn structured_output_belongs_to_the_revision_that_introduced_it_and_no_earlier_one() {
        assert!(has_structured_output(PROTOCOL_VERSION));
        assert!(!has_structured_output("2024-11-05"));
    }
}
