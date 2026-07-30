//! Which exports exist, and what each one needs from a build.
//!
//! Split from the packaging for the same reason [`crate::zip`] is split from
//! it: this is the list a person edits when they add a target, and it is read
//! three times over — by the parser, by the help text, and by the report that
//! says what was written. One table, so a target cannot print one spelling and
//! parse another.
//!
//! Every target also carries the sentence saying what survives the trip. An
//! export that silently loses the animation is worse than one that says it
//! will, and a promise kept next to the thing that makes it is a promise that
//! stays true.

/// One thing `slidx export` can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTarget {
    /// The static site the build already emits.
    Browser,
    /// The deck as one document.
    Pdf,
    /// One PDF per slide, which is the thing a single document cannot be.
    PdfZip,
    /// One image per stop.
    Png,
}

/// Something a build has to be asked for, beyond its ordinary output.
///
/// Every one of these is rendered by `@slidx/vite-plugin` driving a browser
/// over the print shell it emitted. Naming them here rather than rendering them
/// here is the boundary this crate exists to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// The whole-deck PDF, which the build knows how to write and normally
    /// does not, because it costs a browser download nobody asked for.
    Pdf,
    PdfPerSlide,
    Png,
}

/// Where the build writes the frames an export asked for.
///
/// Under the build's output directory, so it is emptied with everything else on
/// the next build and no export can be packaged from a stale render.
pub const FRAME_DIRECTORY: &str = "export";

/// Every target, in the order the help text lists them.
pub const EXPORT_TARGETS: &[ExportTarget] =
    &[ExportTarget::Browser, ExportTarget::Pdf, ExportTarget::PdfZip, ExportTarget::Png];

impl ExportTarget {
    /// The spelling `--target` accepts and the report prints.
    pub fn as_token(&self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Pdf => "pdf",
            Self::PdfZip => "pdf-zip",
            Self::Png => "png",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        EXPORT_TARGETS.iter().copied().find(|target| target.as_token() == token)
    }

    /// What the file is, in the words of somebody choosing between them.
    pub fn summary(&self) -> &'static str {
        match self {
            Self::Browser => "the static site, ready to upload or hand over",
            Self::Pdf => "the deck as one document",
            Self::PdfZip => "one PDF per slide, in a zip",
            Self::Png => "one image per stop, in a zip",
        }
    }

    /// What survives the trip, said before the file is opened somewhere else.
    ///
    /// The stop is the unit of every static export, which is the answer the
    /// print shell already gave: a handout that collapses an eight-step build
    /// into one slide shows the punchline without the setup. Being consistent
    /// with that is worth more than any per-target cleverness.
    pub fn keeps(&self) -> &'static str {
        match self {
            Self::Browser => {
                "every slide, every stop, the presenter view and the snippet pages — this is the \
                 deck itself, and it needs no network"
            }
            Self::Pdf => "one page per stop, so the build reads in the order the room saw it",
            Self::PdfZip => {
                "one file per slide, each holding that slide's stops as its pages — nothing is \
                 dropped, the boundary is just the slide"
            }
            Self::Png => {
                "one image per stop, so a slide that builds in four steps is four images rather \
                 than one showing the answer"
            }
        }
    }

    /// What the build has to render before this can be packaged.
    pub fn frame(&self) -> Option<Frame> {
        match self {
            Self::Browser => None,
            Self::Pdf => Some(Frame::Pdf),
            Self::PdfZip => Some(Frame::PdfPerSlide),
            Self::Png => Some(Frame::Png),
        }
    }

    /// What the exported file is called, given the deck's slug.
    ///
    /// The slug is in the name because these land in a downloads folder next to
    /// last year's. A file called `deck.zip` is one somebody has to open to
    /// find out what it is.
    pub fn file_name(&self, slug: &str) -> String {
        match self {
            Self::Browser => format!("{slug}-site.zip"),
            Self::Pdf => format!("{slug}.pdf"),
            Self::PdfZip => format!("{slug}-pdfs.zip"),
            Self::Png => format!("{slug}-pngs.zip"),
        }
    }
}

