//! Whether an image has the pixels for the size it is drawn at.
//!
//! Two failures, both invisible on the machine the deck was written on. A file
//! drawn wider than its own pixels is resampled and goes soft, and a file drawn
//! at a shape other than its own is stretched — and a laptop preview hides both,
//! because at that size the browser is downscaling the whole slide anyway.
//!
//! # Only what the author declared
//!
//! The rule measures a placement against [`crate::image::probe`]'s reading of
//! the file, and it only has a placement when the author wrote one: a `width`,
//! a `height`, or an inline style. A bare `![alt](logo.png)` is drawn at the
//! file's own pixels and capped at its container by the shell's
//! `img { max-width: 100% }`, so there is no upscale to measure and nothing
//! honest to say. Judging an undeclared image would mean measuring the layout,
//! which is a different problem with a different answer.
//!
//! That boundary is the whole reason this rule can be trusted: everything it
//! reports is arithmetic on two numbers the author wrote and two the file
//! carries. Nothing is inferred.
//!
//! # Reading from disk
//!
//! Safe because of the offline guarantee — [`crate::rules::offline`] makes a
//! remote asset a lint error, so every image a deck may legally reference is a
//! path on this machine. A file that is missing, unreadable, or in a format
//! [`crate::image::probe`] does not know is not a finding: the deck under edit
//! references files that do not exist yet, and a linter that reports its own
//! gaps as the author's mistakes is one the author switches off.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use slidx_core::{Diagnostic, Diagnostics, Severity, Slide, SourceSpan};

use crate::image::{self, Intrinsic};
use crate::markup;
use crate::{LintInput, LintOptions};

/// Longest prefix of a file this reads.
///
/// A JPEG keeps its frame header behind whatever EXIF and colour profile the
/// camera wrote, so a handful of bytes is not enough to be useful. A whole file
/// is far too many: this runs over every image on every build, and reading
/// forty megabytes of screenshot to learn two integers is not a trade worth
/// making. Past this the rule reports nothing, like any other header it cannot
/// read.
const HEADER_BYTES: u64 = 64 * 1024;

pub fn check(input: &LintInput<'_>, options: &LintOptions, sink: &mut Diagnostics) {
    let Some(root) = input.assets else { return };

    for slide in &input.deck.slides {
        let content = markup::scannable(slide);

        for placement in placements(&content) {
            let width = placement.width.map(|length| length.resolve(input.target.width_px));
            let height = placement.height.map(|length| length.resolve(input.target.height_px));
            if width.is_none() && height.is_none() {
                continue;
            }

            let Some(intrinsic) = intrinsic(root, placement.url) else { continue };
            let span =
                SourceSpan::line(slide.source_line + markup::line_at(&content, placement.offset))
                    .on_slide(slide.index);

            // A height alone still fixes a width, because the browser keeps the
            // file's ratio for the dimension the author left out.
            //
            // Capped at the slide, because the shell's `max-width: 100%` caps
            // it too: naming the declared number would name a size nobody in
            // the room ever sees, and would overstate the softness by as much
            // as the author overstated the box.
            let drawn = width
                .or_else(|| Some(height? * intrinsic.aspect()?))
                .map(|px| px.min(input.target.width_px));

            let findings = [
                drawn.and_then(|drawn| upscaled(&placement, intrinsic, drawn, slide, options)),
                width
                    .zip(height)
                    .and_then(|(w, h)| stretched(&placement, intrinsic, w, h, slide, options)),
            ];

            sink.extend(findings.into_iter().flatten().map(|finding| finding.at(span)));
        }
    }
}

