//! The offline guarantee.
//!
//! slidx promises that a built deck makes zero network requests. The promise is
//! not about purity — it is about the room. The venue Wi-Fi is down, the laptop
//! is someone else's, the deck opens, and the fonts were on a CDN. Every asset a
//! slide fetches at presentation time is a way for the deck to arrive blank.
//!
//! Nothing else can enforce this. The bundler inlines what it can see, so the
//! guarantee holds by construction right up to the moment an author pastes a
//! Google Fonts `<link>` or a CDN `<img>` into a slide — a reference the bundler
//! has no way to fix and every reason to pass through untouched. That is the
//! case this rule exists to catch, and the reason the claim can be made at all.
//!
//! # Fetching versus navigating
//!
//! The whole rule turns on one distinction. `<img src="https://…">` is fetched
//! while the slide is on screen; `<a href="https://…">` is not. A link is a
//! destination the audience may visit later, from their own machine, on their
//! own network — it costs the deck nothing to carry. Flagging links would make
//! the rule unusable, because decks cite sources, and would protect nobody.
//!
//! So the element and the attribute together decide, not the URL: `href` on
//! `<link>` is a fetch and `href` on `<a>` is a promise to someone else.

use slidx_core::scanner::FenceTracker;
use slidx_core::{Diagnostic, Diagnostics, Severity, Slide, SourceSpan};

use crate::{LintInput, LintOptions};

pub fn check(input: &LintInput<'_>, _options: &LintOptions, sink: &mut Diagnostics) {
    for slide in &input.deck.slides {
        let content = scannable(slide);
        let mut found = Vec::new();

        markdown_images(&content, &mut found);
        html(&content, &mut found);

        for reference in found.iter().filter(|reference| is_remote(reference.url)) {
            sink.push(report(slide, &content, reference));
        }
    }
}

/// Something the page would pull over the network while a slide is on screen.
#[derive(Debug, Clone, Copy)]
struct Reference<'a> {
    url: &'a str,
    /// Byte offset into the slide body, so the diagnostic can name a line.
    offset: usize,
    kind: Kind,
}

/// What is being fetched, which decides the wording and the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Image,
    Script,
    Stylesheet,
    Media,
    /// A whole nested document: `<iframe>`, `<object>`, `<embed>`.
    Embed,
    /// A `url(…)` in CSS — a font, a background, anything.
    Asset,
}

impl Kind {
    /// Embeds carry their own code so that a deck with one deliberate live
    /// demo can allow that one thing without switching off the font and image
    /// guarantee for every other slide.
    fn code(self) -> &'static str {
        match self {
            Self::Embed => "offline/remote-embed",
            _ => "offline/remote-asset",
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Script => "script",
            Self::Stylesheet => "stylesheet",
            Self::Media => "media",
            Self::Embed => "embed",
            Self::Asset => "asset",
        }
    }

    fn help(self) -> &'static str {
        // A live document has nothing to copy into the bundle, so the only
        // honest advice is to stop depending on it being reachable.
        if self == Self::Embed {
            return "bundle a recording or a screenshot next to the deck; if the demo has to be \
                    live, allow `offline/remote-embed` and prepare a slide to fall back to";
        }

        "copy the file next to the deck and reference it with a relative path, or inline it as a \
         `data:` URI"
    }
}

fn report(slide: &Slide, content: &str, reference: &Reference<'_>) -> Diagnostic {
    let line = content.as_bytes()[..reference.offset].iter().filter(|&&b| b == b'\n').count();

    Diagnostic::new(
        reference.kind.code(),
        // The guarantee is the product, so breaking it stops the build rather
        // than adding to a list of things to look at later.
        Severity::Error,
        format!(
            "{} on \"{}\" loads {} from the network",
            reference.kind.noun(),
            slide.display_title(),
            reference.url
        ),
    )
    .at(SourceSpan::line(slide.source_line + line as u32).on_slide(slide.index))
    .with_help(reference.kind.help())
}

