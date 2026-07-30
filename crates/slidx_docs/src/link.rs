//! Rewriting the links, so one Markdown file works in two places.
//!
//! The pages under `docs/content/` are read twice: on the site, and in the
//! repository by anyone who clicks a `.md` file on GitHub. Writing them twice
//! would mean maintaining them twice, so they are written once as plain
//! Markdown with plain relative links — the spelling that works on GitHub — and
//! the two link shapes that would break on the site are rewritten here.
//!
//! **A sibling page** `start.md` becomes `start.html`, because the site emits
//! one file per page and nothing serves a `.md`.
//!
//! **A link that leaves the site** — `../../ROADMAP.md`, the source of truth
//! this documentation is under orders to link rather than copy — becomes a URL
//! into the repository. A relative path out of `docs/content/` resolves to a
//! file that the site does not publish, so left alone it would be a 404 on
//! every page that did the honest thing.

/// Where a link that leaves the site points instead.
///
/// The default branch rather than a tag: the documentation describes the tree
/// it merged into, and a reader following a link to `ROADMAP.md` wants the
/// roadmap as it stands, not as it stood at the last release.
const REPOSITORY_BLOB: &str = "https://github.com/ubugeeei-prod/slidx/blob/main/";

/// Rewrites every relative Markdown link in a rendered page.
///
/// Applied to the HTML rather than to the Markdown, so nothing here has to know
/// which of the several link spellings CommonMark accepts produced the `href`.
/// Ox Content has already collapsed them all into one attribute by this point.
pub fn rewrite(html: &str) -> String {
    replace_hrefs(html, rewrite_href)
}

/// One `href`, as the site should spell it.
fn rewrite_href(href: &str) -> String {
    // Anything absolute, protocol-relative, or an in-page anchor is already
    // pointing where its author meant it to.
    if href.starts_with('#') || href.contains("://") || href.starts_with("//") {
        return href.to_string();
    }

    let (path, fragment) = match href.split_once('#') {
        Some((path, fragment)) => (path, format!("#{fragment}")),
        None => (href, String::new()),
    };

    if !path.ends_with(".md") {
        return href.to_string();
    }

    if path.starts_with("../") {
        // Out of `docs/content/` and therefore out of the site. `docs/content`
        // is two directories deep, so the first two `../` are the ones that
        // reach the repository root and anything after them is a real path.
        let repository_path = path.trim_start_matches("../");
        return format!("{REPOSITORY_BLOB}{repository_path}{fragment}");
    }

    format!("{}.html{fragment}", path.trim_end_matches(".md"))
}

/// Applies a function to the value of every `href="…"` in a document.
///
/// A four-line scanner rather than a dependency: the input is this crate's own
/// renderer output, where an `href` is always double-quoted and its value
/// already escaped, so the two things a real parser would buy — attribute
/// spelling variants and entity decoding — are both already settled.
fn replace_hrefs(html: &str, mut f: impl FnMut(&str) -> String) -> String {
    const OPEN: &str = "href=\"";

    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(start) = rest.find(OPEN) {
        let (before, after) = rest.split_at(start + OPEN.len());
        out.push_str(before);

        match after.split_once('"') {
            Some((href, remainder)) => {
                out.push_str(&f(href));
                out.push('"');
                rest = remainder;
            }
            None => {
                rest = after;
                break;
            }
        }
    }

    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_to_a_sibling_page_becomes_a_link_to_its_html() {
        assert_eq!(rewrite(r#"<a href="tonight.md">"#), r#"<a href="tonight.html">"#);
    }

    #[test]
    fn a_fragment_survives_the_rewrite() {
        // Every heading on a page carries an id, so a link into the middle of
        // another page is the normal way to answer a question precisely.
        assert_eq!(
            rewrite(r#"<a href="tonight.md#the-demo-died">"#),
            r#"<a href="tonight.html#the-demo-died">"#
        );
    }

    #[test]
    fn a_link_out_of_the_site_becomes_a_link_into_the_repository() {
        // ROADMAP.md is the honest place for anything not yet built, and a
        // relative path to it from a published page reaches nothing.
        assert_eq!(
            rewrite(r#"<a href="../../ROADMAP.md">"#),
            r#"<a href="https://github.com/ubugeeei-prod/slidx/blob/main/ROADMAP.md">"#
        );
    }

    #[test]
    fn a_link_into_a_repository_file_keeps_the_rest_of_its_path() {
        assert_eq!(
            rewrite(r#"<a href="../../CONTRIBUTING.md#checks">"#),
            r#"<a href="https://github.com/ubugeeei-prod/slidx/blob/main/CONTRIBUTING.md#checks">"#
        );
    }

    #[test]
    fn an_anchor_on_the_same_page_is_left_alone() {
        let html = r##"<a href="#what-you-have-now">"##;
        assert_eq!(rewrite(html), html);
    }

    #[test]
    fn an_absolute_url_is_left_alone() {
        let html = r#"<a href="https://github.com/ubugeeei-prod/ox-content">"#;
        assert_eq!(rewrite(html), html);
    }

    #[test]
    fn a_link_to_an_image_is_left_alone() {
        // The screenshots are published beside the pages, so their relative
        // paths already resolve.
        assert_eq!(
            rewrite(r#"<img src="images/2-light.png">"#),
            r#"<img src="images/2-light.png">"#
        );
    }

    #[test]
    fn every_link_in_a_document_is_rewritten_and_not_just_the_first() {
        let html = r#"<a href="a.md">a</a> and <a href="b.md">b</a>"#;
        assert_eq!(rewrite(html), r#"<a href="a.html">a</a> and <a href="b.html">b</a>"#);
    }

    #[test]
    fn text_after_the_last_link_survives() {
        let html = r#"<a href="a.md">a</a> and then some prose."#;
        assert!(rewrite(html).ends_with("and then some prose."));
    }
}