/// An image drawn from fewer pixels than the slide gives it.
fn upscaled(
    placement: &Placement<'_>,
    intrinsic: Intrinsic,
    drawn_px: f64,
    slide: &Slide,
    options: &LintOptions,
) -> Option<Diagnostic> {
    // Vector art has no resolution to run out of: the renderer re-runs the
    // paths at whatever size the layout asks for.
    if intrinsic.format.is_scalable() {
        return None;
    }

    let ratio = intrinsic.upscale(drawn_px)?;
    if ratio <= options.images.max_upscale {
        return None;
    }

    let comfortable = f64::from(intrinsic.width) * options.images.max_upscale;

    Some(
        Diagnostic::new(
            "resolution/upscaled",
            // A soft image still shows. Stopping a build over one, minutes
            // before a talk, is how a linter gets switched off — and then it
            // catches the contrast failure next to it for nobody.
            Severity::Warning,
            format!(
                "{} is {}px wide but drawn at {drawn_px:.0}px on \"{}\", {ratio:.1}x its own pixels",
                placement.url,
                intrinsic.width,
                slide.display_title()
            ),
        )
        .with_help(format!(
            "export it at {drawn_px:.0}px or wider, or draw it no wider than {comfortable:.0}px"
        )),
    )
}

/// A declared box that disagrees with the shape of the file in it.
fn stretched(
    placement: &Placement<'_>,
    intrinsic: Intrinsic,
    width_px: f64,
    height_px: f64,
    slide: &Slide,
    options: &LintOptions,
) -> Option<Diagnostic> {
    let drift = intrinsic.aspect_drift(width_px, height_px)?;
    if drift <= options.images.max_aspect_drift {
        return None;
    }

    Some(
        Diagnostic::new(
            "resolution/aspect",
            Severity::Warning,
            format!(
                "{} is {}x{} but drawn in a {width_px:.0}x{height_px:.0} box on \"{}\", \
                 {:.0}% off its own shape",
                placement.url,
                intrinsic.width,
                intrinsic.height,
                slide.display_title(),
                drift * 100.0
            ),
        )
        .with_help(format!(
            "declare one of the two and let the other follow the file, or re-export it at \
             {width_px:.0}x{height_px:.0}"
        )),
    )
}

/// An image on a slide, with whatever size the author declared for it.
#[derive(Debug, Clone, Copy)]
struct Placement<'a> {
    url: &'a str,
    width: Option<Length>,
    height: Option<Length>,
    /// Byte offset of the `src` value, so a multi-line tag names its own line.
    offset: usize,
}

/// A declared size, before the slide is known.
#[derive(Debug, Clone, Copy)]
enum Length {
    Px(f64),
    Percent(f64),
}

impl Length {
    /// Resolved against the slide.
    ///
    /// A percentage is measured against the whole canvas rather than the box
    /// the image sits in, because the linter has no theme and therefore no
    /// safe-area padding to subtract. The real container is narrower by that
    /// padding, so a percentage-sized image is judged slightly larger than it
    /// draws — slack the default tolerance carries several times over.
    fn resolve(self, canvas_px: f64) -> f64 {
        match self {
            Self::Px(px) => px,
            Self::Percent(percent) => canvas_px * percent / 100.0,
        }
    }
}

/// Every `<img>` on a slide, with its declared size.
///
/// Markdown's `![alt](url)` is deliberately not collected: it carries no size,
/// so there is nothing to compare it against.
fn placements(content: &str) -> Vec<Placement<'_>> {
    let mut found = Vec::new();
    let mut at = 0;

    while let Some(open) = content[at..].find('<') {
        let start = at + open + 1;
        at = start;

        let Some((tag, mut cursor)) = markup::tag_name(content, start) else { continue };
        let mut image = Placement { url: "", width: None, height: None, offset: start };
        let mut style = None;

        while let Some(attribute) = markup::attribute(content, cursor) {
            cursor = attribute.next;

            match attribute.name.to_ascii_lowercase().as_str() {
                "src" => {
                    image.url = attribute.value.trim();
                    image.offset = attribute.offset;
                }
                "width" => image.width = length(attribute.value),
                "height" => image.height = length(attribute.value),
                "style" => style = Some(attribute.value),
                _ => {}
            }
        }

        // Past the `>` that closed the open tag, when the author wrote one.
        at = cursor + usize::from(content[cursor..].starts_with('>'));

        // An inline style outranks a presentational hint in every browser, so
        // wherever an author wrote both, the style is the size on the screen.
        //
        // It decides even when its value is one the linter cannot resolve: a
        // `style="width: auto"` discards the `width` attribute beside it, and
        // falling back to that attribute would report a size nobody was shown.
        if let Some(style) = style {
            if let Some(declared) = declaration(style, "width") {
                image.width = length(declared);
            }
            if let Some(declared) = declaration(style, "height") {
                image.height = length(declared);
            }
        }

        if tag.eq_ignore_ascii_case("img") && !image.url.is_empty() {
            found.push(image);
        }
    }

    found
}

