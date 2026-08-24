//! An image's own size, written onto the tag that draws it.
//!
//! A browser does not know how tall an image is until enough of it has arrived,
//! so a slide with one reflows when it lands: the heading jumps, the bullets
//! move, and on the wifi a venue has rather than the one it advertises, that
//! happens while a room is reading. `width` and `height` on the tag are what
//! stop it — a browser reserves the box from the ratio before a byte of the
//! file is there.
//!
//! # The build already knew
//!
//! This is not a new measurement. The Vite plugin reads every image's header
//! and hands the sizes across the WebAssembly boundary, because the linter
//! cannot open a file and needs them to say whether an image is too small for
//! the size it is drawn at. The same map answers this question, and the answer
//! was being thrown away.
//!
//! # What it refuses to touch
//!
//! **A tag that already carries either attribute.** An author who wrote
//! `width` means it, and pairing their number with an intrinsic height would
//! silently distort the image.
//!
//! **An image nobody measured.** A remote URL is a lint error under the offline
//! guarantee, and an SVG has no pixel size worth writing. Absent from the map
//! means absent from the tag, which is the same rule every other reading in
//! this project follows: no measurement, no claim.
//!
//! **A reference that names no file under the deck.** A remote URL is a lint
//! error under the offline guarantee and a `data:` URI carries its own bytes.
//! `slidx_core::asset::key` decides which references have a name here, because
//! the linter asks the same question of the same references.
//!
//! **The rendered box.** These are the file's own dimensions and not a
//! placement — CSS still decides how large the image is drawn, and the
//! attributes exist so the *ratio* is known early. Writing a layout number here
//! would be the one thing that could make a slide wrong rather than jumpy.

use std::collections::BTreeMap;

/// Every image the caller measured, keyed the way a slide writes the path.
pub type Sizes = BTreeMap<String, (u32, u32)>;

/// Adds each measured image's own dimensions to the tag that draws it.
pub fn size_images(html: &str, sizes: &Sizes) -> String {
    if sizes.is_empty() || !html.contains("<img") {
        return html.to_string();
    }

    let mut out = String::with_capacity(html.len() + sizes.len() * 24);
    let mut rest = html;

    while let Some(at) = rest.find("<img ") {
        let (before, from) = rest.split_at(at);
        out.push_str(before);

        let Some(end) = from.find('>') else {
            out.push_str(from);
            return out;
        };

        let (tag, after) = from.split_at(end);
        out.push_str(&sized(tag, sizes));
        rest = after;
    }

    out.push_str(rest);
    out
}

/// One tag, with its size added when there is one to add and room to add it.
fn sized(tag: &str, sizes: &Sizes) -> String {
    if tag.contains(" width=") || tag.contains(" height=") {
        return tag.to_string();
    }

    let Some(source) = attribute(tag, "src") else { return tag.to_string() };
    // `slidx_core::asset::key` and not a normalisation of its own. The linter
    // asks the same question of the same references, and the two answering it
    // differently is what #307 was.
    let Some(key) = slidx_core::asset::key(source) else { return tag.to_string() };
    let Some((width, height)) = sizes.get(&key) else { return tag.to_string() };

    format!("{tag} width=\"{width}\" height=\"{height}\"")
}

/// The value of a double-quoted attribute, which is the only form emitted here.
///
/// The HTML this runs over is written by one renderer rather than typed by a
/// person, so there is no single-quoted or bare form to handle — and guessing
/// at one would be a parser pretending to be a parser.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let opening = format!(" {name}=\"");
    let at = tag.find(&opening)? + opening.len();

    tag[at..].find('"').map(|end| &tag[at..at + end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(path: &str, width: u32, height: u32) -> Sizes {
        Sizes::from([(path.to_string(), (width, height))])
    }

    #[test]
    fn a_measured_image_carries_its_own_size() {
        // The whole point. A browser reserves the box from the ratio before a
        // byte of the file has arrived, so the heading does not jump.
        let html = size_images(
            r#"<p><img src="./chart.png" alt="A chart"></p>"#,
            &one("chart.png", 1200, 900),
        );

        assert_eq!(
            html,
            r#"<p><img src="./chart.png" alt="A chart" width="1200" height="900"></p>"#
        );
    }

    #[test]
    fn the_ordinary_way_to_write_a_relative_path_is_the_way_it_is_keyed() {
        // `readAssetSizes` keys on the path relative to the deck's directory,
        // and a slide writes `./chart.png`. An exact lookup would miss the most
        // common form there is.
        for written in ["./chart.png", "chart.png", "/chart.png", "./chart.png?width=800"] {
            let html =
                size_images(&format!(r#"<img src="{written}">"#), &one("chart.png", 1200, 900));

            assert!(html.contains(r#"width="1200""#), "{written} was not sized: {html}");
        }
    }

    #[test]
    fn an_image_nobody_measured_is_left_alone() {
        // No measurement, no claim — the rule every other reading here follows.
        let html = r#"<img src="./unknown.png" alt="">"#;

        assert_eq!(size_images(html, &one("chart.png", 10, 10)), html);
    }

    #[test]
    fn an_author_who_wrote_a_width_means_it() {
        // Pairing their number with an intrinsic height would silently distort
        // the image, which is worse than the reflow this exists to stop.
        let html = r#"<img src="./chart.png" width="600">"#;

        assert_eq!(size_images(html, &one("chart.png", 1200, 900)), html);
    }

    #[test]
    fn a_height_alone_is_the_same_case() {
        let html = r#"<img src="./chart.png" height="450">"#;

        assert_eq!(size_images(html, &one("chart.png", 1200, 900)), html);
    }

    #[test]
    fn every_image_on_a_slide_is_sized() {
        let sizes = Sizes::from([("a.png".to_string(), (10, 20)), ("b.png".to_string(), (30, 40))]);
        let html = size_images(r#"<img src="./a.png"><img src="./b.png">"#, &sizes);

        assert_eq!(
            html,
            r#"<img src="./a.png" width="10" height="20"><img src="./b.png" width="30" height="40">"#
        );
    }

    #[test]
    fn nothing_measured_means_nothing_written() {
        let html = r#"<img src="./chart.png">"#;

        assert_eq!(size_images(html, &Sizes::new()), html);
    }

    #[test]
    fn a_body_with_no_image_comes_back_unchanged() {
        let html = "<h1>One</h1>\n<p>Words.</p>";

        assert_eq!(size_images(html, &one("chart.png", 1, 1)), html);
    }

    #[test]
    fn a_tag_that_was_never_closed_is_not_rewritten_into_something_worse() {
        // The renderer does not produce this. It is here because a rewriter
        // that guesses at the end of a tag is a rewriter that can corrupt a
        // slide, and a slide is what a room is looking at.
        let html = r#"<p><img src="./chart.png" alt="unterminated"#;

        assert_eq!(size_images(html, &one("chart.png", 1, 1)), html);
    }
}
