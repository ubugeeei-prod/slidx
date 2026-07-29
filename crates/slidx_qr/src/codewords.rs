//! Splitting data into blocks, protecting each, and interleaving the result.
//!
//! The interleave is the part that is easy to skip and impossible to detect by
//! looking at the output: a symbol built from concatenated blocks is a valid
//! grid of the right size that decodes on nothing. It exists so that physical
//! damage — a thumb, a fold, a glare spot — is spread across every block
//! instead of destroying one block entirely, which is the difference between
//! correctable and lost.

use crate::galois;
use crate::options::Ecc;
use crate::version::Version;

/// Produces the symbol's codewords in the order they are placed in the grid.
pub(crate) fn interleave(data: &[u8], version: Version, ecc: Ecc) -> Vec<u8> {
    let layout = version.layout(ecc);
    let mut blocks: Vec<&[u8]> = Vec::with_capacity(layout.blocks());

    let mut offset = 0;
    for index in 0..layout.blocks() {
        let length =
            if index < layout.group1_blocks { layout.group1_data } else { layout.group2_data };
        blocks.push(&data[offset..offset + length]);
        offset += length;
    }

    let correction: Vec<Vec<u8>> =
        blocks.iter().map(|block| galois::error_correction(block, layout.ecc_per_block)).collect();

    let mut interleaved = Vec::with_capacity(layout.total_codewords());

    // Group 2's blocks are one codeword longer, so the final data column has a
    // gap where group 1's blocks have already run out.
    let longest = layout.group1_data.max(layout.group2_data);
    for column in 0..longest {
        for block in &blocks {
            if let Some(&codeword) = block.get(column) {
                interleaved.push(codeword);
            }
        }
    }

    // Every block's correction is the same length, so this half never has gaps.
    for column in 0..layout.ecc_per_block {
        for block in &correction {
            interleaved.push(block[column]);
        }
    }

    interleaved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload;

    fn codewords_for(text: &str, version: u8, ecc: Ecc) -> Vec<u8> {
        let version = Version::new(version).unwrap();
        interleave(&payload::data_codewords(text.as_bytes(), version, ecc), version, ecc)
    }

    #[test]
    fn the_symbol_receives_exactly_the_codewords_the_version_holds() {
        for version in Version::ALL {
            for ecc in Ecc::ALL {
                // Short enough to fit version 1 at high correction, which holds
                // seven bytes and is the tightest combination in the tables.
                let data = payload::data_codewords(b"slidx", version, ecc);

                assert_eq!(
                    interleave(&data, version, ecc).len(),
                    version.layout(ecc).total_codewords(),
                    "version {} at {}",
                    version.value(),
                    ecc.as_token()
                );
            }
        }
    }

    #[test]
    fn a_single_block_version_leaves_the_data_in_order() {
        // With one block there is nothing to interleave, so the data half must
        // come through untouched — the check that interleaving is not reordering
        // codewords it was only supposed to weave.
        let version = Version::new(1).unwrap();
        let data = payload::data_codewords(b"slidx", version, Ecc::Medium);
        let symbol = interleave(&data, version, Ecc::Medium);

        assert_eq!(&symbol[..data.len()], &data[..]);
    }

    #[test]
    fn multi_block_versions_take_one_codeword_from_each_block_in_turn() {
        // Version 3 at quartile is two blocks of 17. Damage confined to one
        // region must land across both blocks, not inside one.
        let version = Version::new(3).unwrap();
        let layout = version.layout(Ecc::Quartile);
        assert_eq!((layout.group1_blocks, layout.group1_data), (2, 17));

        let data: Vec<u8> = (0..34u8).collect();
        let symbol = interleave(&data, version, Ecc::Quartile);

        assert_eq!(&symbol[..6], &[0, 17, 1, 18, 2, 19]);
    }

    #[test]
    fn the_longer_group_two_block_contributes_a_final_lone_codeword() {
        // Version 5 at quartile is two blocks of 15 then two of 16. The last
        // data column holds only the two longer blocks; a naive interleave
        // either drops them or reads past the shorter blocks' ends.
        let version = Version::new(5).unwrap();
        let layout = version.layout(Ecc::Quartile);
        assert_eq!((layout.group1_data, layout.group2_data), (15, 16));

        let data: Vec<u8> = (0..62u8).collect();
        let symbol = interleave(&data, version, Ecc::Quartile);

        assert_eq!(&symbol[..4], &[0, 15, 30, 46]);
        assert_eq!(&symbol[60..62], &[45, 61], "the two longer blocks close the data half");
    }

    #[test]
    fn correction_codewords_all_follow_the_data() {
        // A reader splits the stream at this boundary by version alone, so a
        // correction codeword that leaks into the data half corrupts both.
        let version = Version::new(5).unwrap();
        let layout = version.layout(Ecc::Quartile);
        let data: Vec<u8> = (0..62u8).collect();
        let symbol = interleave(&data, version, Ecc::Quartile);

        let mut sorted_data = symbol[..layout.data_codewords()].to_vec();
        sorted_data.sort_unstable();

        assert_eq!(sorted_data, data);
    }

    #[test]
    fn interleaving_is_deterministic() {
        assert_eq!(
            codewords_for("https://slidx.dev", 4, Ecc::Medium),
            codewords_for("https://slidx.dev", 4, Ecc::Medium)
        );
    }
}