/// One declaration out of an inline `style` attribute.
fn declaration<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        name.trim().eq_ignore_ascii_case(property).then(|| value.trim())
    })
}

/// A declared length, in pixels or as a share of the slide.
///
/// Anything else — `auto`, `em`, `vw`, a `calc()` — depends on a font or a
/// viewport the linter cannot see, so it is left alone rather than guessed at.
fn length(value: &str) -> Option<Length> {
    let value = value.trim();

    if let Some(percent) = value.strip_suffix('%') {
        return percent.trim().parse().ok().filter(positive).map(Length::Percent);
    }

    // A bare number is what the HTML attribute carries; CSS needs the unit.
    let number = value.strip_suffix("px").unwrap_or(value).trim();
    number.parse().ok().filter(positive).map(Length::Px)
}

fn positive(value: &f64) -> bool {
    *value > 0.0 && value.is_finite()
}

/// What the file behind a reference says about its own size.
fn intrinsic(root: &Path, url: &str) -> Option<Intrinsic> {
    image::probe(&header(&resolve(root, url)?)?)
}

/// The file a reference points at, or `None` when it does not point at one.
///
/// Anything carrying a scheme is not a path: `https:` belongs to the offline
/// rule, and a `data:` URI is the file rather than a name for one.
fn resolve(root: &Path, url: &str) -> Option<PathBuf> {
    let url = url.trim();
    if url.is_empty() || url.starts_with("//") || markup::scheme(url).is_some() {
        return None;
    }

    // Vite carries build instructions in the query and a fragment selects part
    // of an SVG; neither is part of the name on disk.
    let path = url.split(['?', '#']).next()?;

    // A root-relative reference is served out of the deck's own output, which
    // is the directory being linted.
    Some(root.join(path.trim_start_matches('/')))
}

fn header(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path).ok()?.take(HEADER_BYTES).read_to_end(&mut bytes).ok()?;

    Some(bytes)
}

#[cfg(test)]
mod tests {
    use crate::image::fixtures as file;
    use crate::test_support::Assets;
    use slidx_core::{Diagnostic, Severity};

    /// Only what this rule reported — a lint run covers every rule.
    fn resolution(assets: &Assets, source: &str) -> Vec<Diagnostic> {
        assets.lint(source).into_iter().filter(|d| d.code.starts_with("resolution/")).collect()
    }

    fn first(assets: &Assets, source: &str) -> Diagnostic {
        let diagnostics = resolution(assets, source);
        assert_eq!(diagnostics.len(), 1, "expected exactly one finding in: {source}");
        diagnostics[0].clone()
    }

    #[test]
    fn a_logo_drawn_across_half_a_slide_from_a_quarter_of_the_pixels_is_flagged() {
        // The failure the rule exists for: 400px of logo stretched over 960px
        // of a 1920 slide, which looks fine on a laptop and mushy from row 20.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let diagnostic =
            first(&assets, "# Results\n\n<img src=\"logo.png\" width=\"960\" alt=\"logo\">\n");

        assert_eq!(diagnostic.code, "resolution/upscaled");
        assert!(diagnostic.message.contains("400"), "got: {}", diagnostic.message);
        assert!(diagnostic.message.contains("960"), "got: {}", diagnostic.message);
        assert!(diagnostic.message.contains("2.4"), "got: {}", diagnostic.message);
    }

