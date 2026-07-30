//! One documentation page: its frontmatter, and its body as HTML.
//!
//! # Why this is not a slide
//!
//! Every other Markdown file in this repository goes through
//! [`slidx_core::parse_deck`], and a documentation page deliberately does not.
//! A deck parse strips `<!-- notes: -->` comments and compiles `[text]{.accent}`
//! into a span — both correct for a slide, and both wrong for a page whose job
//! includes *showing* that syntax in a sentence. It also splits on `---`, and a
//! document is one document.
//!
//! So the frontmatter block is located here and its contents handed to
//! [`slidx_core::frontmatter`], which is the module that owns what a key means.
//! The body then goes to [`slidx_render::render_markdown`] — Ox Content, the
//! same engine that renders every slide slidx builds.

use slidx_core::{frontmatter, Diagnostics};
use slidx_render::{render_markdown, MarkdownOptions};

use crate::link;
use crate::nav::Section;

/// A page, read and validated.
#[derive(Debug, Clone)]
pub struct Page {
    /// The file stem, which is also the published file name and the URL.
    pub slug: String,
    /// The `<h1>` a reader sees and the name the navigation uses.
    pub title: String,
    /// One sentence: the page's description, and the hint under its nav entry.
    pub summary: String,
    pub section: Section,
    /// Position within the section.
    pub order: u32,
    /// The Markdown after the frontmatter block.
    pub body: String,
}

impl Page {
    /// Reads one page.
    ///
    /// Every frontmatter key is required, and that is not strictness for its own
    /// sake. A page with no `summary` publishes an empty description and a nav
    /// entry that says nothing; a page with no `section` belongs to no reader
    /// and appears in no navigation. Both are invisible in review and obvious to
    /// a reader who cannot find the page.
    pub fn parse(slug: &str, source: &str) -> Result<Self, String> {
        let (matter, body) =
            split_frontmatter(source).ok_or_else(|| format!("{slug}.md: no frontmatter block"))?;

        // The block's own diagnostics are dropped rather than reported: a
        // malformed one yields an empty object, and the required-key errors
        // below then name the missing keys, which is the more useful message.
        let mut diagnostics = Diagnostics::default();
        let matter = frontmatter::parse(matter, 1, &mut diagnostics);

        let required = |key: &str| {
            frontmatter::string(&matter, key)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| format!("{slug}.md: frontmatter has no {key}:"))
        };

        let section_token = required("section")?;
        let section = Section::parse(&section_token).ok_or_else(|| {
            let known: Vec<&str> = Section::ALL.iter().map(|section| section.as_token()).collect();
            format!("{slug}.md: unknown section {section_token:?} — one of {}", known.join(", "))
        })?;

        // Read as a number rather than through `frontmatter::string`, because
        // YAML gives `order: 1` as one and a page would otherwise have to quote
        // it to be read at all.
        let order = matter
            .get("order")
            .and_then(serde_json::Value::as_u64)
            .and_then(|order| u32::try_from(order).ok())
            .ok_or_else(|| format!("{slug}.md: order: must be a whole number"))?;

        Ok(Self {
            slug: slug.to_string(),
            title: required("title")?,
            summary: required("summary")?,
            section,
            order,
            body: body.to_string(),
        })
    }

    /// The file this page is published as.
    ///
    /// Flat, with the extension left on. A published page is opened from a
    /// GitHub Pages URL and from a `file://` path on the machine that built it,
    /// and `start.html` is the one spelling that resolves under both — which is
    /// the same reason a built deck is one document per slide rather than a
    /// route table.
    pub fn file_name(&self) -> String {
        format!("{}.html", self.slug)
    }

    /// The body as HTML.
    ///
    /// Ox Content with GFM on, because these pages are full of tables, and with
    /// highlighting on, because they are full of Rust, TypeScript, YAML and
    /// shell. Both are the deck defaults, so the code on this site is coloured
    /// by the scanner that colours a slide.
    pub fn html(&self) -> String {
        link::rewrite(&render_markdown(&self.body, &MarkdownOptions::default()))
    }
}

