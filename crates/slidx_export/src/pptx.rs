//! A presentation somebody can open in Google Slides.
//!
//! The honest offline answer to "something Google Slides can open" is a
//! `.pptx`. Google Slides imports one natively and keeps the slides separate;
//! an imported PDF collapses into a stack of flat images with no slide
//! boundaries, no notes, and nothing to edit. And a `.pptx` is a zip holding a
//! few XML parts, so writing one needs no service, no account, and no network —
//! which is the whole point. slidx produces a file; the author opens it.
//!
//! ## What survives the trip, and what does not
//!
//! **Survives:** every stop, as its own slide, at exactly the size and typeface
//! it was rendered at, because each one is the image the build already made.
//! And the speaker notes, as real notes text — editable, searchable, and visible
//! in presenter mode, rather than baked into a picture.
//!
//! **Does not:** the text of the slide itself, the animation, the transitions,
//! the links, and the theme. A slide arrives as a picture, so nothing on it can
//! be re-typed on the other side. That is a real loss and it is stated wherever
//! this export is offered, because an export that quietly loses the animation is
//! worse than one that says it will.
//!
//! The alternative — mapping slidx's layout onto OOXML shapes so the text stays
//! text — would be a second renderer with a second idea of what a slide looks
//! like, and the first thing it would get wrong is the one thing this pipeline
//! exists to protect: that what is handed over is what was rehearsed.
//!
//! ## Notes belong to the slide, not to the stop
//!
//! A slide that builds in four steps becomes four slides here, because Google
//! Slides has no timeline to import one into and the stop is the unit every
//! slidx export uses. Its notes are attached to all four: they are what the
//! speaker means to say about that slide, and a speaker who has advanced to the
//! third stop still needs them.

pub mod parts;

use crate::zip::{self, Entry};
use parts::{DECLARATION, NAMESPACES, RELATIONSHIP, RELATIONSHIPS_NAMESPACE};

/// One stop, as it becomes one slide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PptxSlide {
    /// The rendered image, full bleed. PNG, as the build writes it.
    pub image: Vec<u8>,
    /// What the slide is, for the picture's alternative text.
    pub title: Option<String>,
    /// The speaker notes of the slide this stop belongs to.
    pub notes: Vec<String>,
}

/// A deck, as a presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PptxDeck {
    pub title: String,
    /// The deck's aspect ratio, as its two sides.
    pub aspect: (u32, u32),
    pub slides: Vec<PptxSlide>,
}

/// English Metric Units in one inch, which is how OOXML measures everything.
const EMU_PER_INCH: u64 = 914_400;

/// Ten inches across, the width the print shell also lays a page out at.
const SLIDE_WIDTH: u64 = 10 * EMU_PER_INCH;

/// The standard notes page: 7.5 by 10 inches, portrait.
const NOTES_WIDTH: u64 = 6_858_000;
const NOTES_HEIGHT: u64 = 9_144_000;

/// Writes the presentation.
pub fn write(deck: &PptxDeck) -> Vec<u8> {
    let height = slide_height(deck.aspect);
    let count = deck.slides.len();

    // `[Content_Types].xml` first. The specification calls it a package-level
    // part rather than an entry, and readers that scan from the front for it
    // reject a package that buries it behind the media.
    let mut entries = vec![
        Entry::new("[Content_Types].xml", content_types(count).into_bytes()),
        Entry::new("_rels/.rels", package_relationships().into_bytes()),
        Entry::new("ppt/presentation.xml", presentation(count, height).into_bytes()),
        Entry::new(
            "ppt/_rels/presentation.xml.rels",
            presentation_relationships(count).into_bytes(),
        ),
        Entry::new("ppt/slideMasters/slideMaster1.xml", parts::slide_master().into_bytes()),
        Entry::new(
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            relationships(&[
                ("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"),
                ("rId2", "theme", "../theme/theme1.xml"),
            ])
            .into_bytes(),
        ),
        Entry::new("ppt/slideLayouts/slideLayout1.xml", parts::slide_layout().into_bytes()),
        Entry::new(
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
            relationships(&[("rId1", "slideMaster", "../slideMasters/slideMaster1.xml")])
                .into_bytes(),
        ),
        Entry::new("ppt/notesMasters/notesMaster1.xml", parts::notes_master().into_bytes()),
        Entry::new(
            "ppt/notesMasters/_rels/notesMaster1.xml.rels",
            relationships(&[("rId1", "theme", "../theme/theme1.xml")]).into_bytes(),
        ),
        Entry::new("ppt/theme/theme1.xml", parts::theme().into_bytes()),
    ];

    for (index, slide) in deck.slides.iter().enumerate() {
        let at = index + 1;

        entries.push(Entry::new(
            format!("ppt/slides/slide{at}.xml"),
            slide_part(slide, height).into_bytes(),
        ));
        entries.push(Entry::new(
            format!("ppt/slides/_rels/slide{at}.xml.rels"),
            relationships(&[
                ("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"),
                ("rId2", "image", &format!("../media/image{at}.png")),
                ("rId3", "notesSlide", &format!("../notesSlides/notesSlide{at}.xml")),
            ])
            .into_bytes(),
        ));
        entries.push(Entry::new(
            format!("ppt/notesSlides/notesSlide{at}.xml"),
            notes_part(&slide.notes).into_bytes(),
        ));
        entries.push(Entry::new(
            format!("ppt/notesSlides/_rels/notesSlide{at}.xml.rels"),
            relationships(&[
                ("rId1", "slide", &format!("../slides/slide{at}.xml")),
                ("rId2", "notesMaster", "../notesMasters/notesMaster1.xml"),
            ])
            .into_bytes(),
        ));
        entries.push(Entry::new(format!("ppt/media/image{at}.png"), slide.image.clone()));
    }

    zip::write(&entries)
}