    #[test]
    fn an_image_drawn_at_its_own_size_produces_nothing() {
        // A one-to-one asset is correct on the 1080p projector a track room
        // still has. Flagging it would flag every well-made deck.
        let assets = Assets::new().with("chart.png", &file::png(800, 600));
        assert!(resolution(&assets, "<img src=\"chart.png\" width=\"800\" alt=\"c\">\n").is_empty());
    }

    #[test]
    fn an_image_drawn_smaller_than_its_own_pixels_produces_nothing() {
        let assets = Assets::new().with("photo.jpg", &file::jpeg(4032, 3024));
        assert!(resolution(&assets, "<img src=\"photo.jpg\" width=\"900\" alt=\"p\">\n").is_empty());
    }

    #[test]
    fn a_slight_upscale_is_left_alone() {
        // 20% is inside the band where resampling is invisible at projection
        // distance, and reporting it would bury the cases that matter.
        let assets = Assets::new().with("logo.png", &file::png(800, 400));
        assert!(resolution(&assets, "<img src=\"logo.png\" width=\"960\" alt=\"l\">\n").is_empty());
    }

    #[test]
    fn the_upscale_threshold_is_configurable() {
        let assets = Assets::new().with("logo.png", &file::png(800, 400));
        let source = "<img src=\"logo.png\" width=\"960\" alt=\"l\">\n";

        let strict = assets.lint_with(source, |options| options.images.max_upscale = 1.1);
        assert!(strict.iter().any(|d| d.code == "resolution/upscaled"));

        let lax = assets.lint_with(source, |options| options.images.max_upscale = 4.0);
        assert!(lax.iter().all(|d| d.code != "resolution/upscaled"));
    }

    #[test]
    fn a_percentage_width_is_measured_against_the_slide() {
        // "half the slide" is a number, because the deck's design size is
        // known — 50% of a 1920 canvas is 960px of a 400px logo.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let diagnostic =
            first(&assets, "<img src=\"logo.png\" style=\"width: 50%\" alt=\"logo\">\n");

        assert_eq!(diagnostic.code, "resolution/upscaled");
    }

    #[test]
    fn a_width_wider_than_the_slide_is_reported_at_the_size_it_will_actually_draw() {
        // The shell caps every image at its container, so naming the declared
        // number would name a size nobody ever sees.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let diagnostic = first(&assets, "<img src=\"logo.png\" width=\"6000\" alt=\"logo\">\n");

        assert!(diagnostic.message.contains("1920"), "got: {}", diagnostic.message);
        assert!(!diagnostic.message.contains("6000"), "got: {}", diagnostic.message);
    }

    #[test]
    fn a_height_alone_still_says_how_wide_the_image_is_drawn() {
        // The browser keeps the file's ratio for the dimension the author left
        // out, so `height=` is as much of a size declaration as `width=`.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let diagnostic = first(&assets, "<img src=\"logo.png\" height=\"600\" alt=\"l\">\n");

        assert_eq!(diagnostic.code, "resolution/upscaled");
        assert!(diagnostic.message.contains("1200"), "got: {}", diagnostic.message);
    }

    #[test]
    fn an_image_that_is_both_soft_and_stretched_reports_both() {
        // One diagnostic per fix, so the author can work down a list rather
        // than rebuild and rediscover the second problem.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let codes: Vec<String> =
            resolution(&assets, "<img src=\"logo.png\" width=\"960\" height=\"960\" alt=\"l\">\n")
                .into_iter()
                .map(|d| d.code)
                .collect();

        assert_eq!(codes, vec!["resolution/upscaled", "resolution/aspect"]);
    }

    #[test]
    fn a_tag_spread_over_several_lines_is_still_measured() {
        // Formatters wrap long tags, and a wrapped tag is the same tag.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let source = "<img\n  src=\"logo.png\"\n  width=\"960\"\n  alt=\"l\"\n>\n";

        assert_eq!(first(&assets, source).code, "resolution/upscaled");
    }

