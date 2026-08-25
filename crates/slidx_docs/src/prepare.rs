//! Prepare Markdown for the Ox Content 3 SSG.
//!
//! The pages under [`crate::CONTENT_DIR`] stay the files a reader opens on
//! GitHub: relative links, `../media/` pictures, and `<!-- slidx-docs: … -->`
//! placeholders. Ox Content's Vite plugin cannot fill those placeholders or
//! rewrite a link that leaves the site, so this module writes a generated
//! tree the plugin actually builds.
//!
//! Checking still happens on the authored pages, through [`crate::Site::read`].
//! Publishing happens on what this writes. The two trees cannot drift on
//! placeholders: a name nothing generates fails the site check before anything
//! is copied.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::generated;
use crate::nav::Section;
use crate::{copy_into, Site, CONTENT_DIR, MEDIA_DIR};

/// Where filled Markdown is written, relative to the workspace root.
pub const GENERATED_DIR: &str = "docs/.generated";

/// Where pictures and recordings are copied for Vite's `publicDir`.
pub const PUBLIC_MEDIA_DIR: &str = "docs/public/media";

/// The navigation file the Vite config reads.
pub const NAVIGATION_FILE: &str = "navigation.json";

/// Where a link that leaves the site points instead.
const REPOSITORY_BLOB: &str = "https://github.com/ubugeeei-prod/slidx/blob/main/";

/// Fills placeholders, rewrites site-only paths, and writes the generated tree.
///
/// `content` is the authored directory. `generated` receives one Markdown file
/// per page plus [`NAVIGATION_FILE`]. `media` is copied into `public_media`
/// when it exists.
pub fn prepare(
    content: &Path,
    generated: &Path,
    media: &Path,
    public_media: &Path,
) -> Result<Vec<PathBuf>, String> {
    let site = Site::read(content)?;
    let mut written = Vec::new();

    if generated.exists() {
        fs::remove_dir_all(generated)
            .map_err(|error| format!("{}: {error}", generated.display()))?;
    }
    fs::create_dir_all(generated).map_err(|error| format!("{}: {error}", generated.display()))?;

    for page in site.pages() {
        let source = fs::read_to_string(content.join(format!("{}.md", page.slug)))
            .map_err(|error| format!("{}.md: {error}", page.slug))?;
        let (filled, missing) = generated::fill(&source);
        if let Some(name) = missing.first() {
            return Err(format!(
                "{}.md asks for a generated block called {name:?}, which nothing \
                 generates — the ones that exist are {}",
                page.slug,
                generated::NAMES.join(", "),
            ));
        }

        let path = generated.join(format!("{}.md", page.slug));
        fs::write(&path, rewrite_for_site(&filled))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        written.push(path);
    }

    let navigation = generated.join(NAVIGATION_FILE);
    let json = serde_json::to_string_pretty(&navigation_of(&site))
        .map_err(|error| format!("navigation: {error}"))?;
    fs::write(&navigation, format!("{json}\n"))
        .map_err(|error| format!("{}: {error}", navigation.display()))?;
    written.push(navigation);

    if media.is_dir() {
        written.extend(copy_into(media, public_media).map_err(|error| format!("media: {error}"))?);
    }

    Ok(written)
}

/// Prepares the real documentation tree in this repository.
pub fn prepare_workspace() -> Result<Vec<PathBuf>, String> {
    let root = slidx_brand::assets::workspace_root();
    prepare(
        &root.join(CONTENT_DIR),
        &root.join(GENERATED_DIR),
        &root.join(MEDIA_DIR),
        &root.join(PUBLIC_MEDIA_DIR),
    )
}

#[derive(Debug, Serialize)]
struct NavigationGroup {
    title: String,
    items: Vec<NavigationItem>,
}

#[derive(Debug, Serialize)]
struct NavigationItem {
    title: String,
    path: String,
}

fn navigation_of(site: &Site) -> Vec<NavigationGroup> {
    Section::ALL
        .into_iter()
        .filter_map(|section| {
            let mut pages: Vec<_> =
                site.pages().iter().filter(|page| page.section == section).collect();
            if pages.is_empty() {
                return None;
            }
            pages.sort_by_key(|page| page.order);
            Some(NavigationGroup {
                title: section.label().to_string(),
                items: pages
                    .into_iter()
                    .map(|page| NavigationItem {
                        title: page.title.clone(),
                        path: if page.slug == "index" {
                            "/".to_string()
                        } else {
                            format!("/{}", page.slug)
                        },
                    })
                    .collect(),
            })
        })
        .collect()
}

/// Rewrites the two link shapes that work on GitHub and nowhere on the site.
fn rewrite_for_site(source: &str) -> String {
    let with_media = source.replace("../media/", "/media/");
    rewrite_repository_links(&with_media)
}

fn rewrite_repository_links(source: &str) -> String {
    const MARK: &str = "](../../";

    let mut out = String::with_capacity(source.len());
    let mut rest = source;

    while let Some(start) = rest.find(MARK) {
        let (before, after) = rest.split_at(start);
        out.push_str(before);
        out.push_str("](");
        out.push_str(REPOSITORY_BLOB);

        let path = &after[MARK.len()..];
        match path.split_once(')') {
            Some((target, remainder)) => {
                out.push_str(target);
                out.push(')');
                rest = remainder;
            }
            None => {
                rest = path;
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
    fn a_picture_is_published_at_the_site_root_rather_than_one_level_up() {
        assert_eq!(
            rewrite_for_site("![lint](../media/terminal-lint-light.png)"),
            "![lint](/media/terminal-lint-light.png)"
        );
    }

    #[test]
    fn a_link_out_of_the_site_becomes_a_link_into_the_repository() {
        assert_eq!(
            rewrite_for_site("[the roadmap](../../ROADMAP.md)"),
            "[the roadmap](https://github.com/ubugeeei-prod/slidx/blob/main/ROADMAP.md)"
        );
    }

    #[test]
    fn a_link_into_a_crate_keeps_the_rest_of_its_path() {
        assert_eq!(
            rewrite_for_site("[keys](../../crates/slidx_lsp/src/vocabulary.rs)"),
            "[keys](https://github.com/ubugeeei-prod/slidx/blob/main/crates/slidx_lsp/src/vocabulary.rs)"
        );
    }

    #[test]
    fn a_sibling_page_is_left_for_ox_content_to_resolve() {
        let source = "See [the night before](tonight.md).";
        assert_eq!(rewrite_for_site(source), source);
    }

    #[test]
    fn the_real_site_prepares_without_a_hole_where_a_table_should_be() {
        let directory = std::env::temp_dir().join(format!(
            "slidx-docs-prepare-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&directory);
        let generated = directory.join("generated");
        let public = directory.join("media");

        prepare(
            &crate::content_directory(),
            &generated,
            &slidx_brand::assets::workspace_root().join(MEDIA_DIR),
            &public,
        )
        .expect("the site prepares");

        assert!(generated.join("index.md").is_file());
        assert!(generated.join(NAVIGATION_FILE).is_file());
        let index = fs::read_to_string(generated.join("index.md")).expect("index");
        assert!(!index.contains("<!-- slidx-docs:"), "a placeholder survived: {index}");

        let navigation: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(generated.join(NAVIGATION_FILE)).expect("navigation"),
        )
        .expect("navigation json");
        let titles: Vec<&str> = navigation
            .as_array()
            .expect("a list of groups")
            .iter()
            .map(|group| group["title"].as_str().expect("a title"))
            .collect();
        assert_eq!(titles, ["Start", "Choosing slidx", "The night before", "Reference"]);

        let _ = fs::remove_dir_all(&directory);
    }
}
