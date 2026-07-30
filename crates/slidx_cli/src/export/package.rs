//! Reading what the build wrote, and putting it in one file.
//!
//! Every path here is a read, a copy, or an archive entry. Nothing renders, and
//! nothing is regenerated when it is missing: a frame the build did not write is
//! reported with what to run, because the alternative is an export that quietly
//! contains fewer slides than the deck has.

use std::fs;
use std::path::{Path, PathBuf};

use slidx_core::Deck;
use slidx_export::{pptx, zip, ExportTarget, Frame, PptxDeck, PptxSlide};

use crate::preview::Built;

/// The file an export produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packaged {
    pub path: PathBuf,
    /// How many files went into it, for a report that can be believed.
    ///
    /// A zip holding one page when the deck has forty looks identical in a
    /// listing, so the count is the only part of the report worth printing.
    pub parts: usize,
}

/// Packages one target from a build, into `out`.
///
/// The deck is read for the one thing a build's output does not carry: the
/// speaker notes, which the presentation export has to attach as text.
pub fn package(
    target: ExportTarget,
    built: &Built,
    deck: &Deck,
    out: &Path,
    slug: &str,
) -> Result<Packaged, String> {
    let bytes = match target {
        ExportTarget::Browser => site(built)?,
        ExportTarget::Pdf => document(built)?,
        ExportTarget::PdfZip => frames(built, Frame::PdfPerSlide, "pdf")?,
        ExportTarget::Png => frames(built, Frame::Png, "png")?,
        ExportTarget::Pptx => presentation(built, deck, slug)?,
    };

    let path = out.join(target.file_name(slug));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| unwritable(parent, &error))?;
    }

    fs::write(&path, bytes.0).map_err(|error| unwritable(&path, &error))?;

    Ok(Packaged { path, parts: bytes.1 })
}

/// Bytes, and how many files went into them.
type Bundle = (Vec<u8>, usize);

/// The whole static site, as one archive.
fn site(built: &Built) -> Result<Bundle, String> {
    let files = walk(&built.root);

    if files.is_empty() {
        return Err(format!("{} has nothing in it to package.\n", built.root.display()));
    }

    let mut entries = Vec::with_capacity(files.len());

    for path in &files {
        let bytes = fs::read(path).map_err(|error| unreadable(path, &error))?;
        entries.push(zip::Entry::new(archive_path(&built.root, path), bytes));
    }

    Ok((zip::write(&entries), entries.len()))
}

/// The deck's own PDF, copied rather than rewritten.
fn document(built: &Built) -> Result<Bundle, String> {
    let Some(pdf) = &built.pdf else {
        return Err(no_pdf(&built.root));
    };

    let bytes = fs::read(pdf).map_err(|error| unreadable(pdf, &error))?;

    Ok((bytes, 1))
}

/// Every frame of one kind, as one archive.
fn frames(built: &Built, frame: Frame, extension: &str) -> Result<Bundle, String> {
    let directory = built.root.join(frame.directory().unwrap_or_default());

    let files: Vec<PathBuf> = walk(&directory)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|found| found == extension))
        .collect();

    if files.is_empty() {
        return Err(no_frames(&directory, frame));
    }

    let mut entries = Vec::with_capacity(files.len());

    for path in &files {
        let bytes = fs::read(path).map_err(|error| unreadable(path, &error))?;
        entries.push(zip::Entry::new(archive_path(&directory, path), bytes));
    }

    Ok((zip::write(&entries), entries.len()))
}

