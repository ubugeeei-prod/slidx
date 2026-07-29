//! Putting a link on a slide that a phone can take.
//!
//! An audience cannot type a URL off a screen. They can point a camera at it,
//! and they will — but only if the code is on screen long enough and large
//! enough to scan from the back of the room, which is the constraint that
//! shapes everything here.
//!
//! # Why the code is drawn rather than fetched
//!
//! Every QR service is a URL, and a slide that fetches one is a slide that is
//! blank when the venue Wi-Fi is down. It is also a slide that tells a third
//! party who is presenting what, to whom, and when. Encoding locally costs a
//! few hundred lines and removes both.

use slidx_qr::{encode, Ecc, QrOptions, SvgOptions};
use slidx_theme::{Scheme, Theme};

/// How to draw a code onto a slide.
#[derive(Debug, Clone)]
pub struct SlideQrOptions {
    /// Error correction. Higher survives more of the code being obscured.
    pub ecc: Ecc,
    /// Optional caption under the code, usually the URL in readable text.
    pub caption: Option<String>,
}

impl Default for SlideQrOptions {
    fn default() -> Self {
        // Quartile, not Low. A projected code is scanned at an angle, through
        // a phone camera, sometimes with a head in the way — and the recovery
        // this buys costs a version bump nobody can see from row twelve.
        Self { ecc: Ecc::Quartile, caption: None }
    }
}

/// Renders a link as an inline SVG figure.
///
/// Returns `None` when the text cannot be encoded, so a caller can fall back
/// to showing the URL rather than a broken image. A code that does not scan
/// and a missing code are the same to an audience; a *wrong* code is worse.
pub fn render_qr(text: &str, theme: &Theme, options: &SlideQrOptions) -> Option<String> {
    let code = encode(text, &QrOptions::new(options.ecc)).ok()?;
    let palette = theme.palette(Scheme::Light);

    // A code is scanned by a camera, which needs dark-on-light regardless of
    // the theme. Inverting for a dark deck produces a code most readers
    // refuse, so the tile carries its own light background.
    let svg = code.to_svg(&SvgOptions::default().with_background(palette.surface.to_hex()));

    let caption = options
        .caption
        .as_ref()
        .map(|text| {
            format!("\n  <figcaption class=\"slidx-qr-caption\">{}</figcaption>", escape(text))
        })
        .unwrap_or_default();

    Some(format!("<figure class=\"slidx-qr\">\n  {svg}{caption}\n</figure>"))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        slidx_theme::default_theme()
    }

    #[test]
    fn a_link_becomes_a_scannable_figure() {
        let svg = render_qr("https://example.com/talk", &theme(), &SlideQrOptions::default())
            .expect("a short URL encodes");

        assert!(svg.contains("<svg"));
        assert!(svg.contains("slidx-qr"));
    }

    #[test]
    fn the_code_is_drawn_rather_than_fetched() {
        // A slide that fetches its own QR code is blank when the venue Wi-Fi
        // is down, and tells a third party who is presenting what to whom.
        let svg = render_qr("https://example.com", &theme(), &SlideQrOptions::default()).unwrap();

        assert!(!svg.contains("<img"));
        assert!(!svg.contains("api.qrserver"));
        assert!(!svg.contains("chart.googleapis"));
    }

    #[test]
    fn it_corrects_hard_enough_for_a_projected_code() {
        // Scanned at an angle, through a phone camera, sometimes with a head
        // in the way.
        assert_eq!(SlideQrOptions::default().ecc, Ecc::Quartile);
    }

    #[test]
    fn the_tile_stays_light_whatever_the_theme() {
        // Cameras want dark-on-light. Inverting for a dark deck produces a
        // code most readers refuse.
        let dark = slidx_theme::resolve("terminal").unwrap();
        let svg = render_qr("https://example.com", &dark, &SlideQrOptions::default()).unwrap();

        assert!(svg.contains(&dark.palette(Scheme::Light).surface.to_hex()));
    }

    #[test]
    fn a_caption_shows_the_url_in_words() {
        // Someone at the back with no phone still needs to be able to write
        // it down.
        let options = SlideQrOptions {
            caption: Some("example.com/talk".to_string()),
            ..SlideQrOptions::default()
        };
        let svg = render_qr("https://example.com/talk", &theme(), &options).unwrap();

        assert!(svg.contains("example.com/talk"));
        assert!(svg.contains("figcaption"));
    }

    #[test]
    fn text_too_long_to_encode_reports_rather_than_drawing_nothing() {
        // A code that does not scan and a missing code are the same to an
        // audience. A wrong one is worse, so this refuses instead.
        let long = "https://example.com/".to_string() + &"x".repeat(2000);

        assert!(render_qr(&long, &theme(), &SlideQrOptions::default()).is_none());
    }

    #[test]
    fn a_caption_containing_markup_is_escaped() {
        let options = SlideQrOptions {
            caption: Some("a <script> b".to_string()),
            ..SlideQrOptions::default()
        };
        let svg = render_qr("https://example.com", &theme(), &options).unwrap();

        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&lt;script&gt;"));
    }

    #[test]
    fn the_same_link_always_draws_the_same_code() {
        let first = render_qr("https://example.com", &theme(), &SlideQrOptions::default());
        let second = render_qr("https://example.com", &theme(), &SlideQrOptions::default());

        assert_eq!(first, second);
    }

    #[test]
    fn a_japanese_url_encodes() {
        assert!(
            render_qr("https://example.com/発表", &theme(), &SlideQrOptions::default()).is_some()
        );
    }
}
