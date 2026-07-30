//! Every brand file the repository commits, and the check that keeps them true.
//!
//! Nothing under `assets/brand/` is an export from a design tool. Each file is
//! produced from the constants in this crate, and one test compares every
//! committed copy against what the crate generates now — so a colour changed in
//! Rust and not regenerated fails CI instead of leaving the documentation site
//! drawing last month's brand.
//!
//! This is the same arrangement as `crates/slidx_wasm/deck.d.ts`, and it is here
//! for the same reason: the file has to be committed, because its consumers
//! cannot call Rust, and a committed generated file needs something to keep it
//! honest.
//!
//! One list, read by both sides. `cargo run -p slidx_brand --example emit`
//! writes it and the test below reads it, so a file cannot be emitted and
//! unchecked or checked and never emitted.

use crate::palette::Scheme;
use crate::{css, mark, tokens, wordmark};

/// Where the generated brand files live, relative to the workspace root.
pub const DIRECTORY: &str = "assets/brand";

/// One generated file.
#[derive(Debug, Clone)]
pub struct Asset {
    /// File name inside [`DIRECTORY`].
    pub name: &'static str,
    pub contents: String,
}

/// Every file the brand generates.
///
/// The mark ships in five forms because five different consumers need one each,
/// and hand-cropping between them is exactly what this list exists to prevent:
/// two schemes for a page that follows the reader, a single-colour form for a
/// mask or a stencil, and two tiles for the platforms that crop an app icon to
/// their own shape.
pub fn all() -> Vec<Asset> {
    let asset = |name: &'static str, contents: String| Asset { name, contents };

    vec![
        asset("tokens.json", tokens::render_json()),
        asset("tokens.css", css::render()),
        asset("mark-light.svg", mark::render(Scheme::Light)),
        asset("mark-dark.svg", mark::render(Scheme::Dark)),
        // `currentColor` rather than a hex: the one-colour form exists to take
        // the colour of whatever it is placed in.
        asset("mark-mono.svg", mark::render_mono("currentColor")),
        asset("tile-light.svg", mark::render_tile(Scheme::Light)),
        asset("tile-dark.svg", mark::render_tile(Scheme::Dark)),
        asset("wordmark-light.svg", wordmark::render_wordmark(Scheme::Light)),
        asset("wordmark-dark.svg", wordmark::render_wordmark(Scheme::Dark)),
        asset("lockup-light.svg", wordmark::render_lockup(Scheme::Light)),
        asset("lockup-dark.svg", wordmark::render_lockup(Scheme::Dark)),
    ]
}

/// The workspace root, from this crate's manifest.
pub fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// Writes every asset. Used by the emitter, and by nothing else.
pub fn write_all() -> std::io::Result<Vec<std::path::PathBuf>> {
    let directory = workspace_root().join(DIRECTORY);
    std::fs::create_dir_all(&directory)?;

    let mut written = Vec::new();
    for asset in all() {
        let path = directory.join(asset.name);
        std::fs::write(&path, asset.contents)?;
        written.push(path);
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn committed(name: &str) -> Option<String> {
        std::fs::read_to_string(workspace_root().join(DIRECTORY).join(name)).ok()
    }

    #[test]
    fn every_committed_asset_is_what_the_crate_generates() {
        // The contract with the documentation site and with the icon script.
        // Both read the committed files; neither can call this crate.
        for asset in all() {
            let Some(on_disk) = committed(asset.name) else {
                panic!("{DIRECTORY}/{} is missing. Run `vp run generate:brand`.", asset.name);
            };

            assert_eq!(
                on_disk, asset.contents,
                "{DIRECTORY}/{} no longer describes the brand. Run `vp run generate:brand`.",
                asset.name
            );
        }
    }

    #[test]
    fn the_tokens_path_the_crate_publishes_is_the_one_it_writes() {
        // Consumers resolve `TOKENS_PATH`. A rename that only happened in this
        // list would break them silently.
        assert_eq!(tokens::TOKENS_PATH, format!("{DIRECTORY}/tokens.json"));
    }

    #[test]
    fn no_two_assets_share_a_name() {
        let mut names: Vec<&str> = all().iter().map(|asset| asset.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), total, "an asset would overwrite another");
    }

    #[test]
    fn every_asset_is_named_for_something_a_consumer_asks_for() {
        for asset in all() {
            assert!(
                asset.name.ends_with(".svg")
                    || asset.name.ends_with(".json")
                    || asset.name.ends_with(".css"),
                "{} has no extension a consumer can dispatch on",
                asset.name
            );
            assert!(!asset.contents.is_empty(), "{} is empty", asset.name);
        }
    }

    #[test]
    fn both_schemes_of_every_two_scheme_asset_exist() {
        // A page that follows the reader needs both, and a missing one is a page
        // that shows the wrong mark half the time.
        let names: Vec<&str> = all().iter().map(|asset| asset.name).collect();

        for stem in ["mark", "tile", "wordmark", "lockup"] {
            for scheme in Scheme::ALL {
                let expected = format!("{stem}-{}.svg", scheme.as_token());
                assert!(names.contains(&expected.as_str()), "{expected} is not emitted");
            }
        }
    }
}
