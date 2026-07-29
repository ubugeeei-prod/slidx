//! Turning text into the version's data codewords.
//!
//! Byte mode only. The alternatives — numeric, alphanumeric, kanji — pack
//! denser, but only for payloads drawn from their restricted alphabets, and a
//! URL is not one of them: a single lowercase letter disqualifies alphanumeric
//! mode. Supporting modes that a deck's links can never use would add a segment
//! optimiser and a second set of capacity tables to maintain for no gain.
//!
//! Byte mode also carries UTF-8 unchanged, which is what makes a Japanese label
//! encode without the crate knowing anything about character sets.

use crate::bits::BitBuffer;
use crate::options::Ecc;
use crate::version::Version;

/// The four-bit mode indicator for byte mode.
const BYTE_MODE: u32 = 0b0100;

/// Pad codewords, alternated to fill the tail of an under-full symbol.
///
/// Two alternating values rather than zeroes because a long run of identical
/// codewords produces large uniform regions, which the mask penalty rules
/// punish and readers find harder to lock onto.
const PAD_CODEWORDS: [u8; 2] = [0xEC, 0x11];

/// Encodes `text` as this version's full complement of data codewords.
///
/// The caller has already established that the payload fits, so this cannot
/// fail: everything below is padding a known-short stream up to a known length.
pub(crate) fn data_codewords(text: &[u8], version: Version, ecc: Ecc) -> Vec<u8> {
    let capacity = version.layout(ecc).data_codewords();
    let mut bits = BitBuffer::with_capacity(capacity * 8);

    bits.push(BYTE_MODE, 4);
    bits.push(text.len() as u32, version.character_count_bits());
    for &byte in text {
        bits.push(u32::from(byte), 8);
    }

    // The terminator is up to four zero bits, truncated when the symbol has
    // fewer than four left — a full symbol ends without one.
    let remaining = capacity * 8 - bits.len();
    bits.push(0, remaining.min(4));
    bits.pad_to_byte();

    let mut codewords = bits.into_codewords();
    for index in 0..capacity.saturating_sub(codewords.len()) {
        codewords.push(PAD_CODEWORDS[index % PAD_CODEWORDS.len()]);
    }

    codewords
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(text: &str, version: u8, ecc: Ecc) -> Vec<u8> {
        data_codewords(text.as_bytes(), Version::new(version).unwrap(), ecc)
    }

    #[test]
    fn the_stream_opens_with_the_byte_mode_indicator_and_the_length() {
        // Mode 0100, then an 8-bit count of 1, then 'A' — each field straddling
        // a byte boundary, which is exactly where a shift error would show.
        let codewords = encode("A", 1, Ecc::Medium);

        assert_eq!(codewords[0], 0b0100_0000);
        assert_eq!(codewords[1], 0b0001_0100, "count 1 high nibble, then 'A' high nibble");
        assert_eq!(codewords[2], 0b0001_0000, "'A' low nibble, then the terminator");
    }

    #[test]
    fn the_symbol_is_always_filled_to_its_full_data_capacity() {
        // A short stream left short would place fewer codewords than the matrix
        // has room for, and the trailing modules would decode as garbage.
        for version in Version::ALL {
            for ecc in Ecc::ALL {
                let codewords = data_codewords(b"slidx", version, ecc);

                assert_eq!(
                    codewords.len(),
                    version.layout(ecc).data_codewords(),
                    "version {} at {}",
                    version.value(),
                    ecc.as_token()
                );
            }
        }
    }

    #[test]
    fn the_tail_is_filled_with_the_two_alternating_pad_codewords() {
        let codewords = encode("A", 1, Ecc::Medium);

        assert_eq!(
            codewords[3..],
            [0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC]
        );
    }

    #[test]
    fn a_payload_that_exactly_fills_the_symbol_gets_no_padding() {
        // There is no room for a terminator here either; emitting one anyway
        // would push the stream past capacity.
        let version = Version::new(1).unwrap();
        let text = vec![b'x'; version.byte_capacity(Ecc::Low)];
        let codewords = data_codewords(&text, version, Ecc::Low);

        assert_eq!(codewords.len(), version.layout(Ecc::Low).data_codewords());
        assert_ne!(codewords[codewords.len() - 1], 0xEC);
    }

    #[test]
    fn multi_byte_characters_are_counted_in_bytes_not_characters() {
        // The character count field counts UTF-8 bytes. Counting characters
        // would understate it and truncate the payload for every reader.
        let codewords = encode("あ", 1, Ecc::Low);

        assert_eq!(codewords[0] & 0b1111_0000, 0b0100_0000);
        let length = ((codewords[0] & 0x0F) << 4) | (codewords[1] >> 4);
        assert_eq!(length, 3, "one Japanese character is three UTF-8 bytes");
    }

    #[test]
    fn version_ten_writes_a_sixteen_bit_length_field() {
        // The wider field shifts every payload byte by another byte; a reader
        // that disagrees about the width decodes noise.
        let codewords = encode("AB", 10, Ecc::Low);

        assert_eq!(codewords[0], 0b0100_0000, "mode, then the top four count bits");
        assert_eq!(codewords[1], 0b0000_0000, "count bits 4..12, all zero for a length of 2");
        assert_eq!(codewords[2], 0b0010_0100, "the last count bits, then 'A' begins a nibble late");
        assert_eq!(codewords[3], 0b0001_0100, "'A' finishes, 'B' begins");
    }
}
