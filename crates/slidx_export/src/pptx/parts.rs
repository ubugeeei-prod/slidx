//! The parts of a presentation that are the same for every deck.
//!
//! A master, one blank layout, a theme, and a notes master. None of them is
//! interesting and all of them are required: OOXML has no concept of a slide
//! without a layout, a layout without a master, or a master without a theme, and
//! a reader given a package missing any of them is entitled to refuse the whole
//! file rather than the part.
//!
//! They are constants because there is nothing to vary. Every slide slidx
//! exports is one full-bleed image, so no placeholder inherits a position, no
//! text inherits a font, and the theme's colours are never resolved by anything.
//! What matters is that they exist and validate — so they are kept out of
//! [`super`], where the interesting decisions are, and left alone here.

/// The three namespaces every part in a presentation declares.
pub const NAMESPACES: &str = concat!(
    r#" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#,
    r#" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#,
    r#" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#
);

/// The relationship namespace, for the `.rels` parts.
pub const RELATIONSHIPS_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships";

/// The prefix every relationship type shares.
pub const RELATIONSHIP: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

pub const DECLARATION: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

/// An empty shape tree, which every `cSld` needs whether it holds anything.
pub const EMPTY_TREE: &str = concat!(
    "<p:spTree>",
    r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"#,
    "<p:grpSpPr/>",
    "</p:spTree>"
);

/// Which theme colour each slide-level name resolves to.
///
/// Required on a master, and never consulted: an exported slide is one image and
/// resolves no colour at all. The mapping is the identity one PowerPoint writes.
const COLOUR_MAP: &str = concat!(
    r#"<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2""#,
    r#" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink""#,
    r#" folHlink="folHlink"/>"#
);

pub fn slide_master() -> String {
    format!(
        "{DECLARATION}<p:sldMaster{NAMESPACES}><p:cSld>{EMPTY_TREE}</p:cSld>{COLOUR_MAP}\
         <p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>\
         </p:sldMaster>"
    )
}

/// One blank layout, because a slide has to point at one.
///
/// `type="blank"` and no placeholders: a full-bleed image inherits nothing, and
/// a layout carrying a title box would put an empty one on every slide somebody
/// then has to delete forty times.
pub fn slide_layout() -> String {
    format!(
        "{DECLARATION}<p:sldLayout{NAMESPACES} type=\"blank\" preserve=\"1\">\
         <p:cSld name=\"Blank\">{EMPTY_TREE}</p:cSld>\
         <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"
    )
}

/// The notes master, holding the one placeholder the notes inherit from.
///
/// Without a body placeholder here, the text in a notes slide has no shape to
/// inherit its box from and a reader is free to lay it out anywhere or not at
/// all. The size is the standard notes page: 7.5 by 10 inches, portrait.
pub fn notes_master() -> String {
    format!(
        "{DECLARATION}<p:notesMaster{NAMESPACES}><p:cSld><p:spTree>\
         <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
         <p:grpSpPr/>\
         <p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Notes Placeholder\"/>\
         <p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr>\
         <p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr>\
         <p:spPr><a:xfrm><a:off x=\"685800\" y=\"4343400\"/><a:ext cx=\"5486400\" cy=\"4114800\"/>\
         </a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
         <p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp>\
         </p:spTree></p:cSld>{COLOUR_MAP}</p:notesMaster>"
    )
}

/// A theme, at the smallest size the schema accepts.
///
/// Three fill styles, three line styles, three effect styles and three
/// background fills are all required counts, not choices — a shorter list is
/// invalid and a longer one is ignored. The colours are the Office defaults
/// because nothing an exported slide contains ever resolves one: the slide is an
/// image, and the deck's real theme was already applied when it was rendered.
pub fn theme() -> String {
    let fills = concat!(
        "<a:fillStyleLst>",
        r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
        r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
        r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
        "</a:fillStyleLst>"
    );
    let lines = concat!(
        "<a:lnStyleLst>",
        r#"<a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>"#,
        r#"<a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>"#,
        r#"<a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>"#,
        "</a:lnStyleLst>"
    );
    let effects = "<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle>\
                   <a:effectStyle><a:effectLst/></a:effectStyle>\
                   <a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>";
    let backgrounds = concat!(
        "<a:bgFillStyleLst>",
        r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
        r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
        r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
        "</a:bgFillStyleLst>"
    );

    format!(
        "{DECLARATION}<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         name=\"slidx\"><a:themeElements>{colours}{fonts}\
         <a:fmtScheme name=\"Office\">{fills}{lines}{effects}{backgrounds}</a:fmtScheme>\
         </a:themeElements></a:theme>",
        colours = colour_scheme(),
        fonts = font_scheme(),
    )
}

fn colour_scheme() -> String {
    let entries = [
        ("dk1", "000000"),
        ("lt1", "FFFFFF"),
        ("dk2", "44546A"),
        ("lt2", "E7E6E6"),
        ("accent1", "4472C4"),
        ("accent2", "ED7D31"),
        ("accent3", "A5A5A5"),
        ("accent4", "FFC000"),
        ("accent5", "5B9BD5"),
        ("accent6", "70AD47"),
        ("hlink", "0563C1"),
        ("folHlink", "954F72"),
    ];

    let body: String = entries
        .iter()
        .map(|(name, value)| format!("<a:{name}><a:srgbClr val=\"{value}\"/></a:{name}>"))
        .collect();

    format!("<a:clrScheme name=\"Office\">{body}</a:clrScheme>")
}

fn font_scheme() -> String {
    let face = |kind: &str| {
        format!(
            "<a:{kind}><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/>\
             </a:{kind}>"
        )
    };

    format!(
        "<a:fontScheme name=\"Office\">{}{}</a:fontScheme>",
        face("majorFont"),
        face("minorFont")
    )
}
