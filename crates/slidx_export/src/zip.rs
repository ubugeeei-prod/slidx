//! The archive container, written by hand.
//!
//! A deck's output is a directory and an attachment is a file, so every export
//! that is more than one document ends up here. Three of them do — the static
//! site, the per-slide PDFs, the per-stop images — and a `.pptx` is a zip too,
//! which is the whole reason writing one is a bounded job rather than a project.
//!
//! ## Stored, never compressed
//!
//! Every entry is written with method 0. What goes in these archives is PNG,
//! PDF and already-minified HTML, all of which are compressed streams already;
//! deflating them again buys a percent or two for a compressor in the supply
//! chain of a binary people pipe into a shell. The XML parts of a `.pptx` are
//! the exception and they are kilobytes.
//!
//! ## Fixed timestamps
//!
//! Every entry is dated 1980-01-01, the earliest an MS-DOS field can express.
//! An export is a pure function of the build it packages, so two runs produce
//! the same bytes — which is what makes a cached export safe and a diff of two
//! exports meaningful. A real mtime would change every field in every header on
//! every run and say nothing a person wanted to know.
//!
//! ## What it does not do
//!
//! No zip64, so a single entry or a whole archive over 4 GiB would be written
//! with a truncated size field. A deck's output is megabytes; the limit is
//! recorded rather than handled because handling it would mean carrying the
//! second header format for a case that cannot occur.

/// One file inside an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The path inside the archive, with forward slashes and no leading one.
    ///
    /// Zip's own separator is `/` on every platform, so a caller walking a
    /// directory on Windows has to convert. Storing a backslash produces an
    /// archive whose entries unpack as one file with an odd name.
    pub path: String,
    pub bytes: Vec<u8>,
}

impl Entry {
    pub fn new(path: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self { path: path.into(), bytes }
    }
}

/// The earliest date an MS-DOS field can hold: 1980-01-01, at midnight.
///
/// Year is counted from 1980, and both month and day are one-based, so zero is
/// not a legal value in either field — some tools report an archive full of
/// them as corrupt.
const DOS_DATE: u16 = (1 << 5) | 1;
const DOS_TIME: u16 = 0;

/// Names are UTF-8, said in the flags so a reader does not guess.
///
/// Without this bit a name is code page 437 by specification, and every
/// non-ASCII file name in an author's deck — an image called `図.png` — unpacks
/// as mojibake. The flags have to match between the local and central headers,
/// so this is set in both from one constant.
const UTF8_NAMES: u16 = 0x0800;

/// Deflate is never used, so the version needed to extract is the original one.
const VERSION: u16 = 20;

const LOCAL_HEADER: u32 = 0x0403_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_OF_CENTRAL_DIRECTORY: u32 = 0x0605_4b50;

/// Writes a zip archive holding these entries, in this order.
///
/// Order is the caller's, because it is meaningful: OPC readers expect
/// `[Content_Types].xml` at the front of a `.pptx`, and a person unpacking a
/// site wants `index.html` before the fonts.
pub fn write(entries: &[Entry]) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut directory = Vec::new();

    for entry in entries {
        let offset = archive.len() as u32;
        let crc = crc32(&entry.bytes);
        let size = entry.bytes.len() as u32;

        push_u32(&mut archive, LOCAL_HEADER);
        push_common(&mut archive, crc, size, entry.path.len() as u16);
        archive.extend_from_slice(entry.path.as_bytes());
        archive.extend_from_slice(&entry.bytes);

        push_u32(&mut directory, CENTRAL_HEADER);
        push_u16(&mut directory, VERSION);
        push_common(&mut directory, crc, size, entry.path.len() as u16);
        // Comment length, the disk this entry starts on, and the two attribute
        // fields. All zero: one disk, no comment, and no MS-DOS or Unix mode
        // worth asserting about a file that came out of a build.
        directory.extend_from_slice(&[0; 10]);
        push_u32(&mut directory, offset);
        directory.extend_from_slice(entry.path.as_bytes());
    }

    let directory_offset = archive.len() as u32;
    let count = entries.len() as u16;

    archive.extend_from_slice(&directory);
    push_u32(&mut archive, END_OF_CENTRAL_DIRECTORY);
    // This disk, and the disk the central directory starts on.
    archive.extend_from_slice(&[0; 4]);
    push_u16(&mut archive, count);
    push_u16(&mut archive, count);
    push_u32(&mut archive, directory.len() as u32);
    push_u32(&mut archive, directory_offset);
    // Archive comment length.
    push_u16(&mut archive, 0);

    archive
}

/// The fields a local header and a central header spell the same way.
///
/// Written once because they have to agree: a reader that finds a different
/// method or a different flag word in the two places is entitled to reject the
/// archive, and the two are 16 bytes apart in a hand-written writer.
fn push_common(out: &mut Vec<u8>, crc: u32, size: u32, name_length: u16) {
    push_u16(out, VERSION);
    push_u16(out, UTF8_NAMES);
    // Method 0, stored.
    push_u16(out, 0);
    push_u16(out, DOS_TIME);
    push_u16(out, DOS_DATE);
    push_u32(out, crc);
    // Compressed and uncompressed size are the same thing when nothing is
    // compressed.
    push_u32(out, size);
    push_u32(out, size);
    push_u16(out, name_length);
    // Extra field length.
    push_u16(out, 0);
}

