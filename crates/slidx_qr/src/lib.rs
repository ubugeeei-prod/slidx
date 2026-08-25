//! # slidx QR codes
//!
//! A QR encoder that turns a link into an SVG a slide can carry: the deck's own
//! URL, a resources page, the repository behind a code sample.
//!
//! ## Why this is not a dependency
//!
//! A QR encoder is a few hundred lines of arithmetic over tables that have not
//! changed since 2000. Taking one as a dependency means auditing it, tracking
//! its releases, and shipping its choices about image formats and colour
//! handling into a renderer that has opinions about both. The tables here are
//! transcribed from ISO/IEC 18004 and checked against the spec's own published
//! totals, which is the part that would otherwise need trusting.
//!
//! ## Why the output is an SVG with no colour in it
//!
//! The code is drawn in `currentColor` against a transparent background, so the
//! theme decides how it looks and the linter's contrast rules apply to it like
//! anything else on the slide. Nothing is fetched at display time: a deck is
//! presented offline, from a file, behind conference wifi, and a code that has
//! to load something is a code that is sometimes blank in front of an audience.
//!
//! ## Scope
//!
//! Byte mode, versions 1 through 10, error correction L/M/Q/H. A payload past
//! that is refused rather than encoded: version 10 is already 57 modules across,
//! and projected at slide size those modules fall below what a phone camera
//! resolves from the back of a room, so a larger symbol would scan worse rather
//! than carry more.
//!
//! ```
//! use slidx_qr::{encode, Ecc, QrOptions, SvgOptions};
//!
//! let code = encode("https://slidx.dev", &QrOptions::new(Ecc::Medium)).unwrap();
//! assert_eq!(code.size(), 4 * code.version().value() as usize + 17);
//!
//! let svg = code.to_svg(&SvgOptions::default().with_title("slidx"));
//! assert!(svg.contains("currentColor"));
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

mod bits;
mod code;
mod codewords;
mod error;
mod galois;
mod mask;
mod matrix;
mod options;
mod payload;
mod svg;
mod version;

pub use code::QrCode;
pub use error::QrError;
pub use options::{Ecc, QrOptions, SvgOptions, MIN_QUIET_ZONE};
pub use version::{Version, MAX_VERSION};

use matrix::{format, place, Matrix};