/// How tall a ten-inch slide is at this deck's ratio.
///
/// The same arithmetic the print shell does for `@page`, so a presentation and
/// a printed handout of one deck are the same shape.
fn slide_height(aspect: (u32, u32)) -> u64 {
    let (width, height) = aspect;

    if width == 0 {
        return SLIDE_WIDTH;
    }

    SLIDE_WIDTH * u64::from(height) / u64::from(width)
}

/// What kind of thing every part in the package is.
///
/// A reader is entitled to refuse a part it has no content type for, so this
/// has to name every extension used and override every XML part individually.
fn content_types(count: usize) -> String {
    let office = "application/vnd.openxmlformats-officedocument";

    let mut text = format!(
        "{DECLARATION}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" \
         ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
         <Default Extension=\"png\" ContentType=\"image/png\"/>\
         <Override PartName=\"/ppt/presentation.xml\" \
         ContentType=\"{office}.presentationml.presentation.main+xml\"/>\
         <Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" \
         ContentType=\"{office}.presentationml.slideMaster+xml\"/>\
         <Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" \
         ContentType=\"{office}.presentationml.slideLayout+xml\"/>\
         <Override PartName=\"/ppt/notesMasters/notesMaster1.xml\" \
         ContentType=\"{office}.presentationml.notesMaster+xml\"/>\
         <Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"{office}.theme+xml\"/>"
    );

    for at in 1..=count {
        text.push_str(&format!(
            "<Override PartName=\"/ppt/slides/slide{at}.xml\" \
             ContentType=\"{office}.presentationml.slide+xml\"/>\
             <Override PartName=\"/ppt/notesSlides/notesSlide{at}.xml\" \
             ContentType=\"{office}.presentationml.notesSlide+xml\"/>"
        ));
    }

    text.push_str("</Types>");
    text
}

fn package_relationships() -> String {
    relationships(&[("rId1", "officeDocument", "ppt/presentation.xml")])
}

/// A `.rels` part. `kind` is the last segment of the relationship type.
fn relationships(entries: &[(&str, &str, &str)]) -> String {
    let body: String = entries
        .iter()
        .map(|(id, kind, target)| {
            format!(
                "<Relationship Id=\"{id}\" Type=\"{RELATIONSHIP}/{kind}\" Target=\"{}\"/>",
                escape(target)
            )
        })
        .collect();

    format!(
        "{DECLARATION}<Relationships xmlns=\"{RELATIONSHIPS_NAMESPACE}\">{body}</Relationships>"
    )
}