/// Splits a leading `---` fenced block from the body.
///
/// Returns `None` when the file does not open with one, which is an error rather
/// than a default: the frontmatter is where a page says which reader it is for.
fn split_frontmatter(source: &str) -> Option<(&str, &str)> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let rest = source.strip_prefix("---\n").or_else(|| source.strip_prefix("---\r\n"))?;

    // The first line that is exactly a closing fence. Searching for the line
    // rather than the substring is what keeps a `---` inside the YAML — a value
    // like `summary: "a --- b"` — from closing the block early.
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATTER: &str = concat!(
        "---\n",
        "title: Start\n",
        "summary: From nothing to a deck you have presented.\n",
        "section: start\n",
        "order: 1\n",
        "---\n",
    );

    fn page(body: &str) -> Page {
        Page::parse("index", &format!("{MATTER}\n{body}")).expect("a valid page")
    }

    #[test]
    fn a_page_carries_the_frontmatter_the_navigation_needs() {
        let page = page("# Start\n");

        assert_eq!(page.title, "Start");
        assert_eq!(page.section, Section::Start);
        assert_eq!(page.order, 1);
        assert!(page.summary.starts_with("From nothing"));
    }

    #[test]
    fn the_body_is_rendered_by_the_engine_that_renders_slides() {
        // Ox Content, reached the way `slidx_render` reaches it. A site rendered
        // by something else would be a strange advertisement for the engine.
        let html = page("# Start\n\nSome **prose**.\n").html();

        assert!(html.contains("<h1"));
        assert!(html.contains("<strong>prose</strong>"));
    }

    #[test]
    fn gfm_tables_render_because_these_pages_are_full_of_them() {
        let html = page("| rule | catches |\n| ---- | ------- |\n| a | b |\n").html();
        assert!(html.contains("<th>rule</th>"));
    }

    #[test]
    fn code_arrives_highlighted_by_the_scanner_that_colours_a_slide() {
        let html = page("```rust\nfn main() {}\n```\n").html();

        assert!(html.contains("language-rust"));
        assert!(html.contains("slidx-code-keyword"));
    }

    #[test]
    fn a_heading_carries_an_id_so_one_answer_can_be_linked_precisely() {
        // The night-before page is a list of symptoms, and the useful thing to
        // send a speaker is a link to the symptom rather than to the page.
        assert!(page("## The demo died\n").html().contains("id=\"the-demo-died\""));
    }

    #[test]
    fn links_between_pages_are_rewritten_for_the_site() {
        let html = page("See [the night before](tonight.md).\n").html();
        assert!(html.contains(r#"href="tonight.html""#), "got {html}");
    }

    #[test]
    fn mark_syntax_in_prose_is_left_as_written_because_a_page_is_not_a_slide() {
        // A page documenting `[text]{.accent}` has to be able to print it. Run
        // through the deck parser it would come out as a compiled span, and the
        // one page explaining the syntax would be the one page not showing it.
        let html = page("Write `[3.2x faster]{#result .accent}` in a slide.\n").html();

        assert!(html.contains("{#result .accent}"), "the mark was compiled: {html}");
        assert!(!html.contains("data-slidx-mark"));
    }

    #[test]
    fn a_note_comment_in_prose_survives_because_a_page_is_not_a_slide() {
        let html = page("A slide's notes live in `<!-- notes: … -->`.\n").html();
        assert!(html.contains("notes:"), "the note was extracted: {html}");
    }

    #[test]
    fn a_page_with_no_frontmatter_is_an_error() {
        let error = Page::parse("stray", "# Stray\n").expect_err("no frontmatter");
        assert!(error.contains("no frontmatter"), "got {error}");
    }

    #[test]
    fn a_page_missing_a_required_key_names_the_key() {
        let source = "---\ntitle: Start\nsection: start\norder: 1\n---\n";
        let error = Page::parse("index", source).expect_err("no summary");

        assert!(error.contains("summary"), "got {error}");
    }

    #[test]
    fn a_page_naming_a_section_that_does_not_exist_is_an_error_that_lists_the_ones_that_do() {
        let source = "---\ntitle: T\nsummary: S\nsection: guides\norder: 1\n---\n";
        let error = Page::parse("odd", source).expect_err("unknown section");

        assert!(error.contains("guides"));
        assert!(error.contains("tonight"), "the message should list the real sections: {error}");
    }

    #[test]
    fn an_order_that_is_not_a_number_is_an_error() {
        let source = "---\ntitle: T\nsummary: S\nsection: start\norder: first\n---\n";
        assert!(Page::parse("odd", source).is_err());
    }

    #[test]
    fn a_horizontal_rule_in_the_body_does_not_close_the_frontmatter() {
        // The closing fence is a whole line. A `---` further down is a thematic
        // break, and reading it as the end of the block would silently truncate
        // the page at its first rule.
        let page = page("# Start\n\nOne.\n\n---\n\nTwo.\n");

        assert!(page.body.contains("Two."));
        assert!(page.html().contains("Two."));
    }

    #[test]
    fn the_published_file_name_keeps_its_extension() {
        // So the same link resolves from a Pages URL and from a `file://` path.
        assert_eq!(page("# Start\n").file_name(), "index.html");
    }
}