    #[test]
    fn an_uppercase_tag_and_attribute_are_still_measured() {
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        assert_eq!(
            first(&assets, "<IMG SRC=\"logo.png\" WIDTH=\"960\" ALT=\"l\">\n").code,
            "resolution/upscaled"
        );
    }

    #[test]
    fn a_size_in_a_unit_the_linter_cannot_resolve_is_left_alone() {
        // `em` depends on a font and `vw` on a window; guessing either would
        // put a number in the message that is not the number on the screen.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let source = "<img src=\"logo.png\" style=\"width: 60vw\" alt=\"l\">\n\
                      <img src=\"logo.png\" style=\"width: 40em\" alt=\"l\">\n";

        assert!(resolution(&assets, source).is_empty());
    }

    #[test]
    fn a_style_the_linter_cannot_resolve_still_discards_the_attribute_it_replaces() {
        // `width: auto` throws the attribute away in the browser too, so
        // falling back to it would report a size nobody was ever shown.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let source = "<img src=\"logo.png\" width=\"960\" style=\"width: auto\" alt=\"l\">\n";

        assert!(resolution(&assets, source).is_empty());
    }

    #[test]
    fn a_max_width_is_not_read_as_a_width() {
        // It caps rather than sets, so it is not a declaration of size.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let source = "<img src=\"logo.png\" style=\"max-width: 960px\" alt=\"l\">\n";

        assert!(resolution(&assets, source).is_empty());
    }

    #[test]
    fn an_svg_is_never_reported_as_upscaled() {
        // Vector art has no resolution to run out of; the renderer re-runs the
        // paths at whatever size the layout asks for.
        let assets = Assets::new().with("logo.svg", &file::svg("viewBox=\"0 0 64 32\""));
        assert!(resolution(&assets, "<img src=\"logo.svg\" width=\"960\" alt=\"l\">\n").is_empty());
    }

    #[test]
    fn the_upscale_help_names_both_ways_out() {
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let help = first(&assets, "<img src=\"logo.png\" width=\"960\" alt=\"l\">\n")
            .help
            .expect("a next action");

        assert!(help.contains("960"), "the width to export at: {help}");
        assert!(help.contains("600"), "the width it could be drawn at: {help}");
    }

    #[test]
    fn a_box_that_disagrees_with_the_file_is_reported_with_both_shapes() {
        let assets = Assets::new().with("team.jpg", &file::jpeg(1600, 900));
        let diagnostic = first(
            &assets,
            "# Team\n\n<img src=\"team.jpg\" width=\"800\" height=\"600\" alt=\"t\">\n",
        );

        assert_eq!(diagnostic.code, "resolution/aspect");
        assert!(diagnostic.message.contains("1600x900"), "got: {}", diagnostic.message);
        assert!(diagnostic.message.contains("800x600"), "got: {}", diagnostic.message);
        assert!(diagnostic.message.contains("33%"), "got: {}", diagnostic.message);
    }

    #[test]
    fn a_box_at_the_files_own_ratio_produces_nothing() {
        let assets = Assets::new().with("team.jpg", &file::jpeg(1600, 900));
        assert!(resolution(
            &assets,
            "<img src=\"team.jpg\" width=\"800\" height=\"450\" alt=\"t\">\n"
        )
        .is_empty());
    }

    #[test]
    fn a_ratio_that_is_a_rounding_away_from_the_file_is_left_alone() {
        // Authors write whole pixels. 1000x667 is 3:2 to the eye, and telling
        // them their 900x600 box is wrong would be pedantry.
        let assets = Assets::new().with("photo.jpg", &file::jpeg(1000, 667));
        assert!(resolution(
            &assets,
            "<img src=\"photo.jpg\" width=\"900\" height=\"600\" alt=\"p\">\n"
        )
        .is_empty());
    }

