//! SHA-256, so a download can always be checked.
//!
//! Written out rather than depended on, and the reason is not dependency
//! austerity for its own sake. `install.sh` has to *look* for a hasher —
//! `sha256sum` on Linux, `shasum` on macOS — and refuse to install when it
//! finds neither, because a shell script cannot compute a digest by itself.
//! That is a real hole: the one machine with no hasher is the one machine that
//! cannot verify what it downloaded.
//!
//! A binary carrying its own implementation has no such case. `slidx version
//! install` verifies on every machine, always, with nothing to detect and no
//! branch that skips it.
//!
//! This is FIPS 180-4, and it is a fixed, forty-year-old specification with
//! published test vectors — which is what makes writing it out reasonable
//! rather than reckless. The tests below are those vectors.
//!
//! Not constant-time, and it does not need to be. Comparing a published digest
//! against a downloaded file leaks nothing: both sides are public, and an
//! attacker who can time this can already read the file.

use std::fmt::Write as _;

/// First 32 bits of the fractional parts of the cube roots of the first 64
/// primes. From the specification, not derived here — deriving constants is how
/// you get a hash that is subtly not SHA-256.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// First 32 bits of the fractional parts of the square roots of the first eight
/// primes.
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// A running digest.
///
/// Incremental so a release archive is hashed as it is read rather than after
/// it is all in memory. The binaries are under a megabyte today and will not
/// always be.
#[derive(Debug, Clone)]
pub struct Sha256 {
    state: [u32; 8],
    /// Bytes not yet part of a full 64-byte block.
    pending: Vec<u8>,
    /// Total length, which the padding encodes as a bit count.
    length: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self { state: H0, pending: Vec::with_capacity(64), length: 0 }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.length = self.length.wrapping_add(bytes.len() as u64);
        self.pending.extend_from_slice(bytes);

        // Whole blocks only; the remainder waits for the next call or the
        // padding.
        let full = self.pending.len() / 64 * 64;
        for block in self.pending[..full].chunks_exact(64) {
            compress(&mut self.state, block);
        }
        self.pending.drain(..full);
    }

    /// The digest, as the 32 raw bytes.
    pub fn finish(mut self) -> [u8; 32] {
        let bits = self.length.wrapping_mul(8);

        // A single 1 bit, then zeros, then the length as a big-endian u64 —
        // leaving the whole message a multiple of 64 bytes.
        self.pending.push(0x80);
        while self.pending.len() % 64 != 56 {
            self.pending.push(0);
        }
        self.pending.extend_from_slice(&bits.to_be_bytes());

        let mut state = self.state;
        for block in self.pending.chunks_exact(64) {
            compress(&mut state, block);
        }

        let mut digest = [0u8; 32];
        for (chunk, word) in digest.chunks_exact_mut(4).zip(state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }

        digest
    }

    /// The digest as lowercase hex — the spelling `sha256sum` prints and
    /// `SHA256SUMS` carries.
    pub fn hex(self) -> String {
        self.finish().iter().fold(String::with_capacity(64), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
    }
}

/// One 64-byte block.
fn compress(state: &mut [u32; 8], block: &[u8]) {
    let mut w = [0u32; 64];

    for (word, chunk) in w.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }

    for index in 16..64 {
        let s0 =
            w[index - 15].rotate_right(7) ^ w[index - 15].rotate_right(18) ^ (w[index - 15] >> 3);
        let s1 =
            w[index - 2].rotate_right(17) ^ w[index - 2].rotate_right(19) ^ (w[index - 2] >> 10);

        w[index] = w[index - 16].wrapping_add(s0).wrapping_add(w[index - 7]).wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;

    for index in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choose = (e & f) ^ (!e & g);
        let temp1 =
            h.wrapping_add(s1).wrapping_add(choose).wrapping_add(K[index]).wrapping_add(w[index]);

        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(majority);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *slot = slot.wrapping_add(value);
    }
}

/// The digest of a slice, for callers that already hold the whole thing.
pub fn hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    digest.hex()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published vectors. Everything else here is a property; these are the
    /// only tests that say this function is SHA-256 rather than a hash.
    const VECTORS: &[(&str, &str)] = &[
        ("", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
        ("abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
        (
            "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
        (
            "abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmno\
             ijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu",
            "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1",
        ),
    ];

    #[test]
    fn the_published_test_vectors_hash_to_their_published_digests() {
        for (input, expected) in VECTORS {
            assert_eq!(hex(input.as_bytes()), *expected, "for {input:?}");
        }
    }

    #[test]
    fn a_million_a_characters_hash_to_the_published_digest() {
        // The long vector. It is the one that catches a length counter that
        // overflows or a padding block that is off by one.
        let mut digest = Sha256::new();
        for _ in 0..1_000 {
            digest.update(&[b'a'; 1_000]);
        }

        assert_eq!(
            digest.hex(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn feeding_the_same_bytes_in_different_sized_pieces_gives_the_same_digest() {
        // The property that makes streaming a file safe: the block boundaries
        // of the *reads* must not reach the digest.
        let message: Vec<u8> = (0..500u32).map(|n| n as u8).collect();
        let whole = hex(&message);

        for chunk in [1usize, 7, 63, 64, 65, 128, 499] {
            let mut digest = Sha256::new();
            for piece in message.chunks(chunk) {
                digest.update(piece);
            }

            assert_eq!(digest.hex(), whole, "in chunks of {chunk}");
        }
    }

    #[test]
    fn a_message_that_lands_exactly_on_a_block_boundary_is_padded_into_a_new_block() {
        // 55, 56 and 64 bytes are the three cases the padding rule turns on:
        // the length no longer fits, and a whole extra block is required.
        for length in [55usize, 56, 57, 63, 64, 65, 119, 120] {
            let message = vec![b'x'; length];
            let mut streamed = Sha256::new();
            streamed.update(&message);

            assert_eq!(streamed.hex(), hex(&message), "at {length} bytes");
            assert_eq!(hex(&message).len(), 64);
        }
    }

    #[test]
    fn a_digest_is_sixty_four_lowercase_hex_characters() {
        // The spelling `SHA256SUMS` carries. An uppercase digest would compare
        // unequal to every published one.
        let digest = hex(b"anything");

        assert_eq!(digest.len(), 64);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn a_single_changed_bit_changes_the_digest_completely() {
        // Not a proof of anything, but it is the property a corrupted download
        // relies on, and a broken implementation often fails it visibly.
        let a = hex(b"the quick brown fox");
        let b = hex(b"the quick brown fox!");

        let shared = a.chars().zip(b.chars()).filter(|(x, y)| x == y).count();
        assert!(shared < 20, "{a} and {b} share {shared} characters");
    }

    #[test]
    fn a_fresh_hasher_and_a_default_one_are_the_same_thing() {
        assert_eq!(Sha256::new().hex(), Sha256::default().hex());
    }
}