/// The presentation part: which masters and slides there are, and how big.
///
/// The order of the child elements is the schema's, not a preference. A reader
/// validating against it rejects a `sldSz` that arrives before the slide list.
fn presentation(count: usize, height: u64) -> String {
    let slides: String = (1..=count)
        .map(|at| {
            // Slide identifiers start at 256 by convention and have to be
            // unique; the relationship ids are offset past the two masters.
            format!("<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 255 + at, 2 + at)
        })
        .collect();

    format!(
        "{DECLARATION}<p:presentation{NAMESPACES}>\
         <p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>\
         <p:notesMasterIdLst><p:notesMasterId r:id=\"rId2\"/></p:notesMasterIdLst>\
         <p:sldIdLst>{slides}</p:sldIdLst>\
         <p:sldSz cx=\"{SLIDE_WIDTH}\" cy=\"{height}\"/>\
         <p:notesSz cx=\"{NOTES_WIDTH}\" cy=\"{NOTES_HEIGHT}\"/>\
         </p:presentation>"
    )
}

fn presentation_relationships(count: usize) -> String {
    let mut entries: Vec<(String, String, String)> = vec![
        ("rId1".into(), "slideMaster".into(), "slideMasters/slideMaster1.xml".into()),
        ("rId2".into(), "notesMaster".into(), "notesMasters/notesMaster1.xml".into()),
    ];

    for at in 1..=count {
        entries.push((format!("rId{}", 2 + at), "slide".into(), format!("slides/slide{at}.xml")));
    }

    let borrowed: Vec<(&str, &str, &str)> = entries
        .iter()
        .map(|(id, kind, target)| (id.as_str(), kind.as_str(), target.as_str()))
        .collect();

    relationships(&borrowed)
}

/// One slide: a picture, filling it.
///
/// No placeholder, no title box, no text. The image is the slide, and anything
/// else on top would be a second opinion about what the deck looks like.
fn slide_part(slide: &PptxSlide, height: u64) -> String {
    let description = slide.title.as_deref().unwrap_or("Slide");

    format!(
        "{DECLARATION}<p:sld{NAMESPACES}><p:cSld><p:spTree>\
         <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
         <p:grpSpPr/>\
         <p:pic><p:nvPicPr>\
         <p:cNvPr id=\"2\" name=\"Slide\" descr=\"{description}\"/>\
         <p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>\
         <p:blipFill><a:blip r:embed=\"rId2\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>\
         <p:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/>\
         <a:ext cx=\"{SLIDE_WIDTH}\" cy=\"{height}\"/></a:xfrm>\
         <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
         </p:pic></p:spTree></p:cSld>\
         <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>",
        description = escape(description),
    )
}

/// One notes page, holding the text of the slide this stop belongs to.
///
/// Written even when there are none, so the shape of the package does not
/// depend on whether an author had written their notes yet — and so adding a
/// note months later changes one part rather than the relationships of every
/// slide after it.
fn notes_part(notes: &[String]) -> String {
    let paragraphs: String = notes
        .iter()
        .flat_map(|note| note.lines())
        .map(|line| {
            let text = escape(line.trim());

            if text.is_empty() {
                "<a:p/>".to_string()
            } else {
                format!("<a:p><a:r><a:rPr lang=\"en\" dirty=\"0\"/><a:t>{text}</a:t></a:r></a:p>")
            }
        })
        .collect();

    let body = if paragraphs.is_empty() { "<a:p/>".to_string() } else { paragraphs };

    format!(
        "{DECLARATION}<p:notes{NAMESPACES}><p:cSld><p:spTree>\
         <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
         <p:grpSpPr/>\
         <p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Notes Placeholder\"/>\
         <p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr>\
         <p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr>\
         <p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{body}</p:txBody></p:sp>\
         </p:spTree></p:cSld>\
         <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"
    )
}

/// Text, as XML will accept it.
///
/// A deck title containing `&` is not unusual — "Rust & WebAssembly" is a talk
/// somebody gives every year — and unescaped it produces a package that every
/// reader refuses to open, with no clue as to which of the forty parts is at
/// fault. Control characters are dropped rather than escaped: XML 1.0 has no
/// representation for most of them, so an escaped one is still invalid.
fn escape(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control() || matches!(character, '\t' | '\n' | '\r'))
        .map(|character| match character {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            other => other.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

    fn slide(title: &str, notes: &[&str]) -> PptxSlide {
        PptxSlide {
            image: PNG.to_vec(),
            title: Some(title.to_string()),
            notes: notes.iter().map(|note| note.to_string()).collect(),
        }
    }

    fn deck(slides: Vec<PptxSlide>) -> PptxDeck {
        PptxDeck { title: "Making Decks Fast".into(), aspect: (16, 9), slides }
    }

    /// Every part in a written package, by name.
    fn package(deck: &PptxDeck) -> HashMap<String, String> {
        let archive = write(deck);
        let names = zip::names(&archive);
        let text = String::from_utf8_lossy(&archive).into_owned();

        // Read through the writer's own output rather than a second reader: the
        // names come from the central directory, and each part's text is found
        // by its header. Enough to assert on content without a zip reader that
        // would only ever be used here.
        names
            .iter()
            .map(|name| {
                let body = text
                    .split_once(&format!("{name}<?xml"))
                    .map(|(_, rest)| rest.split("PK\u{3}\u{4}").next().unwrap_or("").to_string())
                    .unwrap_or_default();

                (name.clone(), body)
            })
            .collect()
    }

    #[test]
    fn a_presentation_is_a_zip_holding_the_parts_a_reader_opens_first() {
        // A pptx Google Slides refuses is this target's failure mode, and it
        // starts with the package not being a package.
        let archive = write(&deck(vec![slide("One", &[])]));
        let names = zip::names(&archive);

        assert_eq!(&archive[..4], b"PK\x03\x04");
        assert_eq!(names.first().map(String::as_str), Some("[Content_Types].xml"));
        assert!(names.iter().any(|name| name == "_rels/.rels"), "{names:?}");
        assert!(names.iter().any(|name| name == "ppt/presentation.xml"), "{names:?}");
    }

    #[test]
    fn every_part_in_the_package_has_a_content_type() {
        // A reader is entitled to refuse a part it cannot type, and the answer
        // is a file that opens nowhere with no clue which part is at fault.
        let written = write(&deck(vec![slide("One", &["a note"]), slide("Two", &[])]));
        let names = zip::names(&written);
        let types = String::from_utf8_lossy(&written).into_owned();
        let declared = types.split_once("</Types>").expect("content types").0;

        for name in names.iter().filter(|name| *name != "[Content_Types].xml") {
            let extension = name.rsplit('.').next().unwrap_or_default();
            let default = format!("Extension=\"{extension}\"");
            let override_ = format!("PartName=\"/{name}\"");

            assert!(
                declared.contains(&default) || declared.contains(&override_),
                "{name} has no content type"
            );
        }
    }

    #[test]
    fn every_relationship_points_at_a_part_that_is_in_the_package() {
        // A dangling relationship is how a package opens with a blank slide
        // where a picture should be, or refuses to open at all.
        let written = write(&deck(vec![slide("One", &["a note"]), slide("Two", &[])]));
        let names = zip::names(&written);
        let parts = package(&deck(vec![slide("One", &["a note"]), slide("Two", &[])]));

        for (name, body) in parts.iter().filter(|(name, _)| name.ends_with(".rels")) {
            for target in body.split("Target=\"").skip(1) {
                let target = target.split('"').next().unwrap_or_default();
                let resolved = resolve(name, target);

                assert!(names.contains(&resolved), "{name} points at {resolved}, which is absent");
            }
        }
    }

    /// A relationship target, resolved against the part that declares it.
    fn resolve(rels: &str, target: &str) -> String {
        let base = rels.rsplit_once("_rels/").map(|(base, _)| base).unwrap_or("");
        let mut segments: Vec<&str> = base.trim_end_matches('/').split('/').collect();

        if base.is_empty() {
            segments.clear();
        }

        for segment in target.split('/') {
            match segment {
                ".." => {
                    segments.pop();
                }
                "." | "" => {}
                other => segments.push(other),
            }
        }

        segments.join("/")
    }

    #[test]
    fn one_stop_is_one_slide_with_its_own_image() {
        // The unit every slidx export uses, and the one the print shell chose:
        // a slide that builds in four steps is four slides here, because Google
        // Slides has no timeline to import one into.
        let names = zip::names(&write(&deck(vec![
            slide("One", &[]),
            slide("One", &[]),
            slide("Two", &[]),
        ])));

        for at in 1..=3 {
            assert!(names.iter().any(|name| name == &format!("ppt/slides/slide{at}.xml")));
            assert!(names.iter().any(|name| name == &format!("ppt/media/image{at}.png")));
        }
        assert!(!names.iter().any(|name| name == "ppt/slides/slide4.xml"), "{names:?}");
    }

    #[test]
    fn the_notes_travel_as_notes_text_rather_than_burned_into_the_picture() {
        // The part a speaker has to keep editing and searching. An image of the
        // notes would be worse than no notes at all, because it looks like they
        // survived.
        let parts = package(&deck(vec![slide("One", &["Open with the outcome."])]));
        let notes = parts.get("ppt/notesSlides/notesSlide1.xml").expect("a notes part");

        assert!(notes.contains("<a:t>Open with the outcome.</a:t>"), "{notes}");
        assert!(notes.contains("<p:ph type=\"body\" idx=\"1\"/>"), "{notes}");
    }

    #[test]
    fn a_slides_notes_are_attached_to_every_stop_of_it() {
        // Notes belong to the slide. A speaker who has advanced to the third
        // stop still needs what they meant to say about it.
        let stop = slide("One", &["Say the number, then the caveat."]);
        let parts = package(&deck(vec![stop.clone(), stop]));

        for at in 1..=2 {
            let notes = parts.get(&format!("ppt/notesSlides/notesSlide{at}.xml")).expect("notes");
            assert!(notes.contains("Say the number"), "stop {at}: {notes}");
        }
    }

    #[test]
    fn a_stop_with_no_notes_still_gets_a_notes_page() {
        // So the shape of the package does not depend on whether the author had
        // written their notes yet.
        let parts = package(&deck(vec![slide("One", &[])]));

        assert!(parts.contains_key("ppt/notesSlides/notesSlide1.xml"));
    }

    #[test]
    fn a_title_with_an_ampersand_in_it_does_not_produce_a_package_nobody_can_open() {
        // "Rust & WebAssembly" is a talk somebody gives every year. Unescaped it
        // makes every reader refuse the file, with no clue which part is wrong.
        let parts = package(&deck(vec![slide("Rust & WebAssembly <fast>", &["a & b"])]));
        let slide = parts.get("ppt/slides/slide1.xml").expect("a slide");
        let notes = parts.get("ppt/notesSlides/notesSlide1.xml").expect("notes");

        assert!(slide.contains("Rust &amp; WebAssembly &lt;fast&gt;"), "{slide}");
        assert!(!slide.contains("& W"), "{slide}");
        assert!(notes.contains("a &amp; b"), "{notes}");
    }

    #[test]
    fn a_control_character_is_dropped_rather_than_escaped_into_something_invalid() {
        // XML 1.0 has no representation for most of them, so an escaped one is
        // still a package that will not parse.
        let parts = package(&deck(vec![slide("One", &["bell\u{7}rung"])]));
        let notes = parts.get("ppt/notesSlides/notesSlide1.xml").expect("notes");

        assert!(notes.contains("bellrung"), "{notes}");
    }

    #[test]
    fn the_slide_size_follows_the_decks_own_aspect_ratio() {
        // The same arithmetic the print shell does for `@page`, so a
        // presentation and a printed handout of one deck are the same shape.
        let wide = package(&PptxDeck { aspect: (16, 9), ..deck(vec![slide("One", &[])]) });
        let square = package(&PptxDeck { aspect: (4, 3), ..deck(vec![slide("One", &[])]) });

        assert!(wide["ppt/presentation.xml"].contains("cx=\"9144000\" cy=\"5143500\""));
        assert!(square["ppt/presentation.xml"].contains("cx=\"9144000\" cy=\"6858000\""));
    }

    #[test]
    fn the_picture_fills_the_slide_exactly() {
        // Full bleed is the whole reason this looks right: the image is the
        // slide, so a margin anywhere would be a border the deck never had.
        let parts = package(&PptxDeck { aspect: (4, 3), ..deck(vec![slide("One", &[])]) });
        let slide = &parts["ppt/slides/slide1.xml"];

        assert!(slide.contains("<a:off x=\"0\" y=\"0\"/>"), "{slide}");
        assert!(slide.contains("<a:ext cx=\"9144000\" cy=\"6858000\"/>"), "{slide}");
    }

    #[test]
    fn the_picture_carries_the_slides_title_as_its_alternative_text() {
        let parts = package(&deck(vec![slide("What actually goes wrong", &[])]));

        assert!(parts["ppt/slides/slide1.xml"].contains("descr=\"What actually goes wrong\""));
    }

    #[test]
    fn every_slide_declares_the_layout_it_inherits_from() {
        // OOXML has no concept of a slide without one, and a reader given a
        // slide that points at nothing may refuse the package rather than
        // the slide.
        let parts = package(&deck(vec![slide("One", &[])]));

        assert!(parts["ppt/slides/_rels/slide1.xml.rels"].contains("slideLayout1.xml"));
        assert!(parts["ppt/slides/_rels/slide1.xml.rels"].contains("image1.png"));
        assert!(parts["ppt/slides/_rels/slide1.xml.rels"].contains("notesSlide1.xml"));
    }

    #[test]
    fn exporting_the_same_deck_twice_produces_the_same_bytes() {
        assert_eq!(
            write(&deck(vec![slide("One", &["a"])])),
            write(&deck(vec![slide("One", &["a"])]))
        );
    }

    #[test]
    fn a_deck_with_no_slides_is_still_a_package_rather_than_a_broken_file() {
        // An empty deck is a mistake somebody makes, and a file that will not
        // open is a worse way to find out than an empty presentation.
        let archive = write(&deck(Vec::new()));

        assert_eq!(&archive[..4], b"PK\x03\x04");
        assert!(zip::names(&archive).iter().any(|name| name == "ppt/presentation.xml"));
    }
}