/// The slide body with fenced code blanked out.
///
/// Blanking rather than deleting keeps every byte where it was, so an offset
/// still resolves to the line it came from. Fences are blanked because a fence
/// is displayed, not fetched: a talk about CDNs has to be able to show
/// `<img src="https://…">` on a slide without failing its own build.
fn scannable(slide: &Slide) -> String {
    let mut fences = FenceTracker::new();
    let mut scannable = String::with_capacity(slide.content.len());

    for line in slide.content.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);

        if fences.feed(body) {
            scannable.push_str(line);
        } else {
            scannable.push_str(&" ".repeat(body.len()));
            scannable.push_str(&line[body.len()..]);
        }
    }

    scannable
}

/// Collects `![alt](url)`.
///
/// The link form `[text](url)` is deliberately not scanned: it navigates.
fn markdown_images<'a>(content: &'a str, found: &mut Vec<Reference<'a>>) {
    for (start, _) in content.match_indices("![") {
        let alt = &content[start + 2..];
        let Some(close) = alt.find(']') else { continue };

        let destination = &alt[close + 1..];
        if !destination.starts_with('(') {
            continue;
        }
        let Some(end) = destination.find(')') else { continue };

        let offset = start + 2 + close + 2;
        found.push(Reference { url: target(&destination[1..end]), offset, kind: Kind::Image });
    }
}

/// The URL of a Markdown destination, without its `"title"` or `<>` wrapper.
fn target(destination: &str) -> &str {
    let url = destination.split_whitespace().next().unwrap_or("");
    url.trim_start_matches('<').trim_end_matches('>')
}

/// Walks HTML tags and reports the attributes that cause a fetch.
///
/// Not an HTML parser and does not need to be. It needs the element name, the
/// attributes, and quoted values to stay opaque so that a `>` inside alt text
/// cannot end a tag early and hide the `src` that follows it.
fn html<'a>(content: &'a str, found: &mut Vec<Reference<'a>>) {
    let mut at = 0;

    while let Some(open) = content[at..].find('<') {
        let start = at + open + 1;
        at = start;

        let Some((tag, mut cursor)) = tag_name(content, start) else { continue };

        while let Some(attribute) = attribute(content, cursor) {
            cursor = attribute.next;
            collect(tag, &attribute, found);
        }

        // Past the `>` that closed the open tag, when the author wrote one.
        at = cursor + usize::from(content[cursor..].starts_with('>'));

        // A `<style>` body is CSS rather than markup, and CSS has its own way
        // of reaching the network.
        if tag.eq_ignore_ascii_case("style") {
            let end = content[at..].find("</style>").map_or(content.len(), |offset| at + offset);
            css(&content[at..end], at, found);
            at = end;
        }
    }
}

/// The element name just past a `<`, and the offset after it.
fn tag_name(content: &str, start: usize) -> Option<(&str, usize)> {
    let rest = &content[start..];
    if !rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }

    let len = rest.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(rest.len());
    Some((&rest[..len], start + len))
}

/// One attribute inside an open tag.
#[derive(Debug, Clone, Copy)]
struct Attribute<'a> {
    name: &'a str,
    value: &'a str,
    /// Byte offset of the value, so a multi-line tag reports the right line.
    offset: usize,
    /// Where the scanner resumes.
    next: usize,
}

