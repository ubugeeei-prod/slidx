//! What an image file says about its own size, and how far a slide may draw it
//! from that.
//!
//! Two integers decide whether a picture survives projection. An image drawn
//! wider than its own pixels is resampled and goes soft; an image drawn at a
//! shape other than its own is stretched. Both are invisible on a laptop, where
//! the browser is downscaling the entire slide anyway, and obvious from row 20.
//!
//! # Reading a header, not an image
//!
//! Every format a deck can carry puts its dimensions in the first few dozen
//! bytes, so this reads a header rather than decoding a picture. That is the
//! whole design: the linter runs on every build and, in the editor, on every
//! keystroke, and taking on a decoder — its allocations, its compile time, and
//! its share of the image-parsing CVE feed — to learn two integers would be a
//! bad trade for a check that never looks at a pixel.
//!
//! # Why opening the file at all is safe
//!
//! Because of the offline guarantee. [`crate::rules::offline`] makes a remote
//! asset a lint error, so every image a deck may legally reference is already a
//! path on the machine doing the linting. Nothing here resolves a URL, and a
//! deck that would need it does not build.
//!
//! # What it refuses to guess
//!
//! [`probe`] returns `None` for a format it does not recognise or a header it
//! cannot make sense of, and the rule above it then says nothing at all. "I
//! have never seen an AVIF" is a fact about the linter, not a finding about the
//! deck, and a linter that reports those gets switched off.

use serde::{Deserialize, Serialize};

/// A format whose size can be read from its header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    Png,
    Jpeg,
    Gif,
    Webp,
    Svg,
}

impl Format {
    /// True when the file stores geometry rather than samples.
    ///
    /// Drawing an SVG larger costs nothing: the renderer re-runs the paths at
    /// the new size, so there is no resolution to run out of. Stretching one
    /// still stretches it, which is why this distinguishes the two checks
    /// rather than exempting vector art from both.
    pub fn is_scalable(self) -> bool {
        matches!(self, Self::Svg)
    }

    pub fn as_token(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Svg => "svg",
        }
    }
}

/// The size a file claims for itself, in its own pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Intrinsic {
    pub format: Format,
    pub width: u32,
    pub height: u32,
}

impl Intrinsic {
    /// Width over height, or `None` when the header gave a zero.
    pub fn aspect(self) -> Option<f64> {
        (self.width > 0 && self.height > 0).then(|| f64::from(self.width) / f64::from(self.height))
    }

    /// How many rendered pixels each source pixel has to cover across.
    ///
    /// At or below 1.0 the image is being drawn no wider than it was made, and
    /// a browser downscaling an image never softens it.
    pub fn upscale(self, rendered_width_px: f64) -> Option<f64> {
        (self.width > 0 && rendered_width_px > 0.0)
            .then(|| rendered_width_px / f64::from(self.width))
    }

    /// How far a declared box departs from the file's own shape, as a fraction.
    ///
    /// Symmetric on purpose: a box 25% too wide and one 25% too tall are the
    /// same amount of visible distortion, so they should report the same
    /// number rather than 0.25 and 0.33.
    pub fn aspect_drift(self, box_width_px: f64, box_height_px: f64) -> Option<f64> {
        let own = self.aspect()?;
        let declared =
            (box_width_px > 0.0 && box_height_px > 0.0).then_some(box_width_px / box_height_px)?;

        let ratio = declared / own;
        Some(if ratio >= 1.0 { ratio - 1.0 } else { 1.0 / ratio - 1.0 })
    }
}

/// How much softness and how much stretch a deck will accept.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tolerance {
    /// Largest multiple of its own width an image may be drawn at.
    pub max_upscale: f64,
    /// Largest fraction a declared box may depart from the file's own shape.
    pub max_aspect_drift: f64,
}

