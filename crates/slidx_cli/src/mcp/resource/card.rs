//! The one resource an agent can look at rather than read.
//!
//! Everything else this server serves is text about a slide. This is a picture
//! of one, which is the difference between a model reasoning about Markdown and
//! a model seeing that the title runs to three lines.
//!
//! ## Why a PNG when there is one and an SVG when there is not
//!
//! slidx draws the card as SVG, from the same theme tokens as the deck, because
//! rendering text to pixels needs a font stack, a shaper and a layout engine —
//! several megabytes of dependency and a second implementation of type layout
//! that would drift from the one the slides use. The build rasterises it with
//! the browser that is already installed for the PDF.
//!
//! So a deck that has been built has PNGs on disk and this serves them; a deck
//! that has not gets the SVG. Both are said out loud in the answer rather than
//! quietly substituted, because an agent that thought it was looking at a
//! rasterised card and was reading markup would be confidently wrong about what
//! a viewer sees.
//!
//! There is no rasteriser here and there should not be. `vite build` in the
//! deck's own project is what produces one.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use slidx_render::{render_slide_card, OgOptions};

use crate::mcp::resource::{deck, text};
use crate::mcp::workspace::Workspace;

/// Where the plugin writes a built deck, when nobody has said otherwise.
const BUILD_DIR: &str = "dist";

/// One slide's card, as an image.
pub fn read(
    workspace: &Workspace,
    uri: &str,
    project: &Path,
    index: usize,
) -> Result<Vec<Value>, String> {
    let reading = workspace.read_deck(&project.display().to_string(), None)?;
    let slide = reading.deck.slides.get(index).ok_or_else(|| deck::missing(index, &reading))?;

    // The card the build wrote, if it wrote one. Named for the slide's position
    // counting from one, which is what `ogFileBase` in the plugin produces.
    if let Some(png) = built_card(project, index) {
        return Ok(vec![json!({
            "uri": uri,
            "mimeType": "image/png",
            "blob": base64(&png),
        })]);
    }

    let theme = reading
        .deck
        .meta
        .theme
        .as_deref()
        .and_then(slidx_theme::resolve)
        .unwrap_or_else(slidx_theme::default_theme);

    let options = OgOptions {
        theme,
        eyebrow: reading.deck.meta.talk.event.clone().or_else(|| reading.deck.meta.title.clone()),
        footer: reading.deck.meta.author.clone(),
    };

    Ok(vec![text(uri, "image/svg+xml", render_slide_card(&reading.deck, slide, &options))])
}

/// The PNG the build rasterised, if this deck has been built.
///
/// Looks beside the project rather than anywhere clever: `vite build` writes to
/// `dist/` unless the author said otherwise, and a search for an image across
/// somebody's machine is not something a resource read should do.
fn built_card(project: &Path, index: usize) -> Option<Vec<u8>> {
    let card = PathBuf::from(project).join(BUILD_DIR).join(format!("og-{}.png", index + 1));

    std::fs::read(card).ok()
}

/// Base64, as the protocol wants a blob.
///
/// Hand-written for the same reason the framing is: it is one table and three
/// lines of shifting, and this workspace does not take a dependency for that.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let block = chunk
            .iter()
            .enumerate()
            .fold(0u32, |block, (at, byte)| block | (u32::from(*byte) << (16 - 8 * at)));

        for at in 0..4 {
            // A chunk of one byte carries two characters, a chunk of two carries
            // three; the rest is padding, which is not optional in this encoding.
            if at <= chunk.len() {
                encoded.push(ALPHABET[((block >> (18 - 6 * at)) & 0x3F) as usize] as char);
            } else {
                encoded.push('=');
            }
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::uri::{self, SlideView};
    use std::fs;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("slidx-card-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("slides")).expect("scratch");
            fs::write(
                path.join("slides/0001.md"),
                "---\ntitle: A talk\nevent: SlidxConf\n---\n\n# Making Decks Fast\n",
            )
            .expect("write");
            Self(path)
        }

        fn built(&self, name: &str, bytes: &[u8]) {
            let dist = self.0.join(BUILD_DIR);
            fs::create_dir_all(&dist).expect("a build directory");
            fs::write(dist.join(name), bytes).expect("write");
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn workspace(&self) -> Workspace {
            Workspace::new(vec![self.0.clone()]).with_index(self.0.join("no-index.json"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn card(scratch: &Scratch, index: usize) -> Value {
        let uri = uri::slide(scratch.path(), index, SlideView::Card);

        read(&scratch.workspace(), &uri, scratch.path(), index).expect("a card")[0].clone()
    }

    #[test]
    fn a_deck_that_has_not_been_built_still_answers_with_a_picture() {
        // The SVG slidx drew, from the deck's own theme tokens. A resource that
        // said "build first" would be useless at exactly the moment somebody is
        // writing the deck.
        let scratch = Scratch::new("svg");
        let contents = card(&scratch, 0);

        assert_eq!(contents["mimeType"], "image/svg+xml");
        assert!(contents["text"].as_str().expect("markup").starts_with("<svg"));
        assert!(contents["text"].as_str().expect("markup").contains("Making Decks Fast"));
    }

    #[test]
    fn a_built_deck_answers_with_the_png_the_build_rasterised() {
        // Which is the one a model can actually look at: almost nothing renders
        // SVG as an image.
        let scratch = Scratch::new("png");
        scratch.built("og-1.png", b"\x89PNG\r\n\x1a\nnot really a png");

        let contents = card(&scratch, 0);

        assert_eq!(contents["mimeType"], "image/png");
        assert!(contents["text"].is_null(), "an image is a blob, not text");
        assert_eq!(contents["blob"], base64(b"\x89PNG\r\n\x1a\nnot really a png"));
    }

    #[test]
    fn the_card_for_slide_zero_is_the_file_the_plugin_named_one() {
        // The plugin numbers cards from one, because that is how a person counts
        // slides. Getting this off by one serves the wrong slide's picture,
        // which is worse than serving none.
        let scratch = Scratch::new("offset");
        scratch.built("og-1.png", b"first");
        scratch.built("og-2.png", b"second");

        assert_eq!(card(&scratch, 0)["blob"], base64(b"first"));
    }

    #[test]
    fn a_slide_that_is_not_there_says_how_many_there_are() {
        let scratch = Scratch::new("missing");
        let uri = uri::slide(scratch.path(), 9, SlideView::Card);

        let refusal =
            read(&scratch.workspace(), &uri, scratch.path(), 9).expect_err("no such slide");
        assert!(refusal.contains("numbered from zero"), "{refusal}");
    }

    #[test]
    fn base64_encodes_the_way_every_decoder_expects() {
        // Including the padding, which is not optional in this encoding: a blob
        // a client cannot decode is an image nobody sees.
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_survives_bytes_that_are_not_text() {
        // A PNG is full of them, and an encoder that went through a string would
        // have mangled every one.
        let bytes: Vec<u8> = (0u8..=255).collect();
        let encoded = base64(&bytes);

        assert_eq!(encoded.len(), 344, "256 bytes is 344 characters with padding");
        assert!(encoded.chars().all(|c| c.is_ascii_alphanumeric() || "+/=".contains(c)));
    }
}
