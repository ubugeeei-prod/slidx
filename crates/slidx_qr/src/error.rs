//! Why an encode refused.
//!
//! Encoding is arithmetic over a fixed set of tables, so every failure is a
//! capacity question answered before any of that arithmetic runs. Making them
//! values rather than panics matters because the caller is a build step: a deck
//! with one over-long link should report that line, not abort the build.

use std::fmt;

use crate::options::Ecc;
use crate::version::MAX_VERSION;

/// A refusal to encode, with the numbers needed to act on it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QrError {
    /// There was nothing to encode.
    ///
    /// A zero-length payload is representable, and readers do decode it — to
    /// nothing. That is never what an author meant by putting a code on a
    /// slide, so it is reported instead of rendered.
    EmptyText,

    /// The payload does not fit any supported version at this level.
    TooLong {
        /// Length of the payload in bytes — not characters, which is the
        /// distinction that surprises authors pasting non-ASCII text.
        bytes: usize,
        /// The largest payload the largest supported version holds here.
        capacity: usize,
        ecc: Ecc,
    },
}

impl fmt::Display for QrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText => formatter.write_str("cannot encode an empty string as a QR code"),
            Self::TooLong { bytes, capacity, ecc } => write!(
                formatter,
                "{bytes} bytes is too long for a QR code: version {MAX_VERSION} at error \
                 correction `{}` holds {capacity} bytes",
                ecc.as_token()
            ),
        }
    }
}

impl std::error::Error for QrError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_too_long_payload_names_the_limit_it_exceeded() {
        // The author's next action is to shorten the link or lower the level,
        // and neither is obvious without both numbers in the message.
        let message = QrError::TooLong { bytes: 500, capacity: 213, ecc: Ecc::Medium }.to_string();

        assert!(message.contains("500 bytes"), "{message}");
        assert!(message.contains("213 bytes"), "{message}");
        assert!(message.contains("medium"), "{message}");
    }

    #[test]
    fn the_empty_text_message_says_what_was_wrong_with_the_input() {
        assert!(QrError::EmptyText.to_string().contains("empty"));
    }

    #[test]
    fn errors_are_usable_as_std_errors() {
        // Callers box this into their own error type; without the impl they
        // cannot, and the crate leaks into every signature above it.
        let error: Box<dyn std::error::Error> = Box::new(QrError::EmptyText);

        assert!(!error.to_string().is_empty());
    }
}