impl Default for Tolerance {
    /// **1.5x upscale.** Not 1.0: a deck authored against a 1920-wide canvas is
    /// drawn one design pixel to one device pixel on the 1080p projector that
    /// is still the conference norm, so an asset that exactly fills its box is
    /// correct and flagging it would flag every well-made deck. Resampling
    /// stays invisible at projection distance up to roughly a half again, and
    /// past that edges and any type baked into the image visibly soften. The
    /// case this is calibrated against is the one that motivated the rule: a
    /// 400px logo drawn across half of a 1920 slide is 2.4x, comfortably over.
    ///
    /// **2% aspect drift.** Below this a stretch is under the threshold at
    /// which a viewer reads a face as distorted, and it leaves room for the
    /// rounding an author does when they write whole pixels for a box that is
    /// nearly, but not exactly, the file's own ratio.
    fn default() -> Self {
        Self { max_upscale: 1.5, max_aspect_drift: 0.02 }
    }
}

/// The intrinsic size of an image, read from the head of its file.
///
/// `None` for a format this does not know, and for a truncated or malformed
/// header of one it does.
pub fn probe(bytes: &[u8]) -> Option<Intrinsic> {
    png(bytes)
        .or_else(|| gif(bytes))
        .or_else(|| jpeg(bytes))
        .or_else(|| webp(bytes))
        .or_else(|| svg(bytes))
}

const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// PNG puts IHDR first by specification, so the size sits at a fixed offset:
/// eight bytes of signature, eight of chunk header, then two big-endian words.
fn png(bytes: &[u8]) -> Option<Intrinsic> {
    if !bytes.starts_with(PNG_SIGNATURE) || bytes.get(12..16)? != b"IHDR" {
        return None;
    }

    Some(Intrinsic { format: Format::Png, width: be_u32(bytes, 16)?, height: be_u32(bytes, 20)? })
}

/// GIF's logical screen descriptor follows the six-byte version string.
fn gif(bytes: &[u8]) -> Option<Intrinsic> {
    if !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return None;
    }

    Some(Intrinsic {
        format: Format::Gif,
        width: u32::from(le_u16(bytes, 6)?),
        height: u32::from(le_u16(bytes, 8)?),
    })
}

/// JPEG hides its size behind a variable run of metadata, so the segments have
/// to be walked. EXIF and colour profiles routinely push the frame header a few
/// kilobytes in, which is why this cannot be a fixed offset like the others.
fn jpeg(bytes: &[u8]) -> Option<Intrinsic> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }

    let mut at = 2;
    loop {
        // Every segment opens with `0xFF`; anything else means the walk has
        // lost sync and any number it produced from here would be invented.
        if *bytes.get(at)? != 0xFF {
            return None;
        }

        // A marker may be preceded by any number of `0xFF` fill bytes.
        let mut code = at + 1;
        while bytes.get(code) == Some(&0xFF) {
            code += 1;
        }

        let marker = *bytes.get(code)?;
        at = code + 1;

        // Standalone markers carry no length field to skip over.
        if matches!(marker, 0x01 | 0xD0..=0xD8) {
            continue;
        }

        // Scan data and end-of-image: past here there is no header left.
        if matches!(marker, 0xD9 | 0xDA) {
            return None;
        }

        let length = usize::from(be_u16(bytes, at)?);
        if is_start_of_frame(marker) {
            // The frame header is precision, height, width — height first.
            return Some(Intrinsic {
                format: Format::Jpeg,
                width: u32::from(be_u16(bytes, at + 5)?),
                height: u32::from(be_u16(bytes, at + 3)?),
            });
        }

        at = at.checked_add(length.max(2))?;
    }
}

/// True for the frame headers that carry a size.
///
/// `C0`–`CF` is the start-of-frame range with three holes in it: `C4` is a
/// Huffman table, `C8` is reserved, and `CC` is an arithmetic-coding table.
/// Reading a size out of one of those would return a number rather than fail.
fn is_start_of_frame(marker: u8) -> bool {
    matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
}

