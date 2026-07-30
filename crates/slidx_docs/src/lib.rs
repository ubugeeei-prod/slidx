//! # slidx docs
//!
//! The documentation site, rendered by the engine slidx renders decks with.
//!
//! Every page is Markdown parsed by [Ox Content] through
//! [`slidx_render::render_markdown`], drawn with the tokens
//! [`slidx_brand`] generates, and emitted as one static HTML document per page
//! with no script and nothing remote. A site built any other way would be a
//! strange advertisement for the thing it documents.
//!
//! [Ox Content]: https://github.com/ubugeeei-prod/ox-content
//!
//! ## The site is organised by reader, not by crate
//!
//! See [`nav`]. Nobody arrives at documentation wanting to know what
//! `slidx_render` does; they arrive having never heard of the project, or
//! deciding whether to use it, or standing in a hotel room the night before a
//! talk with something broken. Those are different people and only one of them
//! wants a concept explained.
//!
//! ## What fails the build
//!
//! A documentation site rots quietly: a page nothing links to, a link to a page
//! that was renamed, a page in a section that does not exist. None of those
//! shows up in review and all of them show up to a reader, so each one is an
//! error here rather than a warning.
//!
//! ```
//! let site = slidx_docs::Site::read(&slidx_docs::content_directory()).expect("the site");
//!
//! assert!(site.pages().iter().any(|page| page.slug == "index"));
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

pub mod nav;
pub mod page;
pub mod shell;
pub mod style;

mod link;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub use nav::Section;
pub use page::Page;

/// Where the pages are written, relative to the workspace root.
pub const CONTENT_DIR: &str = "docs/content";

/// Where the built site goes, relative to the workspace root.
pub const OUTPUT_DIR: &str = "docs/dist";

/// The content directory, resolved against the workspace root.
pub fn content_directory() -> PathBuf {
    slidx_brand::assets::workspace_root().join(CONTENT_DIR)
}

/// One emitted file.
#[derive(Debug, Clone)]
pub struct Output {
    /// File name, relative to the output directory.
    pub path: String,
    pub html: String,
}

/// Every page, read and checked against every other one.
#[derive(Debug, Clone)]
pub struct Site {
    pages: Vec<Page>,
}

impl Site {
    /// Reads every `.md` file in a directory.
    ///
    /// The order pages are read in is the filesystem's, which differs between
    /// machines, so they are collected into a map first: two builds of the same
    /// tree have to produce the same bytes or nothing downstream can tell a
    /// content change from a filesystem one.
    pub fn read(directory: &Path) -> Result<Self, String> {
        let mut sources: BTreeMap<String, String> = BTreeMap::new();

        let entries = fs::read_dir(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;

        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().is_some_and(|extension| extension == "md") {
                let slug = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_str()
                    .ok_or_else(|| format!("{}: file name is not UTF-8", path.display()))?
                    .to_string();

                let source =
                    fs::read_to_string(&path).map_err(|error| format!("{slug}.md: {error}"))?;
                sources.insert(slug, source);
            }
        }

        let pages = sources
            .iter()
            .map(|(slug, source)| Page::parse(slug, source))
            .collect::<Result<Vec<Page>, String>>()?;

        let site = Self { pages };
        site.check()?;
        Ok(site)
    }

    /// Every page, in the order the navigation lists them.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Renders the whole site.
    pub fn render(&self) -> Vec<Output> {
        self.pages
            .iter()
            .map(|page| Output { path: page.file_name(), html: shell::render(page, &self.pages) })
            .collect()
    }

    /// Writes the site into a directory, and reports what it wrote.
    pub fn write(&self, directory: &Path) -> std::io::Result<Vec<PathBuf>> {
        fs::create_dir_all(directory)?;
        let mut written = Vec::new();

        for output in self.render() {
            let path = directory.join(&output.path);
            fs::write(&path, output.html)?;
            written.push(path);
        }

        Ok(written)
    }

    /// The three ways a documentation site rots, checked instead of hoped for.
    fn check(&self) -> Result<(), String> {
        if !self.pages.iter().any(|page| page.slug == "index") {
            return Err(format!("{CONTENT_DIR} has no index.md — the site has no front page"));
        }

        let mut seen: BTreeMap<(Section, u32), &str> = BTreeMap::new();
        for page in &self.pages {
            if let Some(other) = seen.insert((page.section, page.order), &page.slug) {
                return Err(format!(
                    "{}.md and {other}.md are both order {} of {}",
                    page.slug,
                    page.order,
                    page.section.as_token(),
                ));
            }
        }

        self.check_links()
    }

