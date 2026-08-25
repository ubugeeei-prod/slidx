//! A bit stream that is not byte-aligned until it has to be.
//!
//! QR fields do not respect byte boundaries — a mode indicator is 4 bits and a
//! character count is 8 or 16 — so the payload is assembled as bits and packed
//! only at the end. Doing it the other way round means hand-rolling shift
//! bookkeeping at every call site, which is where bit-stream bugs live.

/// Bits in the order they are written into the symbol, most significant first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BitBuffer {
    bits: Vec<bool>,
}

impl BitBuffer {
    pub(crate) fn with_capacity(bits: usize) -> Self {
        Self { bits: Vec::with_capacity(bits) }
    }

    /// Appends the `count` least significant bits of `value`, high bit first.
    pub(crate) fn push(&mut self, value: u32, count: usize) {
        for offset in (0..count).rev() {
            self.bits.push((value >> offset) & 1 == 1);
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.bits.len()
    }

    /// Pads with zero bits until the stream ends on a byte boundary.
    pub(crate) fn pad_to_byte(&mut self) {
        while !self.bits.len().is_multiple_of(8) {
            self.bits.push(false);
        }
    }

    /// Packs into codewords. The stream must already be byte-aligned.
    pub(crate) fn into_codewords(self) -> Vec<u8> {
        self.bits
            .chunks(8)
            .map(|chunk| chunk.iter().fold(0u8, |codeword, &bit| (codeword << 1) | u8::from(bit)))
            .collect()
    }
}

/// Reads bit `index` of a codeword stream, counting from the first bit written.
///
/// Positions past the end read as light: the last few module positions in most
/// versions are remainder bits with no codeword behind them, and the spec fills
/// them with zeroes rather than leaving them undefined.
pub(crate) fn bit_of(codewords: &[u8], index: usize) -> bool {
    match codewords.get(index / 8) {
        Some(codeword) => (codeword >> (7 - index % 8)) & 1 == 1,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_are_written_most_significant_first() {
        // The symbol is filled in this order; reversing it produces a stream no
        // reader can parse, and nothing else in the pipeline would notice.
        let mut buffer = BitBuffer::default();
        buffer.push(0b0100, 4);
        buffer.push(0b0000_0011, 8);
        buffer.pad_to_byte();

        assert_eq!(buffer.into_codewords(), vec![0b0100_0000, 0b0011_0000]);
    }

    #[test]
    fn only_the_requested_number_of_bits_is_taken() {
        let mut buffer = BitBuffer::default();
        buffer.push(0xFFFF, 4);
        buffer.pad_to_byte();

        assert_eq!(buffer.into_codewords(), vec![0b1111_0000]);
    }

    #[test]
    fn padding_stops_at_the_next_byte_boundary() {
        let mut buffer = BitBuffer::default();
        buffer.push(0, 8);
        buffer.pad_to_byte();

        assert_eq!(buffer.len(), 8, "an aligned stream must not gain a whole empty byte");
    }

    #[test]
    fn a_bit_index_past_the_stream_reads_light() {
        // Remainder bits have no codeword behind them and the spec fills them
        // with zeroes; reading them as dark corrupts the last modules placed.
        assert!(bit_of(&[0b1000_0000], 0));
        assert!(!bit_of(&[0b1000_0000], 1));
        assert!(!bit_of(&[0b1000_0000], 900));
        assert!(!bit_of(&[], 0));
    }
}
