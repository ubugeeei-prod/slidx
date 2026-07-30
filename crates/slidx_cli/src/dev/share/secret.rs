//! The bytes a share link is guarded by.
//!
//! A share URL is a capability: whoever has it can read an unreleased talk off
//! a filesystem-backed dev server, and with the second secret can rewrite it.
//! Nothing rate-limits a guess and nothing can attribute one, so the only thing
//! standing between the link and a stranger on the same network is that the
//! secret is unguessable.
//!
//! ## There is no fallback, on purpose
//!
//! `packages/runtime/src/remote.ts` refuses to fall back to `Math.random()` for
//! the phone remote's pairing, and says why: a secret drawn from a predictable
//! source looks exactly like a real one and protects nothing. This is the same
//! rule on the Rust side. When the platform has no cryptographic randomness, the
//! answer is that sharing does not happen — not a weaker secret nobody can see
//! is weaker.
//!
//! ## Why `getrandom` rather than reading `/dev/urandom`
//!
//! Every other dependency in this binary was argued down to nothing, and this
//! one is not an exception to that so much as a case where writing it by hand is
//! the worse option. Unix is a file read; Windows is `BCryptGenRandom` behind
//! `unsafe extern`; WASI is a third thing again. That is exactly the code nobody
//! should hand-roll and exactly the code `getrandom` is, and it was already in
//! this workspace's lockfile, so the supply chain does not grow either. It
//! compiles to a syscall wrapper — a few hundred bytes in the binary, and no
//! start-up cost at all, because nothing calls it unless `--crdt` is passed.

/// Bytes of session identifier. Only has to be unique among live sessions.
///
/// The same eight `remote.ts` uses, because it is the same kind of name: it
/// appears in the URL, it is not secret, and two talks in one building must not
/// collide.
pub const SESSION_BYTES: usize = 8;

/// Bytes of secret.
///
/// Sixteen, matching `SECRET_BYTES` in `packages/runtime/src/remote.ts`, and for
/// the same reason: this is guarded by nothing but its own length, and the URL
/// is on a screen in a room full of people with cameras.
pub const SECRET_BYTES: usize = 16;

/// The platform has no cryptographic randomness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoRandomness;

impl NoRandomness {
    /// What to tell somebody who asked to share and cannot.
    ///
    /// Says what will not happen rather than what went wrong, because the thing
    /// they were about to do is the thing they need to hear about.
    pub fn message(self) -> String {
        "This machine has no cryptographic randomness, so slidx will not mint a share \
         secret.\n\n\
         A secret drawn from a predictable source looks exactly like a real one and \
         protects nothing, and the deck being shared is an unreleased talk on a server that \
         can write your files. So there is no weaker option here.\n\n\
         `slidx dev` without --crdt still works, on loopback.\n"
            .to_string()
    }
}

/// A lowercase hex token of `bytes` bytes.
pub fn token(bytes: usize) -> Result<String, NoRandomness> {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer).map_err(|_| NoRandomness)?;

    Ok(buffer.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_is_lowercase_hex_of_the_length_the_reader_expects() {
        // `readPairing` accepts lowercase hex and nothing else, so a token in
        // any other alphabet would be refused by the editor it was minted for.
        let secret = token(SECRET_BYTES).expect("randomness");

        assert_eq!(secret.len(), SECRET_BYTES * 2);
        assert!(secret.chars().all(|character| character.is_ascii_hexdigit()));
        assert_eq!(secret, secret.to_lowercase());
    }

    #[test]
    fn two_secrets_minted_in_a_row_are_not_the_same() {
        // A weak generator that returned a counter or a clock would pass every
        // other test here.
        let first = token(SECRET_BYTES).expect("randomness");
        let second = token(SECRET_BYTES).expect("randomness");

        assert_ne!(first, second);
    }

    #[test]
    fn the_secret_is_the_same_length_the_phone_remote_uses() {
        // One answer in the repository. `SECRET_BYTES` in
        // packages/runtime/src/remote.ts is sixteen for the same reason.
        assert_eq!(SECRET_BYTES, 16);
        assert_eq!(SESSION_BYTES, 8);
    }

    #[test]
    fn a_machine_with_no_randomness_is_told_what_will_not_happen() {
        let message = NoRandomness.message();

        assert!(message.contains("will not mint"), "{message}");
        assert!(message.contains("without --crdt still works"), "{message}");
    }
}