/// The deck as a presentation: the rendered stops, with the notes as text.
///
/// The images come from the build and the notes come from the deck, and putting
/// them back together needs to know which slide each image belongs to. That
/// comes out of the file name the build wrote — `slide-03-stop-02.png` — and a
/// name that does not parse is an error rather than a slide whose notes quietly
/// went missing. The two sides of that spelling are `frames.ts` and
/// `slidx_export`'s `Frame`, and `tests/export.rs` is what keeps them honest.
fn presentation(built: &Built, deck: &Deck, title: &str) -> Result<Bundle, String> {
    let directory = built.root.join(Frame::Png.directory().unwrap_or_default());

    let images: Vec<PathBuf> = walk(&directory)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|found| found == "png"))
        .collect();

    if images.is_empty() {
        return Err(no_frames(&directory, Frame::Png));
    }

    let mut slides = Vec::with_capacity(images.len());

    for path in &images {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let index = slide_of(&name).ok_or_else(|| unreadable_name(&name))?;
        let slide = deck.slides.get(index);

        slides.push(PptxSlide {
            image: fs::read(path).map_err(|error| unreadable(path, &error))?,
            title: slide.and_then(|slide| slide.title.clone()),
            notes: slide.map(|slide| slide.notes.clone()).unwrap_or_default(),
        });
    }

    let total = slides.len();
    let (width, height) = deck.meta.aspect.dimensions();

    Ok((
        pptx::write(&PptxDeck { title: title.to_string(), aspect: (width, height), slides }),
        total,
    ))
}

/// Which slide an image belongs to, from the name the build gave it.
///
/// Zero-based, because that is how the deck is indexed. The stop is deliberately
/// ignored: notes belong to the slide, and every stop of one carries them.
fn slide_of(name: &str) -> Option<usize> {
    let digits: String =
        name.strip_prefix("slide-")?.chars().take_while(char::is_ascii_digit).collect();

    digits.parse::<usize>().ok().filter(|number| *number > 0).map(|number| number - 1)
}

/// Every file under a directory, in a fixed order.
///
/// Sorted, because an export is meant to be a pure function of the build it
/// packages and directory order is whatever the filesystem feels like. Two
/// exports of one build that differed only in entry order would still be two
/// different files, which is exactly what a cache or a diff cannot see past.
///
/// A previous export's frames are skipped: they are staged output rather than
/// part of the site, and a build that was asked for them emptied the directory
/// first, so what is left is only ever from this run.
fn walk(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else { return Vec::new() };

    let mut found: Vec<PathBuf> =
        entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
    found.sort();

    found
        .into_iter()
        .flat_map(|path| {
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == slidx_export::FRAME_DIRECTORY) {
                    return Vec::new();
                }
                return walk(&path);
            }

            vec![path]
        })
        .collect()
}

/// A path inside the archive: relative to the root, with forward slashes.
///
/// Zip's separator is `/` on every platform. A backslash from a Windows build
/// would store one file whose name contains a slash-shaped character, which
/// unpacks as a flat directory of oddly named files.
fn archive_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn no_pdf(root: &Path) -> String {
    format!(
        "The build in {} produced no PDF.\n\n\
         Rendering one needs a browser, which is an optional install:\n\n\
         \x20 vp add -D playwright && vp exec playwright install chromium\n\n\
         Then `slidx export --target pdf` again.\n",
        root.display()
    )
}

fn no_frames(directory: &Path, frame: Frame) -> String {
    format!(
        "The build wrote no {} frames, so there is nothing to package.\n\n\
         slidx asked for them in {}. They are rendered by a browser, which is an\n\
         optional install:\n\n\
         \x20 vp add -D playwright && vp exec playwright install chromium\n\n\
         With --no-build, they are only there if the last build was an export of\n\
         the same target — a build empties its output directory.\n",
        frame.as_token(),
        directory.display()
    )
}

/// A frame whose name the packaging cannot place in the deck.
///
/// Loud rather than quiet, because the quiet version is a presentation whose
/// notes are missing from every slide — and nobody checks the notes until they
/// are standing up.
fn unreadable_name(name: &str) -> String {
    format!(
        "The build wrote a frame called {name}, which does not say which slide it is.\n\n\
         slidx expects `slide-<number>-stop-<number>.png`. A name in another shape means\n\
         the binary and @slidx/vite-plugin are different versions:\n\n\
         \x20 vp add -D @slidx/vite-plugin@latest\n"
    )
}