/// Encodes text as a QR code.
///
/// The version is chosen for the payload rather than requested, because the
/// smallest symbol that fits is always the most readable one: every extra
/// version puts finer modules on the wall for a camera to resolve.
///
/// # Errors
///
/// Returns [`QrError::EmptyText`] for an empty payload, and
/// [`QrError::TooLong`] when the text exceeds what version [`MAX_VERSION`]
/// holds at the requested level.
pub fn encode(text: &str, options: &QrOptions) -> Result<QrCode, QrError> {
    if text.is_empty() {
        return Err(QrError::EmptyText);
    }

    // Bytes, not characters: byte mode carries UTF-8 unchanged, so a Japanese
    // label costs three times what its length suggests.
    let payload = text.as_bytes();
    let version = version::select(payload.len(), options.ecc)?;

    let data = payload::data_codewords(payload, version, options.ecc);
    let codewords = codewords::interleave(&data, version, options.ecc);

    let mut matrix = Matrix::new(version);
    format::write_version(&mut matrix);
    place::fill(&mut matrix, &codewords);

    let (chosen, masked) = mask::apply_best(&matrix, options.ecc);

    Ok(QrCode::new(version, options.ecc, chosen, masked.into_modules()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(text: &str) -> QrCode {
        encode(text, &QrOptions::default()).unwrap()
    }

    /// Whether a 7×7 finder pattern sits with its top-left corner here.
    fn is_finder(code: &QrCode, top: usize, left: usize) -> bool {
        (0..7).all(|row| {
            (0..7).all(|column| {
                let distance = (row as isize - 3).abs().max((column as isize - 3).abs());

                code.module(top + row, left + column) == (distance != 2)
            })
        })
    }

    #[test]
    fn a_finder_pattern_survives_encoding_in_three_corners() {
        // These are what a reader looks for before anything else, and they are
        // the one part of the symbol that masking must not touch — so finding
        // them in a finished code checks the whole pipeline, not just the draw.
        for text in ["a", "https://slidx.dev", &"x".repeat(200)] {
            let code = code(text);
            let last = code.size() - 7;

            assert!(is_finder(&code, 0, 0), "top-left missing for {text:?}");
            assert!(is_finder(&code, 0, last), "top-right missing for {text:?}");
            assert!(is_finder(&code, last, 0), "bottom-left missing for {text:?}");
        }
    }

    #[test]
    fn the_fourth_corner_carries_data_so_a_reader_can_tell_the_orientation() {
        let code = code("https://slidx.dev");
        let last = code.size() - 7;

        assert!(!is_finder(&code, last, last));
    }

    #[test]
    fn the_timing_patterns_alternate_across_the_finished_symbol() {
        // Masking skips them; a mask that did not would leave the reader with
        // no module pitch to measure against.
        let code = code("https://slidx.dev/deck");

        for position in 8..code.size() - 8 {
            assert_eq!(code.module(6, position), position.is_multiple_of(2), "row 6 at {position}");
            assert_eq!(
                code.module(position, 6),
                position.is_multiple_of(2),
                "column 6 at {position}"
            );
        }
    }

    #[test]
    fn the_module_that_is_always_dark_is_dark() {
        for ecc in Ecc::ALL {
            let code = encode("slidx", &QrOptions::new(ecc)).unwrap();

            assert!(code.module(code.size() - 8, 8), "{}", ecc.as_token());
        }
    }

    #[test]
    fn a_longer_payload_selects_a_larger_version() {
        // The alternative — a fixed version — would either refuse short links
        // or waste modules on them.
        let short = code("hi");
        let medium = code(&"x".repeat(60));
        let long = code(&"x".repeat(200));

        assert!(short.version() < medium.version());
        assert!(medium.version() < long.version());
        assert_eq!(short.size(), 21);
    }

    #[test]
    fn stronger_error_correction_needs_a_larger_symbol_for_the_same_text() {
        let text = "https://slidx.dev/talks/2026/rendering";

        let low = encode(text, &QrOptions::new(Ecc::Low)).unwrap();
        let high = encode(text, &QrOptions::new(Ecc::High)).unwrap();

        assert!(low.version() <= high.version());
        assert!(low.size() <= high.size());
    }

    #[test]
    fn the_same_input_always_produces_the_same_code() {
        // Deck builds are cached and diffed. A code that varied between runs
        // would invalidate every downstream artefact for no reason, and would
        // make a rendering regression impossible to spot in a diff.
        let first = encode("https://slidx.dev", &QrOptions::new(Ecc::Quartile)).unwrap();
        let second = encode("https://slidx.dev", &QrOptions::new(Ecc::Quartile)).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.to_svg(&SvgOptions::default()), second.to_svg(&SvgOptions::default()));
    }

    #[test]
    fn japanese_text_encodes_without_panicking() {
        // Byte mode carries UTF-8 unchanged, so this needs no character set
        // handling — but it is also the case most likely to index into the
        // middle of a character and panic.
        let code = code("スライドはこちら");

        assert_eq!(code.size(), 4 * code.version().value() as usize + 17);
        assert!(is_finder(&code, 0, 0));
    }

    #[test]
    fn multi_byte_text_is_measured_in_bytes_when_choosing_a_version() {
        // Eight Japanese characters are 24 bytes and do not fit version 1 at
        // medium correction, which holds 14.
        let ascii = code("12345678");
        let japanese = code("スライドはこちら");

        assert_eq!(ascii.version().value(), 1);
        assert!(japanese.version() > ascii.version());
    }

    #[test]
    fn an_empty_string_is_refused_rather_than_encoded_to_nothing() {
        assert_eq!(encode("", &QrOptions::default()), Err(QrError::EmptyText));
    }

    #[test]
    fn text_beyond_the_largest_version_is_refused_with_the_limit_named() {
        let text = "x".repeat(300);
        let error = encode(&text, &QrOptions::new(Ecc::Medium)).unwrap_err();

        assert_eq!(error, QrError::TooLong { bytes: 300, capacity: 213, ecc: Ecc::Medium });
        assert!(error.to_string().contains("213"));
    }

    #[test]
    fn the_capacity_boundary_is_exactly_where_the_tables_say_it_is() {
        // Off-by-one here either refuses a payload that fits or accepts one
        // that silently overruns the symbol.
        for ecc in Ecc::ALL {
            let capacity = Version::new(MAX_VERSION).unwrap().byte_capacity(ecc);

            assert!(encode(&"x".repeat(capacity), &QrOptions::new(ecc)).is_ok());
            assert!(encode(&"x".repeat(capacity + 1), &QrOptions::new(ecc)).is_err());
        }
    }

    #[test]
    fn every_version_and_level_encodes_a_payload_that_fills_it() {
        // Exercises every row of the block tables, every character-count width,
        // and the mask selector against real data rather than a fixed sample.
        for version in Version::ALL {
            for ecc in Ecc::ALL {
                let capacity = version.byte_capacity(ecc);
                let code = encode(&"A".repeat(capacity), &QrOptions::new(ecc)).unwrap();

                assert_eq!(code.version(), version, "{} at {}", capacity, ecc.as_token());
                assert_eq!(code.size(), version.size());
            }
        }
    }

    #[test]
    fn a_finished_code_is_neither_mostly_dark_nor_mostly_light() {
        // What mask selection is for. A symbol far from balanced is one a
        // camera has to threshold against very little contrast.
        for text in ["a", "https://slidx.dev", &"x".repeat(200)] {
            let code = code(text);
            let dark = (0..code.size())
                .flat_map(|row| (0..code.size()).map(move |column| (row, column)))
                .filter(|&(row, column)| code.module(row, column))
                .count();
            let total = code.size() * code.size();

            assert!(
                (total * 35 / 100..total * 65 / 100).contains(&dark),
                "{text:?} produced {dark} dark modules of {total}"
            );
        }
    }

    #[test]
    fn the_svg_renders_every_dark_module_and_nothing_else() {
        // The bridge between the grid and what is actually shown: a renderer
        // that dropped or added a run would produce a code that looks right.
        let code = code("https://slidx.dev");
        let svg = code.to_svg(&SvgOptions::default());
        let runs: usize = svg.matches('M').count();

        let expected: usize = (0..code.size())
            .map(|row| {
                (0..code.size())
                    .filter(|&column| {
                        code.module(row, column) && (column == 0 || !code.module(row, column - 1))
                    })
                    .count()
            })
            .sum();

        assert_eq!(runs, expected);
    }
}