/// WebP is a RIFF container, and the size lives in whichever of its three
/// bitstream chunks came first — lossy, lossless, or the extended header an
/// animated or alpha file uses.
fn webp(bytes: &[u8]) -> Option<Intrinsic> {
    if !bytes.starts_with(b"RIFF") || bytes.get(8..12)? != b"WEBP" {
        return None;
    }

    let payload = bytes.get(20..)?;
    let (width, height) = match bytes.get(12..16)? {
        b"VP8 " => vp8(payload),
        b"VP8L" => vp8l(payload),
        b"VP8X" => vp8x(payload),
        _ => None,
    }?;

    Some(Intrinsic { format: Format::Webp, width, height })
}

/// Lossy: a three-byte frame tag, the sync code, then two 14-bit fields.
fn vp8(payload: &[u8]) -> Option<(u32, u32)> {
    if payload.get(3..6)? != [0x9D, 0x01, 0x2A] {
        return None;
    }

    Some((u32::from(le_u16(payload, 6)? & 0x3FFF), u32::from(le_u16(payload, 8)? & 0x3FFF)))
}

/// Lossless: a signature byte, then width and height packed as 14 bits each,
/// both stored one less than their real value.
fn vp8l(payload: &[u8]) -> Option<(u32, u32)> {
    if *payload.first()? != 0x2F {
        return None;
    }

    let bits = le_u32(payload, 1)?;
    Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
}

/// Extended: flags, reserved bytes, then the canvas size as two 24-bit fields,
/// also stored one less than their real value.
fn vp8x(payload: &[u8]) -> Option<(u32, u32)> {
    Some((le_u24(payload, 4)? + 1, le_u24(payload, 7)? + 1))
}

/// SVG carries its size as text on the root element.
///
/// `width` and `height` are what the author declared; `viewBox` is the
/// coordinate system, and its third and fourth numbers are what a browser falls
/// back to when either is missing or given as a percentage — which is the
/// common case, because an SVG exported for the web usually has no fixed size.
fn svg(bytes: &[u8]) -> Option<Intrinsic> {
    let text = std::str::from_utf8(bytes).ok()?.trim_start_matches('\u{feff}');
    if !text.trim_start().starts_with('<') {
        return None;
    }

    let start = text.find("<svg")?;
    let rest = &text[start..];
    let tag = &rest[..rest.find('>').unwrap_or(rest.len())];

    let view_box = attribute(tag, "viewBox").and_then(view_box_size);
    let width = attribute(tag, "width").and_then(length).or(view_box.map(|(w, _)| w))?;
    let height = attribute(tag, "height").and_then(length).or(view_box.map(|(_, h)| h))?;

    Some(Intrinsic { format: Format::Svg, width, height })
}

/// An XML attribute value, matched on a whole name so that `stroke-width` does
/// not answer to `width`.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut at = 0;

    while let Some(found) = tag[at..].find(name) {
        let start = at + found;
        at = start + name.len();

        if !tag[..start].ends_with(char::is_whitespace) {
            continue;
        }

        let Some(rest) = tag[at..].trim_start().strip_prefix('=') else { continue };
        let rest = rest.trim_start();

        for quote in ['"', '\''] {
            if let Some(body) = rest.strip_prefix(quote) {
                return body.find(quote).map(|end| &body[..end]);
            }
        }
    }

    None
}

/// A CSS length as whole pixels. A percentage has no size of its own.
fn length(value: &str) -> Option<u32> {
    let value = value.trim();
    if value.ends_with('%') {
        return None;
    }

    let digits = value.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let number: f64 = digits.trim().parse().ok()?;

    (number >= 1.0).then(|| number.round() as u32)
}

/// The width and height out of a `viewBox`'s four numbers.
fn view_box_size(value: &str) -> Option<(u32, u32)> {
    let numbers: Vec<f64> = value
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect();

    let [.., width, height] = numbers[..] else { return None };
    (numbers.len() == 4 && width >= 1.0 && height >= 1.0)
        .then(|| (width.round() as u32, height.round() as u32))
}