/// The next attribute inside an open tag.
///
/// `None` means the element is finished — at its `>`, at the end of the slide,
/// or at the start of the next tag when the author never closed this one.
fn attribute(content: &str, at: usize) -> Option<Attribute<'_>> {
    // A `/` here belongs to `<br />` rather than to a name.
    let start = skip_while(content, at, |c| c.is_whitespace() || c == '/');
    let rest = &content[start..];
    if rest.is_empty() || rest.starts_with(['>', '<']) {
        return None;
    }

    let len = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '=' | '>' | '<' | '/'))
        .unwrap_or(rest.len());
    if len == 0 {
        return None;
    }

    let name = &rest[..len];
    let after_name = skip_while(content, start + len, char::is_whitespace);
    if !content[after_name..].starts_with('=') {
        // A valueless attribute such as `controls`.
        return Some(Attribute { name, value: "", offset: start, next: start + len });
    }

    let offset = skip_while(content, after_name + 1, char::is_whitespace);
    let (value, next) = attribute_value(content, offset);
    Some(Attribute { name, value, offset, next })
}

/// An attribute value: quoted, or a bare token up to whitespace or `>`.
fn attribute_value(content: &str, at: usize) -> (&str, usize) {
    let rest = &content[at..];

    for quote in ['"', '\''] {
        if let Some(body) = rest.strip_prefix(quote) {
            let end = body.find(quote).unwrap_or(body.len());
            return (&body[..end], (at + end + 2).min(content.len()));
        }
    }

    let end =
        rest.find(|c: char| c.is_whitespace() || matches!(c, '>' | '<')).unwrap_or(rest.len());
    (&rest[..end], at + end)
}

/// Offset of the first character at or after `at` that `skip` rejects.
fn skip_while(content: &str, at: usize, skip: impl Fn(char) -> bool) -> usize {
    content.len() - content[at..].trim_start_matches(skip).len()
}

/// Records the fetch an attribute causes, if it causes one.
fn collect<'a>(tag: &str, attribute: &Attribute<'a>, found: &mut Vec<Reference<'a>>) {
    let tag = tag.to_ascii_lowercase();
    let name = attribute.name.to_ascii_lowercase();

    // A `style` attribute holds CSS, not a URL.
    if name == "style" {
        css(attribute.value, attribute.offset, found);
        return;
    }

    let Some(kind) = fetched_by(&tag, &name) else { return };

    for url in urls_in(&name, attribute.value) {
        found.push(Reference { url, offset: attribute.offset, kind });
    }
}

/// What a given attribute on a given element makes the browser fetch.
///
/// The `href` arm is the point of the rule: on `<link>` it is a stylesheet or a
/// font the page pulls in to render itself, and on `<a>` it is a destination
/// the audience might visit later. Same attribute, opposite consequences.
fn fetched_by(tag: &str, attribute: &str) -> Option<Kind> {
    let fetches = match attribute {
        "src" | "srcset" | "poster" => true,
        "href" => tag == "link",
        "data" => tag == "object",
        _ => false,
    };

    fetches.then(|| kind_of(tag))
}

/// What an element fetches, named the way an author would name it.
fn kind_of(tag: &str) -> Kind {
    match tag {
        "img" | "image" => Kind::Image,
        "script" => Kind::Script,
        "link" => Kind::Stylesheet,
        "iframe" | "object" | "embed" => Kind::Embed,
        "video" | "audio" | "source" | "track" => Kind::Media,
        _ => Kind::Asset,
    }
}

/// The URLs an attribute value holds.
///
/// Almost every attribute holds exactly one. `srcset` holds a comma-separated
/// list of `url descriptor` pairs, which has to be split before the entries are
/// judged — otherwise a remote 2x source hides behind a local 1x one.
fn urls_in<'a>(attribute: &str, value: &'a str) -> Vec<&'a str> {
    if attribute == "srcset" {
        return value.split(',').filter_map(|entry| entry.split_whitespace().next()).collect();
    }

    vec![value.trim()]
}

