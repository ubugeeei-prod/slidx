//! A share link as a code somebody can point a phone at.
//!
//! A co-presenter scanning a code beats reading an IP address and a
//! thirty-two-character secret off somebody else's laptop, out loud, in a room.
//! The encoder is [`slidx_qr`], which is already here for the codes a deck puts
//! on a slide — this module only decides how a matrix of modules becomes rows of
//! text.
//!
//! ## Why this is the one place that does not use `Ink`
//!
//! A camera needs a code dark-on-light, and a terminal is usually light-on-dark.
//! Drawn in the terminal's own colours the code would be inverted, and a reader
//! that refuses an inverted code is indistinguishable from a code that is wrong.
//! So the block sets black on white itself and restores nothing else —
//! `slidx_render`'s slide codes carry their own light background for exactly the
//! same reason, whatever the deck's theme says.
//!
//! It follows that with colour switched off there is no code, only the URL. That
//! is stated rather than worked around: a code that does not scan and a missing
//! code are the same to somebody holding a phone, and one of them is honest.
//!
//! ## Two module rows per text row
//!
//! A terminal cell is about twice as tall as it is wide, so one module per cell
//! would draw a code stretched to twice its height and half its width — which
//! readers do not like and which would not fit in eighty columns anyway. Half
//! blocks put two module rows in one cell and keep the code square.

use slidx_qr::{encode, Ecc, QrOptions, MIN_QUIET_ZONE};

use crate::style::Style;

/// Error correction for a code on a screen an arm's length away.
///
/// Medium rather than the Quartile a *projected* code gets. Nothing is going to
/// obscure a quarter of this one, and every level up is a larger symbol — which
/// on a terminal means more columns rather than more redundancy anybody needs.
const ECC: Ecc = Ecc::Medium;

/// Black foreground on a white background, and then off.
///
/// Written here rather than taken from [`crate::style::Ink`] because it is not a
/// meaning, it is a physical requirement of a camera. See the module docs.
const DARK_ON_LIGHT: &str = "\u{1b}[30;107m";
const RESET: &str = "\u{1b}[0m";

/// The quiet zone, in the units this module counts in.
///
/// `slidx_qr` states it as a `u32` because an SVG's own numbers are; here it is
/// an index into a grid, and one conversion at the boundary beats a cast at each
/// of the four places it is used.
const QUIET: usize = MIN_QUIET_ZONE as usize;

/// The code for a link, as lines of text.
///
/// Empty when colour is off, or when the link is longer than a readable symbol
/// can carry. Both mean the same thing to a caller: print the URL and no code.
pub fn block(link: &str, indent: usize, style: &Style) -> String {
    if !style.is_colored() {
        return String::new();
    }

    let Ok(code) = encode(link, &QrOptions::new(ECC)) else {
        return String::new();
    };

    let span = code.size() + QUIET * 2;
    let margin = " ".repeat(indent);
    let mut text = String::new();

    // The quiet zone is drawn by walking past the edge of the matrix, which
    // reads light — `QrCode::module` answers that rather than panicking, so the
    // margin needs no special case.
    for top in (0..span).step_by(2) {
        text.push_str(&margin);
        text.push_str(DARK_ON_LIGHT);

        for column in 0..span {
            text.push(glyph(
                dark(&code, top, column),
                // An odd number of rows leaves the last cell's lower half
                // outside the symbol, which is quiet zone and therefore light.
                dark(&code, top + 1, column),
            ));
        }

        text.push_str(RESET);
        text.push('\n');
    }

    text
}

/// Whether a module inside the quiet-zone-padded grid is dark.
fn dark(code: &slidx_qr::QrCode, row: usize, column: usize) -> bool {
    let Some(row) = row.checked_sub(QUIET) else { return false };
    let Some(column) = column.checked_sub(QUIET) else { return false };

    code.module(row, column)
}

/// One cell, standing for two stacked modules.
fn glyph(top: bool, bottom: bool) -> char {
    match (top, bottom) {
        (true, true) => '\u{2588}',
        (true, false) => '\u{2580}',
        (false, true) => '\u{2584}',
        (false, false) => ' ',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: &str = "http://192.168.1.42:5173/__slidx/#s=0123456789abcdef.\
                        00112233445566778899aabbccddeeff";

    fn lines(text: &str) -> Vec<&str> {
        text.lines().collect()
    }

    /// Columns a line occupies on screen, escape sequences not counting.
    fn visible(line: &str) -> usize {
        line.replace(DARK_ON_LIGHT, "").replace(RESET, "").chars().count()
    }

    #[test]
    fn a_share_link_becomes_a_code_that_fits_in_a_terminal() {
        // Eighty columns is what `style::WIDTH` promises and what a paste into
        // an issue or a chat window survives.
        let text = block(LINK, 2, &Style::colored());

        assert!(!text.is_empty());
        for line in lines(&text) {
            assert!(visible(line) <= 80, "{} columns", visible(line));
        }
    }

    #[test]
    fn nothing_is_drawn_when_colour_is_off_rather_than_a_code_that_will_not_scan() {
        // A terminal is usually light-on-dark, so an uncoloured code is an
        // inverted one — and a reader that refuses it looks exactly like a code
        // that is wrong.
        assert_eq!(block(LINK, 2, &Style::plain()), "");
    }

    #[test]
    fn every_row_forces_dark_on_light_because_that_is_what_a_camera_needs() {
        for line in lines(&block(LINK, 2, &Style::colored())) {
            assert!(line.contains(DARK_ON_LIGHT), "{line}");
            assert!(line.ends_with(RESET), "{line}");
        }
    }

    #[test]
    fn the_code_is_square_within_one_row_because_two_modules_share_a_cell() {
        // One module per cell would draw a code twice as tall as it is wide,
        // which readers do not like.
        let text = block(LINK, 0, &Style::colored());
        let rows = lines(&text).len();
        let columns = visible(lines(&text)[0]);

        assert!(rows * 2 >= columns && rows * 2 <= columns + 1, "{rows} rows, {columns} columns");
    }

    #[test]
    fn a_quiet_zone_surrounds_the_symbol_so_a_reader_can_find_its_edge() {
        // A code drawn flush against terminal text does not scan. The first row
        // is entirely quiet zone.
        let text = block(LINK, 0, &Style::colored());
        let first = lines(&text)[0].replace(DARK_ON_LIGHT, "").replace(RESET, "");

        assert!(first.chars().all(|character| character == ' '), "{first:?}");
    }

    #[test]
    fn a_link_too_long_for_a_readable_symbol_is_no_code_rather_than_a_bad_one() {
        // The encoder refuses past version 10, and a caller that got half a
        // code would put an unscannable square on somebody's screen.
        assert_eq!(block(&"x".repeat(4000), 2, &Style::colored()), "");
    }

    #[test]
    fn the_indent_is_the_indent_the_rest_of_the_report_uses() {
        let text = block(LINK, 4, &Style::colored());

        assert!(lines(&text).iter().all(|line| line.starts_with("    ")), "{text}");
    }
}