    #[test]
    fn the_aspect_tolerance_is_configurable() {
        let assets = Assets::new().with("photo.jpg", &file::jpeg(1000, 667));
        let source = "<img src=\"photo.jpg\" width=\"900\" height=\"600\" alt=\"p\">\n";

        let strict = assets.lint_with(source, |options| options.images.max_aspect_drift = 0.0001);
        assert!(strict.iter().any(|d| d.code == "resolution/aspect"));
    }

    #[test]
    fn an_svg_in_a_disagreeing_box_is_still_reported_as_stretched() {
        // Vector art scales without softening, and stretches like anything
        // else, so the two checks part company here.
        let assets = Assets::new().with("logo.svg", &file::svg("viewBox=\"0 0 64 32\""));
        let diagnostic =
            first(&assets, "<img src=\"logo.svg\" width=\"400\" height=\"400\" alt=\"l\">\n");

        assert_eq!(diagnostic.code, "resolution/aspect");
    }

    #[test]
    fn only_one_of_the_two_dimensions_is_not_a_disagreement() {
        let assets = Assets::new().with("team.jpg", &file::jpeg(1600, 900));
        assert!(resolution(&assets, "<img src=\"team.jpg\" height=\"450\" alt=\"t\">\n").is_empty());
    }

    #[test]
    fn the_aspect_help_offers_the_fix_that_keeps_the_ratio() {
        let assets = Assets::new().with("team.jpg", &file::jpeg(1600, 900));
        let help =
            first(&assets, "<img src=\"team.jpg\" width=\"800\" height=\"600\" alt=\"t\">\n")
                .help
                .expect("a next action");

        assert!(help.contains("one"), "got: {help}");
        assert!(help.contains("800x600"), "the crop that would make it true: {help}");
    }

    #[test]
    fn an_inline_style_wins_over_the_attribute_it_contradicts() {
        // Author CSS beats a presentational hint, so the style is the size
        // the audience gets and the attribute is decoration.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let source = "<img src=\"logo.png\" width=\"400\" style=\"width: 960px\" alt=\"l\">\n";

        assert_eq!(first(&assets, source).code, "resolution/upscaled");
    }

    #[test]
    fn an_image_with_no_declared_size_is_not_judged() {
        // Without a declaration the shell draws the file at its own size,
        // capped at the container. There is no upscale to measure.
        let assets = Assets::new().with("logo.png", &file::png(40, 20));
        assert!(resolution(&assets, "![a small logo](logo.png)\n").is_empty());
    }

    #[test]
    fn every_recognised_format_is_measured() {
        let assets = Assets::new()
            .with("a.png", &file::png(100, 50))
            .with("b.jpg", &file::jpeg(100, 50))
            .with("c.gif", &file::gif(100, 50))
            .with("d.webp", &file::webp_lossy(100, 50));

        let source = "<img src=\"a.png\" width=\"960\" alt=\"a\">\n\
                      <img src=\"b.jpg\" width=\"960\" alt=\"b\">\n\
                      <img src=\"c.gif\" width=\"960\" alt=\"c\">\n\
                      <img src=\"d.webp\" width=\"960\" alt=\"d\">\n";

        assert_eq!(resolution(&assets, source).len(), 4);
    }

    #[test]
    fn a_format_the_linter_cannot_read_is_not_a_finding() {
        // The rule reports what it knows and stays silent about what it does
        // not. A linter that cries wolf about an unfamiliar format is one the
        // author switches off, and then it catches nothing at all.
        let assets = Assets::new().with("hero.avif", b"\x00\x00\x00\x20ftypavif\x00\x00\x00\x00");
        assert!(
            resolution(&assets, "<img src=\"hero.avif\" width=\"1900\" alt=\"h\">\n").is_empty()
        );
    }