/// Scans CSS for the two ways a stylesheet reaches the network.
fn css<'a>(css: &'a str, offset: usize, found: &mut Vec<Reference<'a>>) {
    for (start, _) in css.match_indices("url(") {
        let rest = &css[start + "url(".len()..];
        let url = rest[..rest.find(')').unwrap_or(rest.len())].trim();

        found.push(Reference { url: unquote(url), offset: offset + start, kind: Kind::Asset });
    }

    for (start, _) in css.match_indices("@import") {
        // `@import url(…)` is already covered above; this is the bare form,
        // `@import "https://…"`, which is just as much of a request.
        if let Some(url) = quoted(css[start + "@import".len()..].trim_start()) {
            found.push(Reference { url, offset: offset + start, kind: Kind::Stylesheet });
        }
    }
}

/// A CSS token with its quotes removed, if it had any.
fn unquote(value: &str) -> &str {
    quoted(value).unwrap_or(value)
}

/// The contents of a leading quoted string.
fn quoted(value: &str) -> Option<&str> {
    for quote in ['"', '\''] {
        if let Some(body) = value.strip_prefix(quote) {
            return Some(&body[..body.find(quote).unwrap_or(body.len())]);
        }
    }

    None
}

/// True when resolving `url` would leave the bundle.
///
/// Relative paths and `data:` URIs are the two shapes that survive a dead
/// network: one is a file sitting next to the deck, the other is the file
/// itself. Anything on `http` or `https` is a request whoever the host is —
/// `localhost` included, because the machine that builds a deck is not the
/// machine that shows it.
pub(crate) fn is_remote(url: &str) -> bool {
    let url = url.trim();

    // `//cdn.example.com/x` is a URL with the scheme left off, not a path: the
    // browser fills in the page's own scheme and goes to the network anyway.
    if url.starts_with("//") {
        return true;
    }

    scheme(url).is_some_and(|scheme| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
}

/// The URL scheme, or `None` for a relative reference.
fn scheme(url: &str) -> Option<&str> {
    let (scheme, _) = url.split_once(':')?;

    // A scheme starts with a letter and continues with letters, digits, `+`,
    // `-`, or `.`. Anything else means the colon belongs to a path segment.
    let valid = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));

    valid.then_some(scheme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::lint_deck;
    use crate::{lint, LintInput, LintOptions, Surface};
    use slidx_core::{parse_deck, DeckParseOptions};

    /// Only what this rule reported — `lint_deck` runs every rule.
    fn offline(source: &str) -> Vec<Diagnostic> {
        lint_deck(source).into_iter().filter(|d| d.code.starts_with("offline/")).collect()
    }

    fn first(source: &str) -> Diagnostic {
        let diagnostics = offline(source);
        assert_eq!(diagnostics.len(), 1, "expected exactly one offender in: {source}");
        diagnostics[0].clone()
    }

    /// Lints with codes suppressed, the way a deck opts out in its config.
    fn allowing(source: &str, allow: &[&str]) -> Vec<Diagnostic> {
        let deck = parse_deck(source, &DeckParseOptions::default());
        let surfaces: Vec<Surface> = Vec::new();
        let options = LintOptions {
            allow: allow.iter().map(|code| code.to_string()).collect(),
            ..LintOptions::default()
        };

        lint(&LintInput::new(&deck, &surfaces), &options)
            .into_iter()
            .filter(|d| d.code.starts_with("offline/"))
            .collect()
    }

    #[test]
    fn a_deck_that_carries_its_own_assets_produces_nothing() {
        assert!(offline("# One\n\n![a chart](./chart.png)\n\n<img src=\"logo.svg\" alt=\"\">\n")
            .is_empty());
    }

    #[test]
    fn a_remote_markdown_image_is_reported() {
        let diagnostic = first("# One\n\n![a chart](https://cdn.example.com/chart.png)\n");
        assert_eq!(diagnostic.code, "offline/remote-asset");
    }

    #[test]
    fn a_remote_img_tag_is_reported() {
        let diagnostic =
            first("# One\n\n<img src=\"https://cdn.example.com/logo.svg\" alt=\"l\">\n");
        assert_eq!(diagnostic.code, "offline/remote-asset");
    }

    #[test]
    fn a_remote_reference_is_an_error_so_the_build_stops() {
        // The guarantee is the product. A warning would let the deck ship.
        assert_eq!(
            first("# One\n\n![c](https://cdn.example.com/c.png)\n").severity,
            Severity::Error
        );
    }

    #[test]
    fn a_google_fonts_link_is_reported() {
        // The failure in the project's own problem statement: the venue Wi-Fi
        // is down and the deck's fonts were on a CDN.
        let diagnostic = first(
            "# One\n\n<link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css2\">\n",
        );

        assert_eq!(diagnostic.code, "offline/remote-asset");
        assert!(diagnostic.message.contains("fonts.googleapis.com"));
    }

    #[test]
    fn a_preconnect_link_is_reported_because_it_opens_a_connection() {
        assert_eq!(
            first("<link rel=\"preconnect\" href=\"https://fonts.gstatic.com\">\n").severity,
            Severity::Error
        );
    }

    #[test]
    fn a_remote_script_is_reported() {
        assert!(first("<script src=\"https://cdn.example.com/chart.js\"></script>\n")
            .message
            .contains("script"));
    }

    #[test]
    fn a_remote_video_is_reported() {
        assert_eq!(
            first("<video src=\"https://cdn.example.com/demo.mp4\" controls></video>\n").code,
            "offline/remote-asset"
        );
    }

    #[test]
    fn a_remote_audio_is_reported() {
        assert_eq!(
            first("<audio src=\"https://cdn.example.com/clip.mp3\"></audio>\n").code,
            "offline/remote-asset"
        );
    }

    #[test]
    fn a_remote_source_inside_a_video_is_reported() {
        // The `src` an author actually writes for multi-format media sits on
        // `<source>`, not on `<video>`.
        let diagnostic = first(
            "<video controls>\n<source src=\"https://cdn.example.com/demo.webm\">\n</video>\n",
        );

        assert!(diagnostic.message.contains("demo.webm"));
    }

    #[test]
    fn a_remote_poster_is_reported() {
        // A local video with a remote still frame still blanks the slide.
        let diagnostic =
            first("<video src=\"./demo.mp4\" poster=\"https://cdn.example.com/p.jpg\">\n");
        assert!(diagnostic.message.contains("p.jpg"));
    }

    #[test]
    fn a_remote_subtitle_track_is_reported() {
        assert_eq!(
            first("<track src=\"https://cdn.example.com/captions.vtt\">\n").code,
            "offline/remote-asset"
        );
    }

    #[test]
    fn a_remote_iframe_is_reported_under_its_own_code() {
        assert_eq!(
            first("<iframe src=\"https://example.com/demo\"></iframe>\n").code,
            "offline/remote-embed"
        );
    }

    #[test]
    fn a_remote_object_is_reported_as_an_embed() {
        assert_eq!(
            first("<object data=\"https://example.com/report.pdf\"></object>\n").code,
            "offline/remote-embed"
        );
    }

    #[test]
    fn a_remote_embed_is_reported_as_an_embed() {
        assert_eq!(
            first("<embed src=\"https://example.com/demo.swf\">\n").code,
            "offline/remote-embed"
        );
    }

    #[test]
    fn a_url_in_a_style_attribute_is_reported() {
        let diagnostic =
            first("<div style=\"background: url(https://cdn.example.com/bg.png)\">x</div>\n");

        assert!(diagnostic.message.contains("bg.png"));
    }

    #[test]
    fn a_font_face_in_a_style_block_is_reported() {
        // The exact shape of the failure the README names: a webfont pulled
        // from a host that is not in the room.
        let diagnostic = first(
            "<style>\n@font-face { src: url('https://fonts.gstatic.com/s/inter.woff2'); }\n</style>\n",
        );

        assert!(diagnostic.message.contains("inter.woff2"));
    }

    #[test]
    fn an_import_of_a_remote_stylesheet_is_reported() {
        assert!(first("<style>\n@import url(https://cdn.example.com/theme.css);\n</style>\n")
            .message
            .contains("theme.css"));
    }

    #[test]
    fn a_bare_string_import_is_reported() {
        // `@import "…"` is the form without `url(…)`, and just as much of a
        // request. Catching only one form would be a hole with a workaround.
        let diagnostic =
            first("<style>\n@import \"https://cdn.example.com/theme.css\";\n</style>\n");
        assert!(diagnostic.message.contains("theme.css"));
    }

    #[test]
    fn css_outside_a_style_block_is_not_scanned() {
        // Prose that happens to mention `url(https://…)` fetches nothing.
        assert!(offline("# One\n\nWrite `url(https://example.com)` to load it.\n").is_empty());
    }

    #[test]
    fn a_local_url_in_css_passes() {
        assert!(offline("<div style=\"background: url(./bg.png)\">x</div>\n").is_empty());
    }

    #[test]
    fn a_markdown_link_to_a_remote_page_passes() {
        // A link is followed later, from someone else's machine. Flagging it
        // would make the rule unusable and protect nobody.
        assert!(offline("# One\n\nSee [the paper](https://example.com/paper).\n").is_empty());
    }

    #[test]
    fn an_anchor_href_passes_while_an_image_src_on_the_same_slide_fails() {
        // The distinction the whole rule turns on, in one slide.
        let diagnostics = offline(
            "# One\n\n<a href=\"https://example.com\">source</a>\n\n<img src=\"https://cdn.example.com/c.png\" alt=\"c\">\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("c.png"));
    }

    #[test]
    fn an_anchor_wrapping_a_remote_image_still_reports_the_image() {
        let diagnostics = offline(
            "<a href=\"https://example.com\"><img src=\"https://cdn.example.com/c.png\" alt=\"c\"></a>\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn a_relative_path_passes() {
        assert!(offline("![a chart](./chart.png)\n<img src=\"../shared/logo.svg\" alt=\"\">\n")
            .is_empty());
    }

    #[test]
    fn a_root_relative_path_passes() {
        // Served from the deck's own origin, so it is bundled output.
        assert!(offline("<img src=\"/assets/logo.svg\" alt=\"\">\n").is_empty());
    }

    #[test]
    fn a_data_uri_passes() {
        // The file is the reference; there is nothing left to fetch.
        assert!(offline("![dot](data:image/png;base64,iVBORw0KGgo=)\n").is_empty());
    }

    #[test]
    fn a_data_uri_in_css_passes() {
        assert!(offline("<div style=\"background: url(data:image/gif;base64,R0lGOD)\">x</div>\n")
            .is_empty());
    }

    #[test]
    fn an_http_url_is_caught_as_well_as_an_https_one() {
        assert_eq!(
            first("<img src=\"http://cdn.example.com/c.png\" alt=\"c\">\n").severity,
            Severity::Error
        );
    }

    #[test]
    fn the_host_does_not_matter() {
        // The machine that builds a deck is not the machine that shows it.
        assert_eq!(offline("<img src=\"http://localhost:5173/c.png\" alt=\"c\">\n").len(), 1);
    }

    #[test]
    fn an_uppercase_scheme_is_still_remote() {
        // Schemes are case-insensitive, so a capital letter must not be an
        // escape hatch from a guarantee.
        assert_eq!(offline("<img src=\"HTTPS://cdn.example.com/c.png\" alt=\"c\">\n").len(), 1);
    }

    #[test]
    fn an_uppercase_tag_and_attribute_are_still_scanned() {
        assert_eq!(offline("<IMG SRC=\"https://cdn.example.com/c.png\" ALT=\"c\">\n").len(), 1);
    }

    #[test]
    fn a_protocol_relative_url_is_caught() {
        // No scheme, but it inherits the page's and goes to the network all
        // the same — the shape that looks most like a path and is not one.
        let diagnostic = first("<img src=\"//cdn.example.com/c.png\" alt=\"c\">\n");
        assert!(diagnostic.message.contains("//cdn.example.com/c.png"));
    }

    #[test]
    fn a_protocol_relative_markdown_image_is_caught() {
        assert_eq!(offline("![c](//cdn.example.com/c.png)\n").len(), 1);
    }

    #[test]
    fn a_path_containing_a_colon_is_not_mistaken_for_a_scheme() {
        assert!(offline("![c](./slides/chapter:1.png)\n").is_empty());
    }

    #[test]
    fn a_remote_srcset_candidate_is_caught_behind_a_local_one() {
        // A list is only as offline as its worst entry, and the remote entry
        // is the one a high-density projector picks.
        let diagnostic =
            first("<img src=\"./c.png\" srcset=\"./c.png 1x, https://cdn.example.com/c@2x.png 2x\" alt=\"c\">\n");

        assert!(diagnostic.message.contains("c@2x.png"));
    }

    #[test]
    fn every_offender_on_a_slide_is_reported_separately() {
        // One diagnostic per fix, so the author can work down a list rather
        // than rebuild and rediscover the next one.
        let diagnostics = offline(
            "# One\n\n![a](https://cdn.example.com/a.png)\n\n<script src=\"https://cdn.example.com/b.js\"></script>\n\n<link href=\"https://cdn.example.com/c.css\">\n",
        );

        assert_eq!(diagnostics.len(), 3);
    }

    #[test]
    fn each_slide_reports_its_own() {
        let diagnostics =
            offline("# One\n\n![a](https://cdn.example.com/a.png)\n\n---\n\n# Two\n\n![b](https://cdn.example.com/b.png)\n");

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].span.slide_index, Some(0));
        assert_eq!(diagnostics[1].span.slide_index, Some(1));
    }

    #[test]
    fn the_message_names_the_url() {
        // Without the URL the author has to search the slide for which of
        // several references is the offender.
        assert!(first("![a](https://cdn.example.com/deep/path/a.png)\n")
            .message
            .contains("https://cdn.example.com/deep/path/a.png"));
    }

    #[test]
    fn the_message_names_the_slide() {
        assert!(first("# Results\n\n![a](https://cdn.example.com/a.png)\n")
            .message
            .contains("Results"));
    }

    #[test]
    fn the_help_says_to_bundle_the_file() {
        let help = first("![a](https://cdn.example.com/a.png)\n").help.unwrap();

        assert!(help.contains("next to the deck"));
        assert!(help.contains("data:"));
    }

    #[test]
    fn an_embed_gets_help_that_admits_it_cannot_be_bundled() {
        let help = first("<iframe src=\"https://example.com/demo\"></iframe>\n").help.unwrap();
        assert!(help.contains("fall back"));
    }

    #[test]
    fn the_diagnostic_points_at_the_line_inside_the_slide() {
        let diagnostics = offline("# One\n\n---\n\n# Two\n\n![a](https://cdn.example.com/a.png)\n");

        assert_eq!(diagnostics[0].span.slide_index, Some(1));
        assert!(
            diagnostics[0].span.line > 5,
            "expected a line in slide two, got {}",
            diagnostics[0].span.line
        );
    }

    #[test]
    fn a_remote_reference_inside_a_code_fence_is_not_reported() {
        // A talk about CDNs has to be able to show one on a slide.
        assert!(offline("# One\n\n```html\n<img src=\"https://cdn.example.com/c.png\">\n```\n")
            .is_empty());
    }

    #[test]
    fn a_markdown_image_inside_a_code_fence_is_not_reported() {
        assert!(offline("```md\n![c](https://cdn.example.com/c.png)\n```\n").is_empty());
    }

    #[test]
    fn a_fence_does_not_hide_the_references_after_it() {
        // Blanking a fence must not shift the rest of the slide out of view.
        let diagnostics =
            offline("```html\n<img src=\"https://a.example.com/a.png\">\n```\n\n![b](https://cdn.example.com/b.png)\n");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("b.png"));
    }

    #[test]
    fn a_tag_spread_over_several_lines_is_still_scanned() {
        // Pasted embed codes arrive wrapped, and a formatter rewraps them.
        let diagnostics =
            offline("<iframe\n  width=\"800\"\n  src=\"https://example.com/demo\"\n></iframe>\n");

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn a_greater_than_inside_alt_text_does_not_hide_the_src() {
        // Quoted values stay opaque, so `>` in prose cannot end the tag early.
        assert_eq!(
            offline("<img alt=\"before > after\" src=\"https://cdn.example.com/c.png\">\n").len(),
            1
        );
    }

    #[test]
    fn an_unquoted_attribute_value_is_read() {
        assert_eq!(offline("<img src=https://cdn.example.com/c.png alt=c>\n").len(), 1);
    }

    #[test]
    fn a_single_quoted_attribute_value_is_read() {
        assert_eq!(offline("<img src='https://cdn.example.com/c.png' alt='c'>\n").len(), 1);
    }

    #[test]
    fn a_data_prefixed_attribute_is_not_a_fetch() {
        // `data-src` is inert markup until a script acts on it, and a deck
        // that ships no script never will.
        assert!(offline("<div data-src=\"https://cdn.example.com/c.png\">x</div>\n").is_empty());
    }

    #[test]
    fn an_image_title_is_not_mistaken_for_the_url() {
        assert!(offline("![c](./c.png \"see https://example.com\")\n").is_empty());
    }

    #[test]
    fn an_angle_bracket_destination_is_unwrapped_before_it_is_judged() {
        assert_eq!(offline("![c](<https://cdn.example.com/c.png>)\n").len(), 1);
    }

    #[test]
    fn a_quoted_css_url_is_reported_without_its_quotes() {
        let message =
            first("<div style=\"background: url('https://cdn.example.com/b.png')\">x</div>\n")
                .message;
        assert!(message.contains("loads https://cdn.example.com/b.png from"), "got: {message}");
    }

    #[test]
    fn the_guarantee_can_be_switched_off_by_group() {
        // Suppression is central and by code, like every other rule, so an
        // opt-out is a line in the config rather than an argument here.
        let source = "![c](https://cdn.example.com/c.png)\n";

        assert_eq!(allowing(source, &[]).len(), 1);
        assert!(allowing(source, &["offline"]).is_empty());
    }

    #[test]
    fn allowing_live_embeds_does_not_allow_remote_fonts() {
        // Why embeds carry their own code: one deliberate live demo must not
        // buy an exemption for everything else on the slide.
        let source =
            "<iframe src=\"https://example.com/demo\"></iframe>\n\n<link href=\"https://fonts.googleapis.com/css2\">\n";
        let remaining = allowing(source, &["offline/remote-embed"]);

        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].code, "offline/remote-asset");
    }

    #[test]
    fn relative_and_data_references_are_not_network_requests() {
        assert!(!is_remote("./chart.png"));
        assert!(!is_remote("/assets/chart.png"));
        assert!(!is_remote("chart.png"));
        assert!(!is_remote("data:image/png;base64,iVBORw0KGgo="));
        assert!(!is_remote("#next"));
        assert!(!is_remote(""));
    }

    #[test]
    fn http_and_protocol_relative_references_are_network_requests() {
        assert!(is_remote("https://example.com/a.png"));
        assert!(is_remote("http://example.com/a.png"));
        assert!(is_remote("//example.com/a.png"));
        assert!(is_remote("  https://example.com/a.png  "));
    }

    #[test]
    fn a_deck_with_no_html_at_all_produces_nothing() {
        assert!(offline("# One\n\n- a\n- b\n\nSome prose with a < b in it.\n").is_empty());
    }
}