/// Every entry name in an archive, read from its central directory.
///
/// Exists so a caller can assert that what it wrote is readable as a zip at
/// all. "A zip that is not a zip" is the failure mode of an export — it looks
/// right in a file listing and fails in the one place it is opened — and the
/// only check worth having is one that parses the bytes back.
///
/// Returns nothing for anything that is not an archive, rather than guessing.
pub fn names(archive: &[u8]) -> Vec<String> {
    let Some(end) = end_of_central_directory(archive) else { return Vec::new() };

    let count = read_u16(archive, end + 10) as usize;
    let mut at = read_u32(archive, end + 16) as usize;
    let mut found = Vec::with_capacity(count);

    for _ in 0..count {
        if read_u32(archive, at) != CENTRAL_HEADER {
            break;
        }

        let length = read_u16(archive, at + 28) as usize;
        let extra = read_u16(archive, at + 30) as usize;
        let comment = read_u16(archive, at + 32) as usize;
        let name = archive.get(at + 46..at + 46 + length).unwrap_or_default();

        found.push(String::from_utf8_lossy(name).into_owned());
        at += 46 + length + extra + comment;
    }

    found
}

/// Where the end-of-central-directory record starts.
///
/// Found by scanning backwards, because the record carries a trailing comment
/// of any length and is therefore not at a fixed offset from the end. Nothing
/// this crate writes has a comment, but an archive that has been through
/// another tool may.
fn end_of_central_directory(archive: &[u8]) -> Option<usize> {
    (0..archive.len().saturating_sub(21))
        .rev()
        .find(|&at| read_u32(archive, at) == END_OF_CENTRAL_DIRECTORY)
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    let pair = bytes.get(at..at + 2).unwrap_or(&[0, 0]);
    u16::from_le_bytes([pair[0], pair[1]])
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    match bytes.get(at..at + 4) {
        Some(quad) => u32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]),
        None => 0,
    }
}

/// CRC-32, as zip means it: the IEEE polynomial, reflected.
///
/// Computed rather than depended on. Every entry needs one in two places, and
/// an archive whose checksums are wrong is refused by the readers that check
/// and accepted by the ones that do not — which is the worst of both.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;

    for byte in bytes {
        crc ^= u32::from(*byte);

        for _ in 0..8 {
            // The polynomial is applied only when the low bit is set. Masking
            // by the negated bit does that without a branch per bit.
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }

    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<Entry> {
        vec![
            Entry::new("slides/index.html", b"<h1>One</h1>".to_vec()),
            Entry::new("slides/2/index.html", b"<h1>Two</h1>".to_vec()),
        ]
    }

    #[test]
    fn an_archive_starts_with_the_signature_every_reader_looks_for() {
        // The failure this whole module exists to prevent: a file called
        // something.zip that no unzipper will open. It looks right in a
        // listing and fails in the one place it is used.
        let archive = write(&entries());

        assert_eq!(&archive[..4], b"PK\x03\x04");
    }

    #[test]
    fn every_entry_is_named_in_the_central_directory_the_end_record_points_at() {
        // Readers find entries through the central directory, not by scanning
        // for local headers. An archive whose directory disagreed with its
        // contents would list the wrong files or none.
        assert_eq!(names(&write(&entries())), ["slides/index.html", "slides/2/index.html"]);
    }

    #[test]
    fn an_entry_keeps_the_bytes_it_was_given() {
        let archive = write(&entries());

        assert!(archive.windows(12).any(|window| window == b"<h1>One</h1>"));
        assert!(archive.windows(12).any(|window| window == b"<h1>Two</h1>"));
    }

    #[test]
    fn entries_keep_the_order_the_caller_chose() {
        // Order is meaningful: an OPC reader expects [Content_Types].xml at the
        // front of a .pptx, and a person unpacking a site wants index.html
        // first.
        let reversed: Vec<Entry> = entries().into_iter().rev().collect();

        assert_eq!(names(&write(&reversed)), ["slides/2/index.html", "slides/index.html"]);
    }

    #[test]
    fn exporting_the_same_build_twice_produces_the_same_bytes() {
        // No clock anywhere: an export is a pure function of what it packages,
        // which is what lets a CI job cache one and what makes a diff of two
        // exports mean something.
        assert_eq!(write(&entries()), write(&entries()));
    }

    #[test]
    fn an_empty_archive_is_still_a_readable_archive() {
        // A deck with nothing shareable in it should produce an empty container
        // rather than a truncated file that reads as corrupt.
        let archive = write(&[]);

        assert_eq!(archive.len(), 22);
        assert!(names(&archive).is_empty());
    }

    #[test]
    fn the_checksum_is_the_one_zip_means() {
        // The published check value for the standard test vector. A writer
        // whose checksums are wrong is refused by the readers that verify and
        // accepted by the ones that do not, which is the worst of both.
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn a_name_that_is_not_ascii_is_declared_as_utf_8_rather_than_left_to_a_guess() {
        // Without the flag a name is code page 437 by specification, and an
        // author's image called 図.png unpacks as mojibake.
        let archive = write(&[Entry::new("slides/図.png", vec![1, 2, 3])]);

        assert_eq!(names(&archive), ["slides/図.png"]);
        assert_eq!(read_u16(&archive, 6) & UTF8_NAMES, UTF8_NAMES);
    }

    #[test]
    fn the_local_and_central_headers_agree_about_how_an_entry_was_stored() {
        // They are sixteen bytes apart in a hand-written writer, and a reader
        // that finds two different answers is entitled to reject the archive.
        let archive = write(&[Entry::new("a.txt", b"hello".to_vec())]);
        let central = read_u32(&archive, archive.len() - 6) as usize;

        for (local, directory) in [(6, 8), (8, 10), (14, 16), (18, 20), (22, 24)] {
            assert_eq!(
                read_u16(&archive, local),
                read_u16(&archive, central + directory),
                "field at {local} disagrees with the directory"
            );
        }
    }

    #[test]
    fn nothing_that_is_not_an_archive_is_read_as_one() {
        assert!(names(b"%PDF-1.7\n").is_empty());
        assert!(names(&[]).is_empty());
    }
}
