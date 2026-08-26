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
/// when it exists. The destination is emptied first — including when there is
/// no source — so a file that left `docs/media` cannot sit in
/// `docs/public/media` and ride Vite's `publicDir` into the published site.
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

    written.extend(write_locale(content, generated, &site, "en")?);

    let japanese = content.join("ja");
    if japanese.is_dir() {
        let site = Site::read(&japanese)?;
        written.extend(write_locale(&japanese, &generated.join("ja"), &site, "ja")?);
    }

    if public_media.exists() {
        fs::remove_dir_all(public_media).map_err(|error| format!("media: {error}"))?;
    }

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

fn write_locale(
    source: &Path,
    destination: &Path,
    site: &Site,
    locale: &str,
) -> Result<Vec<PathBuf>, String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("{}: {error}", destination.display()))?;

    let mut written = Vec::new();

    for page in site.pages() {
        let source = fs::read_to_string(source.join(format!("{}.md", page.slug)))
            .map_err(|error| format!("{locale}/{}.md: {error}", page.slug))?;
        let (filled, missing) = generated::fill(&source);
        if let Some(name) = missing.first() {
            return Err(format!(
                "{locale}/{}.md asks for a generated block called {name:?}, which nothing \
                 generates — the ones that exist are {}",
                page.slug,
                generated::NAMES.join(", "),
            ));
        }

        let path = destination.join(format!("{}.md", page.slug));
        fs::write(&path, rewrite_for_site(&filled))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        written.push(path);
    }

    let navigation = destination.join(NAVIGATION_FILE);
    let json = serde_json::to_string_pretty(&navigation_of(site, locale))
        .map_err(|error| format!("{locale} navigation: {error}"))?;
    fs::write(&navigation, format!("{json}\n"))
        .map_err(|error| format!("{}: {error}", navigation.display()))?;
    written.push(navigation);

    Ok(written)
}

fn navigation_of(site: &Site, locale: &str) -> Vec<NavigationGroup> {
    let prefix = if locale == "en" { String::new() } else { format!("/{locale}") };

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
                title: section.label_for(locale).to_string(),
                items: pages
                    .into_iter()
                    .map(|page| NavigationItem {
                        title: page.title.clone(),
                        path: if page.slug == "index" {
                            if prefix.is_empty() {
                                "/".to_string()
                            } else {
                                format!("{prefix}/")
                            }
                        } else {
                            format!("{prefix}/{}", page.slug)
                        },
                    })
                    .collect(),
            })
        })
        .collect()
}

/// Rewrites the two link shapes that work on GitHub and nowhere on the site.
fn rewrite_for_site(source: &str) -> String {
    // A Japanese page sits one directory deeper, so its pictures are two
    // levels up. Rewrite the longer prefix first.
    let with_media = source.replace("../../media/", "/media/").replace("../media/", "/media/");
    rewrite_repository_links(&with_media)
}

/// Rewrites a link that leaves `docs/` for a file in the repository.
///
/// English pages write `../../README.md`. Japanese pages write
/// `../../../README.md`. Both are the same file once the `../` prefixes
/// are stripped, and both are a 404 on the published site unless they
/// become a blob URL.
fn rewrite_repository_links(source: &str) -> String {
    const OPEN: &str = "](";

    let mut out = String::with_capacity(source.len());
    let mut rest = source;

    while let Some(start) = rest.find(OPEN) {
        let after_open = &rest[start + OPEN.len()..];
        if !after_open.starts_with("../") {
            out.push_str(&rest[..start + OPEN.len()]);
            rest = after_open;
            continue;
        }

        let mut path = after_open;
        let mut ups = 0usize;
        while path.starts_with("../") {
            path = &path[3..];
            ups += 1;
        }

        match path.split_once(')') {
            Some((target, remainder)) if ups >= 2 => {
                out.push_str(&rest[..start]);
                out.push_str(OPEN);
                out.push_str(REPOSITORY_BLOB);
                out.push_str(target);
                out.push(')');
                rest = remainder;
            }
            _ => {
                out.push_str(&rest[..start + OPEN.len()]);
                rest = after_open;
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
    fn a_japanese_page_reaches_the_same_repository_file() {
        assert_eq!(
            rewrite_for_site("[the roadmap](../../../ROADMAP.md)"),
            "[the roadmap](https://github.com/ubugeeei-prod/slidx/blob/main/ROADMAP.md)"
        );
        assert_eq!(
            rewrite_for_site("![lint](../../media/terminal-lint-light.png)"),
            "![lint](/media/terminal-lint-light.png)"
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

    fn page(title: &str, section: &str, order: u32, body: &str) -> String {
        format!(
            "---\ntitle: {title}\nsummary: One sentence.\nsection: {section}\norder: {order}\n---\n\n{body}"
        )
    }

    fn scratch(label: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "slidx-docs-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn a_picture_removed_from_media_is_not_left_in_the_public_copy() {
        // Vite's `publicDir` copies whatever is under `docs/public`. A file
        // that left `docs/media` but stayed in `docs/public/media` would
        // publish as if it were still part of the site.
        let root = scratch("stale-media");
        let content = root.join("content");
        let media = root.join("media");
        let public_media = root.join("public");
        fs::create_dir_all(&content).expect("content");
        fs::create_dir_all(&media).expect("media");
        fs::create_dir_all(&public_media).expect("public");
        fs::write(content.join("index.md"), page("Start", "start", 1, "# S\n")).expect("index");
        fs::write(media.join("kept.png"), [1, 2, 3]).expect("kept");
        fs::write(public_media.join("kept.png"), [9, 9, 9]).expect("old kept");
        fs::write(public_media.join("gone.png"), [0]).expect("stale");

        prepare(&content, &root.join("generated"), &media, &public_media).expect("prepares");

        assert!(public_media.join("kept.png").is_file());
        assert_eq!(fs::read(public_media.join("kept.png")).expect("kept bytes"), [1, 2, 3]);
        assert!(!public_media.join("gone.png").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn no_media_directory_clears_the_public_copy() {
        let root = scratch("no-media");
        let content = root.join("content");
        let public_media = root.join("public");
        fs::create_dir_all(&content).expect("content");
        fs::create_dir_all(&public_media).expect("public");
        fs::write(content.join("index.md"), page("Start", "start", 1, "# S\n")).expect("index");
        fs::write(public_media.join("stale.png"), [0]).expect("stale");

        prepare(&content, &root.join("generated"), &root.join("media"), &public_media)
            .expect("prepares");

        assert!(!public_media.exists());

        let _ = fs::remove_dir_all(&root);
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
        assert!(
            generated.join("ja/index.md").is_file(),
            "the locale map has no Japanese front page"
        );
        assert!(generated.join("ja").join(NAVIGATION_FILE).is_file());
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