fn unreadable(path: &Path, error: &std::io::Error) -> String {
    format!("Could not read {}: {error}\n", path.display())
}

fn unwritable(path: &Path, error: &std::io::Error) -> String {
    format!(
        "Could not write {}: {error}\n\n\
         `slidx export` writes into the current directory. Point it somewhere else\n\
         with `--out <path>`.\n",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A project, holding a build output directory laid out the way the plugin
    /// leaves one.
    ///
    /// The export lands *beside* the output rather than inside it. Inside, a
    /// second run would package the first run's archive — which is a real thing
    /// to do by accident and reads as an export that keeps growing.
    struct Output(PathBuf);

    impl Output {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("slidx-package-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("dist/slides/2")).expect("scratch");
            fs::write(root.join("dist/slides/index.html"), "<h1>One</h1>").expect("slide");
            fs::write(root.join("dist/slides/2/index.html"), "<h1>Two</h1>").expect("slide");
            fs::write(root.join("dist/slides/runtime.js"), "export const x = 1;").expect("runtime");

            Self(root)
        }

        fn dist(&self) -> PathBuf {
            self.0.join("dist")
        }

        fn pdf(self) -> Self {
            fs::write(self.dist().join("deck.pdf"), b"%PDF-1.7\n%%EOF\n").expect("pdf");
            self
        }

        fn frame(self, under: &str, name: &str, bytes: &[u8]) -> Self {
            let directory = self.dist().join(under);
            fs::create_dir_all(&directory).expect("frames");
            fs::write(directory.join(name), bytes).expect("frame");
            self
        }

        fn built(&self) -> Built {
            Built::find(&self.dist()).expect("a build")
        }

        fn package(&self, target: ExportTarget) -> Result<Packaged, String> {
            package(target, &self.built(), &self.deck(), &self.0.join("out"), "making-decks-fast")
        }

        /// The deck those pages were built from, for the notes.
        fn deck(&self) -> Deck {
            slidx_core::parse_deck(
                "---\ntitle: Making Decks Fast\n---\n\n# One\n\n\
                 <!-- notes: open with the outcome -->\n\n---\n\n# Two\n",
                &slidx_core::DeckParseOptions::default(),
            )
        }
    }

    impl Drop for Output {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn read(packaged: &Packaged) -> Vec<u8> {
        fs::read(&packaged.path).expect("the exported file")
    }

    #[test]
    fn the_site_export_is_an_archive_a_reader_can_open() {
        // "A zip that is not a zip" is this command's failure mode: it looks
        // right in a listing and fails in the one place it is opened.
        let output = Output::new("site");
        let packaged = output.package(ExportTarget::Browser).expect("packaged");

        assert_eq!(&read(&packaged)[..4], b"PK\x03\x04");
        assert!(packaged.path.ends_with("making-decks-fast-site.zip"));
    }

    #[test]
    fn the_site_export_holds_every_page_the_build_wrote() {
        let output = Output::new("pages");
        let packaged = output.package(ExportTarget::Browser).expect("packaged");

        assert_eq!(
            zip::names(&read(&packaged)),
            ["slides/2/index.html", "slides/index.html", "slides/runtime.js"]
        );
        assert_eq!(packaged.parts, 3);
    }

    #[test]
    fn the_frames_a_previous_export_staged_are_not_part_of_the_site() {
        // They are output of an export, not pages of the deck. A site zip
        // carrying last run's forty screenshots is one somebody uploads.
        let output = Output::new("staged").frame("export/png", "slide-01-stop-01.png", b"\x89PNG");
        let packaged = output.package(ExportTarget::Browser).expect("packaged");

        assert!(
            !zip::names(&read(&packaged)).iter().any(|name| name.contains("export/")),
            "{:?}",
            zip::names(&read(&packaged))
        );
    }

    #[test]
    fn the_pdf_export_is_the_document_the_build_rendered_rather_than_a_new_one() {
        // Copied byte for byte. Anything else would be a second renderer, and
        // the PDF a speaker hands over could then differ from the one they
        // checked.
        let output = Output::new("pdf").pdf();
        let packaged = output.package(ExportTarget::Pdf).expect("packaged");

        assert_eq!(read(&packaged), b"%PDF-1.7\n%%EOF\n");
        assert!(packaged.path.ends_with("making-decks-fast.pdf"));
    }

    #[test]
    fn a_build_with_no_pdf_says_what_renders_one_rather_than_writing_an_empty_file() {
        let output = Output::new("nopdf");
        let error = output.package(ExportTarget::Pdf).expect_err("no pdf");

        assert!(error.contains("playwright"), "{error}");
    }

    #[test]
    fn the_per_slide_export_holds_one_pdf_per_slide_under_the_name_the_build_gave_it() {
        let output = Output::new("perslide")
            .frame("export/pdf", "slide-01.pdf", b"%PDF-1.7\n")
            .frame("export/pdf", "slide-02.pdf", b"%PDF-1.7\n");
        let packaged = output.package(ExportTarget::PdfZip).expect("packaged");

        assert_eq!(zip::names(&read(&packaged)), ["slide-01.pdf", "slide-02.pdf"]);
        assert_eq!(&read(&packaged)[..4], b"PK\x03\x04");
    }

    #[test]
    fn the_image_export_holds_one_file_per_stop() {
        let output = Output::new("png")
            .frame("export/png", "slide-01-stop-01.png", b"\x89PNG\r\n")
            .frame("export/png", "slide-01-stop-02.png", b"\x89PNG\r\n");
        let packaged = output.package(ExportTarget::Png).expect("packaged");

        assert_eq!(packaged.parts, 2);
        assert!(packaged.path.ends_with("making-decks-fast-pngs.zip"));
    }

    #[test]
    fn a_frame_of_the_wrong_kind_is_not_packaged_as_one() {
        // The frame directory is written by a build; a stray file in it must not
        // become an entry claiming to be a slide.
        let output = Output::new("stray")
            .frame("export/png", "slide-01-stop-01.png", b"\x89PNG\r\n")
            .frame("export/png", "notes.txt", b"scratch\n");
        let packaged = output.package(ExportTarget::Png).expect("packaged");

        assert_eq!(zip::names(&read(&packaged)), ["slide-01-stop-01.png"]);
    }

    #[test]
    fn no_frames_at_all_names_what_renders_them_and_why_no_build_may_have_none() {
        let output = Output::new("noframes");
        let error = output.package(ExportTarget::Png).expect_err("no frames");

        assert!(error.contains("playwright"), "{error}");
        assert!(error.contains("--no-build"), "{error}");
    }

    #[test]
    fn exporting_the_same_build_twice_writes_the_same_bytes() {
        // An export is a pure function of the build it packages, so a cache can
        // be trusted and a diff of two exports means something.
        let output = Output::new("twice");
        let first = read(&output.package(ExportTarget::Browser).expect("packaged"));
        let second = read(&output.package(ExportTarget::Browser).expect("packaged"));

        assert_eq!(first, second);
    }

    #[test]
    fn an_archive_path_uses_the_separator_zip_means_on_every_platform() {
        assert_eq!(
            archive_path(Path::new("dist"), &Path::new("dist").join("slides").join("index.html")),
            "slides/index.html"
        );
    }

    #[test]
    fn a_directory_that_cannot_be_written_says_how_to_point_it_somewhere_else() {
        // `--out` pointed inside a regular file. Chosen over a path nobody has
        // permission to write because that is not the same path on every
        // machine: an absolute one is writable on Windows and as root.
        let output = Output::new("unwritable");
        fs::write(output.0.join("not-a-directory"), "already a file\n").expect("write");
        let occupied = output.0.join("not-a-directory").join("inside-it");

        let error =
            package(ExportTarget::Browser, &output.built(), &output.deck(), &occupied, "talk")
                .expect_err("unwritable");

        assert!(error.contains("--out"), "{error}");
    }
}