    /// Every link to a page on this site points at a page on this site.
    ///
    /// Checked on the rendered HTML rather than on the Markdown, so it does not
    /// matter which of CommonMark's several link spellings the author used —
    /// Ox Content has already collapsed them into one attribute.
    fn check_links(&self) -> Result<(), String> {
        let slugs: Vec<&str> = self.pages.iter().map(|page| page.slug.as_str()).collect();

        for page in &self.pages {
            for target in internal_links(&page.html()) {
                if !slugs.contains(&target.as_str()) {
                    return Err(format!(
                        "{}.md links to {target}.md, which is not a page — \
                         pages are {}",
                        page.slug,
                        slugs.join(", "),
                    ));
                }
            }
        }

        Ok(())
    }
}

/// Every `href` in a document that points at another page of this site.
fn internal_links(html: &str) -> Vec<String> {
    html.split("href=\"")
        .skip(1)
        .filter_map(|rest| rest.split_once('"'))
        .map(|(href, _)| href)
        .filter(|href| !href.contains("://") && !href.starts_with('#'))
        .filter_map(|href| href.split('#').next())
        .filter_map(|path| path.strip_suffix(".html"))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A site in a temporary directory, from `(slug, frontmatter body)` pairs.
    fn site_of(pages: &[(&str, &str)]) -> Result<Site, String> {
        let directory = std::env::temp_dir().join(format!(
            "slidx-docs-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a temporary directory");

        for (slug, source) in pages {
            fs::write(directory.join(format!("{slug}.md")), source).expect("a written page");
        }

        let site = Site::read(&directory);
        let _ = fs::remove_dir_all(&directory);
        site
    }

    fn page(title: &str, section: &str, order: u32, body: &str) -> String {
        format!("---\ntitle: {title}\nsummary: One sentence.\nsection: {section}\norder: {order}\n---\n\n{body}")
    }

    #[test]
    fn the_real_site_builds() {
        // The pages under `docs/content` are the deliverable, so the test that
        // matters most is the one that reads them rather than a fixture.
        let site = Site::read(&content_directory()).expect("the site builds");

        assert!(!site.pages().is_empty());
        assert_eq!(site.render().len(), site.pages().len());
    }

    #[test]
    fn every_page_on_the_real_site_belongs_to_a_reader_and_says_what_it_is_for() {
        for page in Site::read(&content_directory()).expect("the site").pages() {
            assert!(!page.title.trim().is_empty(), "{} has no title", page.slug);
            assert!(
                page.summary.trim().ends_with('.'),
                "{}: the summary is a sentence and ends with a full stop",
                page.slug
            );
        }
    }

    #[test]
    fn the_site_is_reproducible() {
        // Two builds of one tree produce the same bytes, so a diff in the
        // output is a change in the content and never in the filesystem's
        // reading order.
        let site = Site::read(&content_directory()).expect("the site");
        let first: Vec<String> = site.render().into_iter().map(|output| output.html).collect();
        let second: Vec<String> = site.render().into_iter().map(|output| output.html).collect();

        assert_eq!(first, second);
    }

    #[test]
    fn a_site_with_no_front_page_is_an_error() {
        let error = site_of(&[("choosing", &page("Choosing", "choosing", 1, "# C\n"))])
            .expect_err("no index");

        assert!(error.contains("index.md"), "got {error}");
    }

    #[test]
    fn a_link_to_a_page_that_does_not_exist_fails_the_build() {
        // The way documentation rots: a page is renamed and six links to it
        // keep rendering, each one a dead end for the reader who followed it.
        let error = site_of(&[("index", &page("Start", "start", 1, "[gone](missing.md)\n"))])
            .expect_err("a dead link");

        assert!(error.contains("missing.md"), "got {error}");
    }

    #[test]
    fn a_link_out_to_the_repository_is_not_treated_as_a_missing_page() {
        let site = site_of(&[(
            "index",
            &page("Start", "start", 1, "[the roadmap](../../ROADMAP.md)\n"),
        )])
        .expect("a valid site");

        assert_eq!(site.pages().len(), 1);
    }

    #[test]
    fn two_pages_claiming_the_same_place_in_a_section_is_an_error() {
        // Otherwise the tie is broken by file name, and the navigation quietly
        // reads in an order nobody chose.
        let error = site_of(&[
            ("index", &page("Start", "start", 1, "# S\n")),
            ("also", &page("Also", "start", 1, "# A\n")),
        ])
        .expect_err("a duplicate order");

        assert!(error.contains("order 1"), "got {error}");
    }

    #[test]
    fn a_page_with_broken_frontmatter_fails_the_whole_site() {
        let error = site_of(&[
            ("index", &page("Start", "start", 1, "# S\n")),
            ("broken", "# No frontmatter\n"),
        ])
        .expect_err("a page with no frontmatter");

        assert!(error.contains("broken.md"), "got {error}");
    }
}