fn be_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn be_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn le_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn le_u24(bytes: &[u8], at: usize) -> Option<u32> {
    let bytes = bytes.get(at..at + 3)?;
    Some(u32::from(bytes[0]) | u32::from(bytes[1]) << 8 | u32::from(bytes[2]) << 16)
}

fn le_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(test)]
pub(crate) mod fixtures {
    //! Minimal but real files, built byte by byte.
    //!
    //! Committing binaries would hide the contract this module is asserting:
    //! written out, the offsets a header parser depends on are visible in the
    //! test itself.

    /// A PNG: signature, then an IHDR chunk carrying two big-endian words.
    pub(crate) fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend([0, 0, 0, 13]);
        bytes.extend(b"IHDR");
        bytes.extend(width.to_be_bytes());
        bytes.extend(height.to_be_bytes());
        bytes.extend([8, 6, 0, 0, 0]);
        bytes
    }

    /// A GIF: version string, then a little-endian logical screen descriptor.
    pub(crate) fn gif(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend(width.to_le_bytes());
        bytes.extend(height.to_le_bytes());
        bytes.extend([0x70, 0x00, 0x00]);
        bytes
    }

    /// A JPEG with a JFIF segment ahead of the frame header, so the walk has
    /// something to step over rather than finding the size at a fixed offset.
    pub(crate) fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8];
        bytes.extend([0xFF, 0xE0, 0x00, 0x10]);
        bytes.extend(b"JFIF\0");
        bytes.extend([0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        bytes.extend([0xFF, 0xC0, 0x00, 0x11, 0x08]);
        bytes.extend(height.to_be_bytes());
        bytes.extend(width.to_be_bytes());
        bytes.extend([0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01]);
        bytes
    }

    /// A lossy WebP: RIFF wrapper, `VP8 ` chunk, sync code, two 14-bit fields.
    pub(crate) fn webp_lossy(width: u16, height: u16) -> Vec<u8> {
        let mut payload = vec![0x30, 0x01, 0x00, 0x9D, 0x01, 0x2A];
        payload.extend(width.to_le_bytes());
        payload.extend(height.to_le_bytes());
        riff(b"VP8 ", &payload)
    }

    /// A lossless WebP: signature byte, then both sizes packed less one.
    pub(crate) fn webp_lossless(width: u32, height: u32) -> Vec<u8> {
        let bits = (width - 1) | (height - 1) << 14;
        let mut payload = vec![0x2F];
        payload.extend(bits.to_le_bytes());
        riff(b"VP8L", &payload)
    }

    /// An extended WebP: the shape an animated or alpha file arrives in.
    pub(crate) fn webp_extended(width: u32, height: u32) -> Vec<u8> {
        let mut payload = vec![0x10, 0x00, 0x00, 0x00];
        payload.extend(&(width - 1).to_le_bytes()[..3]);
        payload.extend(&(height - 1).to_le_bytes()[..3]);
        riff(b"VP8X", &payload)
    }

    fn riff(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend((payload.len() as u32 + 12).to_le_bytes());
        bytes.extend(b"WEBP");
        bytes.extend(fourcc);
        bytes.extend((payload.len() as u32).to_le_bytes());
        bytes.extend(payload);
        bytes
    }

    pub(crate) fn svg(attributes: &str) -> Vec<u8> {
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" {attributes}><path d=\"M0 0\"/></svg>")
            .into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures as file;
    use super::*;

    fn size(bytes: &[u8]) -> (Format, u32, u32) {
        let intrinsic = probe(bytes).expect("a header this module claims to read");
        (intrinsic.format, intrinsic.width, intrinsic.height)
    }

    #[test]
    fn a_png_reports_the_size_in_its_ihdr_chunk() {
        assert_eq!(size(&file::png(1600, 900)), (Format::Png, 1600, 900));
    }

    #[test]
    fn a_gif_reports_its_logical_screen_size() {
        assert_eq!(size(&file::gif(320, 240)), (Format::Gif, 320, 240));
    }

    #[test]
    fn a_jpeg_reports_the_size_in_its_frame_header() {
        // The segment walk is the point: the size sits behind a JFIF block, so
        // a fixed offset would read the wrong bytes.
        assert_eq!(size(&file::jpeg(4032, 3024)), (Format::Jpeg, 4032, 3024));
    }

    #[test]
    fn a_jpeg_with_a_long_metadata_segment_is_still_read() {
        // A camera photograph arrives with kilobytes of EXIF ahead of the
        // frame header, which is the case that breaks a naive reader.
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE1];
        let exif = vec![0u8; 4000];
        bytes.extend(((exif.len() + 2) as u16).to_be_bytes());
        bytes.extend(&exif);
        bytes.extend(&file::jpeg(800, 600)[2..]);

        assert_eq!(size(&bytes), (Format::Jpeg, 800, 600));
    }

    #[test]
    fn a_lossy_webp_reports_its_frame_size() {
        assert_eq!(size(&file::webp_lossy(1920, 1080)), (Format::Webp, 1920, 1080));
    }

    #[test]
    fn a_lossless_webp_reports_its_frame_size() {
        assert_eq!(size(&file::webp_lossless(640, 480)), (Format::Webp, 640, 480));
    }

    #[test]
    fn an_extended_webp_reports_its_canvas_size() {
        assert_eq!(size(&file::webp_extended(2400, 1200)), (Format::Webp, 2400, 1200));
    }

    #[test]
    fn an_svg_reports_its_declared_width_and_height() {
        assert_eq!(size(&file::svg("width=\"400\" height=\"200\"")), (Format::Svg, 400, 200));
    }

    #[test]
    fn an_svg_length_with_a_unit_is_read_as_a_number() {
        assert_eq!(size(&file::svg("width=\"400px\" height=\"200px\"")), (Format::Svg, 400, 200));
    }

    #[test]
    fn an_svg_falls_back_to_its_view_box() {
        // The common export: no fixed size, only a coordinate system.
        assert_eq!(size(&file::svg("viewBox=\"0 0 512 128\"")), (Format::Svg, 512, 128));
    }

    #[test]
    fn a_percentage_width_falls_back_to_the_view_box_rather_than_reading_as_100px() {
        let bytes = file::svg("width=\"100%\" height=\"100%\" viewBox=\"0 0 64 32\"");
        assert_eq!(size(&bytes), (Format::Svg, 64, 32));
    }

    #[test]
    fn a_view_box_with_a_negative_origin_still_gives_the_size() {
        assert_eq!(size(&file::svg("viewBox=\"-8 -8 48 24\"")), (Format::Svg, 48, 24));
    }

    #[test]
    fn a_comma_separated_view_box_is_read() {
        assert_eq!(size(&file::svg("viewBox=\"0,0,100,50\"")), (Format::Svg, 100, 50));
    }

    #[test]
    fn an_attribute_that_only_ends_in_width_is_not_read_as_the_width() {
        // `stroke-width` on the root element must not become the size.
        let bytes = file::svg("stroke-width=\"4\" viewBox=\"0 0 300 150\"");
        assert_eq!(size(&bytes), (Format::Svg, 300, 150));
    }

    #[test]
    fn an_svg_behind_an_xml_declaration_is_still_found() {
        let mut bytes = b"<?xml version=\"1.0\"?>\n".to_vec();
        bytes.extend(file::svg("width=\"10\" height=\"5\""));

        assert_eq!(size(&bytes), (Format::Svg, 10, 5));
    }

    #[test]
    fn an_unrecognised_format_is_not_a_finding() {
        // The rule reports what it knows and stays silent about the rest, so
        // an unknown format has to come back as "no information" rather than
        // as a guess or an error.
        assert_eq!(probe(b"\x00\x00\x00\x20ftypavif"), None);
        assert_eq!(probe(b"BM\x36\x00\x00\x00"), None);
        assert_eq!(probe(b""), None);
        assert_eq!(probe(b"not an image at all"), None);
    }

    #[test]
    fn a_truncated_header_is_not_a_finding_either() {
        // A half-written file during a dev-server write must not report a
        // size of zero, and must not panic on the slice that is not there.
        for bytes in [
            file::png(1600, 900),
            file::jpeg(800, 600),
            file::webp_lossy(640, 480),
            file::gif(10, 10),
        ] {
            for cut in 1..bytes.len() {
                probe(&bytes[..cut]);
            }
            assert_eq!(probe(&bytes[..bytes.len() / 2]), None, "half a header is not a size");
        }
    }

    #[test]
    fn a_jpeg_that_reaches_its_scan_data_without_a_frame_header_reports_nothing() {
        assert_eq!(probe(&[0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02]), None);
    }

    #[test]
    fn an_svg_with_neither_a_size_nor_a_view_box_reports_nothing() {
        assert_eq!(probe(&file::svg("class=\"logo\"")), None);
    }

    #[test]
    fn upscale_is_the_multiple_of_its_own_width_an_image_is_drawn_at() {
        let logo = Intrinsic { format: Format::Png, width: 400, height: 200 };

        // The failure the rule exists for: 400px across half a 1920 slide.
        assert_eq!(logo.upscale(960.0), Some(2.4));
        assert_eq!(logo.upscale(400.0), Some(1.0));
        assert_eq!(logo.upscale(200.0), Some(0.5));
    }

    #[test]
    fn a_box_matching_the_file_has_no_aspect_drift() {
        let photo = Intrinsic { format: Format::Jpeg, width: 1600, height: 900 };
        assert_eq!(photo.aspect_drift(800.0, 450.0), Some(0.0));
    }

    #[test]
    fn stretching_and_squashing_by_the_same_amount_report_the_same_number() {
        // Otherwise a box 25% too wide reads as worse than one 25% too tall,
        // and the threshold would mean two different things.
        let square = Intrinsic { format: Format::Png, width: 100, height: 100 };

        let wide = square.aspect_drift(125.0, 100.0).unwrap();
        let tall = square.aspect_drift(100.0, 125.0).unwrap();

        assert!((wide - tall).abs() < 1e-12);
        assert!((wide - 0.25).abs() < 1e-12);
    }

    #[test]
    fn a_zero_dimension_produces_no_verdict_rather_than_an_infinity() {
        let broken = Intrinsic { format: Format::Png, width: 0, height: 0 };

        assert_eq!(broken.aspect(), None);
        assert_eq!(broken.upscale(960.0), None);
        assert_eq!(broken.aspect_drift(800.0, 600.0), None);

        let fine = Intrinsic { format: Format::Png, width: 100, height: 100 };
        assert_eq!(fine.aspect_drift(800.0, 0.0), None);
        assert_eq!(fine.upscale(0.0), None);
    }

    #[test]
    fn vector_art_is_the_only_scalable_format() {
        assert!(Format::Svg.is_scalable());
        for format in [Format::Png, Format::Jpeg, Format::Gif, Format::Webp] {
            assert!(!format.is_scalable(), "{} stores samples", format.as_token());
        }
    }

    #[test]
    fn the_default_tolerance_passes_a_one_to_one_asset_and_fails_the_motivating_case() {
        let tolerance = Tolerance::default();
        let logo = Intrinsic { format: Format::Png, width: 400, height: 200 };

        assert!(logo.upscale(400.0).unwrap() <= tolerance.max_upscale);
        assert!(logo.upscale(960.0).unwrap() > tolerance.max_upscale);
    }
}
