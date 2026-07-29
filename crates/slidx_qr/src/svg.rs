//! Drawing a finished code as an SVG a slide can embed.
//!
//! Self-contained, with no external reference of any kind: a deck is opened
//! offline, from a file, and behind conference wifi, and a code that has to
//! fetch something is a code that is sometimes blank in front of an audience.
//!
//! The geometry is one unit per module inside a `viewBox`, so the slide decides
//! the size and the SVG never carries a pixel dimension of its own. The
//! foreground is `currentColor`, which puts the code under the theme's control
//! rather than hard-coding a black that disappears on a dark deck.

use std::fmt::Write as _;

use crate::code::QrCode;
use crate::options::SvgOptions;

pub(crate) fn render(code: &QrCode, options: &SvgOptions) -> String {
    let quiet = options.effective_quiet_zone() as usize;
    let extent = code.size() + quiet * 2;

    let mut svg = String::with_capacity(code.size() * code.size() / 2);

    // `crispEdges` because a module boundary landing between device pixels is
    // antialiased into a grey band, and grey is what a reader has to threshold.
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {extent} {extent}\" \
         shape-rendering=\"crispEdges\" role=\"img\">"
    );

    if let Some(title) = &options.title {
        let _ = write!(svg, "<title>{}</title>", escape(title));
    }

    if let Some(background) = &options.background {
        let _ = write!(
            svg,
            "<rect width=\"{extent}\" height=\"{extent}\" fill=\"{}\"/>",
            escape(background)
        );
    }

    let _ = write!(svg, "<path fill=\"currentColor\" d=\"{}\"/>", path_data(code, quiet));

    svg.push_str("</svg>");

    svg
}

/// The dark modules as a single path, one subpath per horizontal run.
///
/// One path rather than one rect per module, and runs rather than squares,
/// because a version 10 code is 3249 modules: the naive form is tens of
/// kilobytes of markup inlined into every slide that carries a link.
fn path_data(code: &QrCode, quiet: usize) -> String {
    let mut data = String::new();

    for row in 0..code.size() {
        let mut column = 0;
        while column < code.size() {
            if !code.module(row, column) {
                column += 1;
                continue;
            }

            let start = column;
            while column < code.size() && code.module(row, column) {
                column += 1;
            }

            let length = column - start;
            let _ = write!(data, "M{} {}h{length}v1h-{length}z", start + quiet, row + quiet);
        }
    }

    data
}

/// Escapes text for an XML attribute or text node.
///
/// A title is author-supplied and a background may be too; either can carry a
/// quote or an ampersand that would otherwise end the attribute early and
/// produce markup the browser reinterprets.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{Ecc, QrOptions};

    fn code() -> QrCode {
        crate::encode("https://slidx.dev", &QrOptions::new(Ecc::Medium)).unwrap()
    }

    #[test]
    fn the_view_box_covers_the_code_and_its_quiet_zone_on_both_sides() {
        // A viewBox that omitted the margin would crop the quiet zone away the
        // moment the slide scaled the code down.
        let svg = render(&code(), &SvgOptions::default());
        let extent = code().size() + 8;

        assert!(svg.contains(&format!("viewBox=\"0 0 {extent} {extent}\"")), "{svg}");
    }

    #[test]
    fn the_quiet_zone_is_actually_empty_of_modules() {
        // Declaring the margin in the viewBox is not the same as leaving it
        // clear; a module drawn at x=0 would sit flush against slide content.
        let svg = render(&code(), &SvgOptions::default());

        assert!(!svg.contains("d=\"M0 "), "a run starts at the very left edge");
        assert!(svg.contains("M4 4"), "the top-left finder starts one quiet zone in");
    }

    #[test]
    fn a_wider_quiet_zone_moves_the_code_and_grows_the_view_box() {
        let svg = render(&code(), &SvgOptions::default().with_quiet_zone(10));
        let extent = code().size() + 20;

        assert!(svg.contains(&format!("viewBox=\"0 0 {extent} {extent}\"")), "{svg}");
        assert!(svg.contains("M10 10"), "{svg}");
    }

    #[test]
    fn the_svg_carries_no_size_of_its_own() {
        // Width and height attributes would fight the slide's layout; the
        // viewBox alone lets the deck decide how large the code is.
        let svg = render(&code(), &SvgOptions::default());

        assert!(!svg.contains("width=\"") || svg.contains("<rect"), "{svg}");
        assert!(!svg.contains("<svg xmlns=\"http://www.w3.org/2000/svg\" width"), "{svg}");
    }

    #[test]
    fn the_foreground_is_current_color_so_the_theme_owns_it() {
        let svg = render(&code(), &SvgOptions::default());

        assert!(svg.contains("fill=\"currentColor\""));
        assert!(!svg.contains("#000"), "a hard-coded black disappears on a dark theme");
    }

    #[test]
    fn there_is_no_background_unless_one_is_asked_for() {
        // Transparent by default so the code inherits the theme's surface, and
        // with it the contrast the theme already guarantees.
        assert!(!render(&code(), &SvgOptions::default()).contains("<rect"));
        assert!(render(&code(), &SvgOptions::default().with_background("#fff"))
            .contains("<rect width="));
    }

    #[test]
    fn the_svg_references_nothing_outside_itself() {
        // A deck is opened offline and from a file. Any external reference is a
        // code that is sometimes blank in front of an audience.
        let svg =
            render(&code(), &SvgOptions::default().with_title("slidx").with_background("#fff"));

        for reference in ["http://", "https://", "url(", "xlink:href", "<image", "<use", "@import"]
        {
            assert!(
                !svg.replace("http://www.w3.org/2000/svg", "").contains(reference),
                "{reference} found in {svg}"
            );
        }
    }

    #[test]
    fn a_title_gives_the_code_an_accessible_name() {
        // A QR code is a link with no visible text; without this a screen
        // reader announces nothing at all.
        let svg = render(&code(), &SvgOptions::default().with_title("Slides and notes"));

        assert!(svg.contains("<title>Slides and notes</title>"));
        assert!(svg.contains("role=\"img\""));
    }

    #[test]
    fn author_supplied_text_cannot_break_out_of_the_markup() {
        // The title comes from a deck, and a deck is text an author wrote; an
        // unescaped quote turns the rest of the slide into attributes.
        let svg = render(
            &code(),
            &SvgOptions::default()
                .with_title("a & b <script>\"x\"")
                .with_background("\"/><script>"),
        );

        assert!(!svg.contains("<script>"), "{svg}");
        assert!(svg.contains("a &amp; b &lt;script&gt;&quot;x&quot;"), "{svg}");
    }

    #[test]
    fn horizontal_runs_are_merged_into_one_subpath() {
        // A rect per module is tens of kilobytes for a version 10 code, inlined
        // into every slide that carries a link.
        let svg = render(&code(), &SvgOptions::default());

        assert!(svg.contains("h7v1h-7z"), "the finder's dark row should be one run: {svg}");
        assert_eq!(svg.matches("<path").count(), 1);
    }

    #[test]
    fn rendering_the_same_code_twice_produces_identical_markup() {
        let options = SvgOptions::default().with_title("slidx");

        assert_eq!(render(&code(), &options), render(&code(), &options));
    }
}