    #[test]
    fn a_file_that_is_not_there_is_not_a_finding() {
        // A deck under edit references files that do not exist yet, and the
        // build is the thing that reports a missing asset, not the linter.
        let assets = Assets::new();
        assert!(
            resolution(&assets, "<img src=\"missing.png\" width=\"960\" alt=\"m\">\n").is_empty()
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_is_not_a_finding() {
        let assets = Assets::new().with("truncated.png", &file::png(400, 200)[..12]);
        assert!(
            resolution(&assets, "<img src=\"truncated.png\" width=\"960\" alt=\"t\">\n").is_empty()
        );
    }

    #[test]
    fn a_remote_image_is_left_to_the_offline_rule() {
        // Two rules reporting the same reference would double the noise, and
        // this one has nothing to open.
        let assets = Assets::new();
        let source = "<img src=\"https://cdn.example.com/logo.png\" width=\"960\" alt=\"l\">\n";

        assert!(resolution(&assets, source).is_empty());
        assert!(assets.lint(source).iter().any(|d| d.code == "offline/remote-asset"));
    }

    #[test]
    fn a_data_uri_is_not_a_path_to_open() {
        let assets = Assets::new();
        let source = "<img src=\"data:image/png;base64,iVBORw0KGgo=\" width=\"960\" alt=\"d\">\n";

        assert!(resolution(&assets, source).is_empty());
    }

    #[test]
    fn a_build_query_and_a_fragment_are_not_part_of_the_file_name() {
        // Vite carries build instructions in the query, and a fragment picks
        // part of an SVG. Neither changes which file is on disk.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let source = "<img src=\"logo.png?inline\" width=\"960\" alt=\"l\">\n";

        assert_eq!(first(&assets, source).code, "resolution/upscaled");
    }

    #[test]
    fn a_root_relative_path_resolves_against_the_assets_directory() {
        let assets = Assets::new().with("images/logo.png", &file::png(400, 200));
        let source = "<img src=\"/images/logo.png\" width=\"960\" alt=\"l\">\n";

        assert_eq!(first(&assets, source).code, "resolution/upscaled");
    }

    #[test]
    fn an_image_inside_a_code_fence_is_not_checked() {
        // A talk about image sizing has to be able to show a bad example.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let source = "```html\n<img src=\"logo.png\" width=\"960\" alt=\"l\">\n```\n";

        assert!(resolution(&assets, source).is_empty());
    }

    #[test]
    fn nothing_is_read_when_the_deck_has_no_assets_directory() {
        // The editor lints while the author types, before anything is on disk
        // and with no root to resolve against.
        let diagnostics =
            crate::test_support::lint_deck("<img src=\"logo.png\" width=\"960\" alt=\"l\">\n");

        assert!(diagnostics.iter().all(|d| !d.code.starts_with("resolution/")));
    }

    #[test]
    fn a_finding_points_at_the_slide_and_line_it_came_from() {
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let diagnostic = first(
            &assets,
            "# One\n\n---\n\n# Two\n\n<img src=\"logo.png\" width=\"960\" alt=\"l\">\n",
        );

        assert_eq!(diagnostic.span.slide_index, Some(1));
        assert!(
            diagnostic.span.line > 5,
            "expected a line in slide two, got {}",
            diagnostic.span.line
        );
    }

    #[test]
    fn a_soft_image_is_a_warning_rather_than_a_failed_build() {
        // A blurry logo still shows. Stopping a build over one, minutes before
        // a talk, is how a linter gets switched off.
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let diagnostic = first(&assets, "<img src=\"logo.png\" width=\"960\" alt=\"l\">\n");

        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(!diagnostic.is_blocking());
    }

    #[test]
    fn the_findings_can_be_switched_off_by_group() {
        let assets = Assets::new().with("logo.png", &file::png(400, 200));
        let source = "<img src=\"logo.png\" width=\"960\" alt=\"l\">\n";

        let allowed = assets.lint_with(source, |options| {
            options.allow = vec!["resolution".to_string()];
        });

        assert!(allowed.iter().all(|d| !d.code.starts_with("resolution/")));
    }
}