impl Frame {
    /// How the build is asked for this frame.
    pub fn as_token(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::PdfPerSlide => "pdf-slides",
            Self::Png => "png",
        }
    }

    /// Where the frame lands under the build's output, if it is its own file.
    pub fn directory(&self) -> Option<&'static str> {
        match self {
            // The whole-deck PDF is the one the build already names and writes
            // at the top of its output; asking for it only turns it on.
            Self::Pdf => None,
            Self::PdfPerSlide => Some("export/pdf"),
            Self::Png => Some("export/png"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_target_is_reachable_by_the_token_it_prints() {
        // The list, the parser and the help text are one table read three
        // times. A target that printed one spelling and parsed another would
        // be unreachable through the only name a person is shown.
        for target in EXPORT_TARGETS {
            assert_eq!(ExportTarget::parse(target.as_token()), Some(*target));
        }
    }

    #[test]
    fn a_target_nobody_has_is_not_guessed_at() {
        assert_eq!(ExportTarget::parse("keynote"), None);
        assert_eq!(ExportTarget::parse(""), None);
    }

    #[test]
    fn the_static_site_needs_nothing_a_build_does_not_already_emit() {
        // The one target that is pure packaging: the pages are the build's
        // ordinary output, so this export works on a build somebody else ran.
        assert_eq!(ExportTarget::Browser.frame(), None);
    }

    #[test]
    fn every_other_target_names_the_frames_the_build_has_to_render() {
        // Rendering is the plugin's, always. A target that needed images and
        // did not say so would either export nothing or grow a renderer here.
        for target in EXPORT_TARGETS.iter().filter(|target| **target != ExportTarget::Browser) {
            assert!(target.frame().is_some(), "{} asks the build for nothing", target.as_token());
        }
    }

    #[test]
    fn a_frame_that_lands_in_the_output_says_where() {
        assert_eq!(Frame::Png.directory(), Some("export/png"));
        assert_eq!(Frame::PdfPerSlide.directory(), Some("export/pdf"));
        // The whole-deck PDF is the one the build already names and writes at
        // the top of its output, so there is no frame directory to look in.
        assert_eq!(Frame::Pdf.directory(), None);
    }

    #[test]
    fn a_frame_directory_sits_under_the_one_the_build_is_told_to_use() {
        for frame in [Frame::Pdf, Frame::PdfPerSlide, Frame::Png] {
            if let Some(directory) = frame.directory() {
                assert!(directory.starts_with(FRAME_DIRECTORY), "{directory} is somewhere else");
            }
        }
    }

    #[test]
    fn a_file_name_says_which_deck_and_which_export_it_is() {
        // These land in a downloads folder next to last year's. A file called
        // deck.zip is one somebody has to open to identify.
        assert_eq!(
            ExportTarget::Browser.file_name("making-decks-fast"),
            "making-decks-fast-site.zip"
        );
        assert_eq!(ExportTarget::Pdf.file_name("making-decks-fast"), "making-decks-fast.pdf");
        assert_eq!(
            ExportTarget::PdfZip.file_name("making-decks-fast"),
            "making-decks-fast-pdfs.zip"
        );
        assert_eq!(ExportTarget::Png.file_name("making-decks-fast"), "making-decks-fast-pngs.zip");
    }

    #[test]
    fn no_two_targets_share_a_file_name() {
        // Exporting twice into one directory has to leave two files, or the
        // second export silently replaces the first.
        let mut names: Vec<String> =
            EXPORT_TARGETS.iter().map(|target| target.file_name("talk")).collect();
        let total = names.len();
        names.sort();
        names.dedup();

        assert_eq!(names.len(), total);
    }

    #[test]
    fn every_target_says_what_survives_the_trip_and_what_does_not() {
        // An export that silently loses the animation is worse than one that
        // says it will, so the sentence is part of the target rather than a
        // paragraph in a document nobody opens.
        for target in EXPORT_TARGETS {
            assert!(!target.summary().is_empty(), "{} says nothing", target.as_token());
            assert!(!target.keeps().is_empty(), "{} promises silently", target.as_token());
            assert!(!target.keeps().contains('\n'), "{} wraps its own line", target.as_token());
        }
    }
}
