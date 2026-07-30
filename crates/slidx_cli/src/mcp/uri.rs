//! The `slidx://` scheme: how a resource is named.
//!
//! One module because a URI that is built one way and parsed another is a
//! resource a client can list and never read, and nothing about that failure
//! says which half is wrong.
//!
//! ## Why the project path is percent-encoded
//!
//! A project is named by its absolute path, which contains the one character
//! that separates the segments of this URI — and, on Windows, a colon. Encoding
//! it is what keeps `slidx://deck/{project}/slide/{index}/source` a template with
//! exactly four segments however deep the project is buried.
//!
//! ## Slides count from zero
//!
//! The same as the editing tools, and deliberately not the same as the linter's
//! report, which says "slide 2" because nobody counts slides from zero out loud.
//! An agent moves between a resource and a tool in one breath, and a URI that
//! disagreed with the argument beside it would be wrong every other call.

use std::path::{Path, PathBuf};

/// Every deck this machine knows about.
pub const INDEX: &str = "slidx://index";

/// What a `slidx://` URI names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource {
    /// The decks this machine has seen.
    Index,
    /// A whole deck, in one of its shapes.
    Deck { project: PathBuf, view: DeckView },
    /// One slide, in one of its shapes.
    Slide { project: PathBuf, index: usize, view: SlideView },
}

/// The shapes a deck is served in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckView {
    /// The parsed deck: metadata, slides, marks, steps.
    Model,
    /// Parse diagnostics and lint findings, worst first.
    Diagnostics,
    /// The compiled step timeline: every stop of every slide, as a full state.
    Timeline,
}

/// The shapes one slide is served in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideView {
    /// The Markdown, exactly as the author wrote it.
    Source,
    /// The rendered HTML, step anchors included.
    Html,
    /// The social card, as an image.
    Card,
}

impl DeckView {
    fn token(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Diagnostics => "diagnostics",
            Self::Timeline => "timeline",
        }
    }

    fn parse(token: &str) -> Option<Self> {
        match token {
            "model" => Some(Self::Model),
            "diagnostics" => Some(Self::Diagnostics),
            "timeline" => Some(Self::Timeline),
            _ => None,
        }
    }
}

impl SlideView {
    fn token(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Html => "html",
            Self::Card => "card",
        }
    }

    fn parse(token: &str) -> Option<Self> {
        match token {
            "source" => Some(Self::Source),
            "html" => Some(Self::Html),
            "card" => Some(Self::Card),
            _ => None,
        }
    }
}

/// The URI for one of a deck's shapes.
pub fn deck(project: &Path, view: DeckView) -> String {
    format!("slidx://deck/{}/{}", encode(&project.display().to_string()), view.token())
}

/// The URI for one of a slide's shapes.
pub fn slide(project: &Path, index: usize, view: SlideView) -> String {
    format!(
        "slidx://deck/{}/slide/{index}/{}",
        encode(&project.display().to_string()),
        view.token()
    )
}

/// What a URI names, or `None` when it names nothing this server serves.
pub fn parse(uri: &str) -> Option<Resource> {
    if uri == INDEX {
        return Some(Resource::Index);
    }

    let rest = uri.strip_prefix("slidx://deck/")?;
    let mut segments = rest.split('/');
    let project = PathBuf::from(decode(segments.next()?));

    match (segments.next()?, segments.next(), segments.next(), segments.next()) {
        (view, None, None, None) => Some(Resource::Deck { project, view: DeckView::parse(view)? }),
        ("slide", Some(index), Some(view), None) => Some(Resource::Slide {
            project,
            index: index.parse().ok()?,
            view: SlideView::parse(view)?,
        }),
        _ => None,
    }
}

/// Percent-encodes everything that is not unreserved.
///
/// The unreserved set from RFC 3986, and nothing else — a Windows drive letter's
/// colon and every path separator have to survive a round trip through a URI
/// segment, and the cheapest way to be sure is to encode anything that is not
/// plainly safe.
fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());

    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

/// Reverses [`encode`], leaving anything malformed as it was.
///
/// A URI a client mangled should read as a project that does not exist rather
/// than as a failure to parse: the answer is the same either way, and one of
/// them names the path it could not find.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;

    while at < bytes.len() {
        if bytes[at] == b'%' && at + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[at + 1..at + 3], 16) {
                decoded.push(byte);
                at += 3;
                continue;
            }
        }

        decoded.push(bytes[at]);
        at += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deck_uri_round_trips_through_its_own_parser() {
        // The failure this prevents: a resource a client can list and never
        // read, with nothing saying which half is wrong.
        let project = PathBuf::from("/Users/somebody/talks/vue fes");

        for view in [DeckView::Model, DeckView::Diagnostics, DeckView::Timeline] {
            let uri = deck(&project, view);

            assert_eq!(
                parse(&uri),
                Some(Resource::Deck { project: project.clone(), view }),
                "{uri}"
            );
        }
    }

    #[test]
    fn a_slide_uri_round_trips_through_its_own_parser() {
        let project = PathBuf::from("/talks/vueconf");

        for view in [SlideView::Source, SlideView::Html, SlideView::Card] {
            let uri = slide(&project, 7, view);

            assert_eq!(
                parse(&uri),
                Some(Resource::Slide { project: project.clone(), index: 7, view }),
                "{uri}"
            );
        }
    }

    #[test]
    fn a_projects_path_separators_do_not_become_uri_segments() {
        // Otherwise the template has as many segments as the project is deep,
        // and a client cannot fill it in at all.
        let uri = deck(Path::new("/a/b/c"), DeckView::Model);

        assert_eq!(uri, "slidx://deck/%2Fa%2Fb%2Fc/model");

        let segments: Vec<&str> = uri.trim_start_matches("slidx://").split('/').collect();
        assert_eq!(segments.len(), 3, "deck, project, view — however deep the project is: {uri}");
    }

    #[test]
    fn a_windows_path_survives_the_round_trip() {
        // The colon in a drive letter is reserved, and a space in a folder name
        // is ordinary on that platform.
        let project = PathBuf::from(r"C:\Users\somebody\My Talks\vueconf");
        let uri = deck(&project, DeckView::Model);

        assert!(!uri.contains(':') || uri.starts_with("slidx:"), "{uri}");
        assert_eq!(parse(&uri), Some(Resource::Deck { project, view: DeckView::Model }));
    }

    #[test]
    fn a_japanese_project_name_survives_the_round_trip() {
        let project = PathBuf::from("/talks/高速なデッキ");

        assert_eq!(
            parse(&deck(&project, DeckView::Model)),
            Some(Resource::Deck { project, view: DeckView::Model })
        );
    }

    #[test]
    fn the_index_is_its_own_uri() {
        assert_eq!(parse(INDEX), Some(Resource::Index));
    }

    #[test]
    fn a_uri_this_server_does_not_serve_is_none_rather_than_a_guess() {
        for uri in [
            "https://example.com",
            "slidx://nothing",
            "slidx://deck/%2Fa/nonsense",
            "slidx://deck/%2Fa/slide/two/source",
            "slidx://deck/%2Fa/slide/1/source/extra",
        ] {
            assert_eq!(parse(uri), None, "{uri}");
        }
    }

    #[test]
    fn a_mangled_escape_reads_as_a_project_that_does_not_exist() {
        // Rather than as a parse failure. The answer is the same either way and
        // this one can name the path it could not find.
        assert!(parse("slidx://deck/%zz/model").is_some());
    }
}
