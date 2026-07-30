//! Exporting the example deck, through the binary, with nothing mocked.
//!
//! Three features in this repository were implemented, tested, merged and
//! unreachable, and the roadmap now opens by defining a checked box against
//! exactly that. So the unit tests are not the interesting ones here: they build
//! an output directory by hand and ask what the packaging does with it. This
//! runs `slidx export` against `examples/deck`, which means the real Vite, the
//! real plugin, a real browser over the emitted print shell, and then the real
//! archive — and asserts that what came out is the kind of file it claims to be.
//!
//! **A zip that is not a zip is the failure mode of this command.** It looks
//! right in a listing and fails in the one place it is opened, which is on
//! somebody else's machine, usually the day of the talk. Every assertion here is
//! therefore about bytes rather than about existence.
//!
//! ## What it skips, and why that is honest
//!
//! Without the deck's dependencies installed there is no build to drive, and
//! without a browser there is nothing to render frames with. Both are reported
//! and skipped rather than failed, the way the plugin's own browser tests are:
//! there is no way to check rendered output without a renderer, and a mocked one
//! would only prove the mock works. Every skip prints what to run.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use slidx_export::{zip, ExportTarget};

/// The binary under test. `cargo test` puts integration binaries beside it.
fn binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("test binary");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }

    path.join(if cfg!(windows) { "slidx.exe" } else { "slidx" })
}

/// The repository this test is running inside.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("the workspace root")
}

/// Whether the example deck can be built at all.
///
/// Two things have to be there: something to run the build with, and the plugin
/// the deck's config imports. The plugin is consumed through its published
/// `exports`, so a tree that has never run `vp run build:packages` has the
/// source and not the module — and the build fails while loading its config,
/// before slidx is involved at all.
fn buildable() -> bool {
    let root = workspace();

    root.join("node_modules/.bin/vite").is_file()
        && root.join("packages/vite-plugin/dist/index.mjs").is_file()
}

/// Whether a browser is installed, asked the way the plugin asks.
///
/// Playwright being a dependency does not mean Chromium is on the machine; the
/// binaries are a separate download, and CI runs the browser tests in one job
/// that has them.
fn browser_available() -> bool {
    let script = "import('playwright').then((p) => p.chromium.launch()).then((b) => b.close())\
                  .then(() => process.exit(0), () => process.exit(1))";

    Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(script)
        .current_dir(workspace())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// A directory to build into and export into, so two runs cannot collide.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("slidx-e2e-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch");

        Self(root)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs `slidx export` against the example deck, and returns what it wrote.
///
/// Each target builds into its own directory. A build empties its output, so two
/// exports sharing one would take each other's frames away.
fn export(target: ExportTarget, scratch: &Scratch) -> Vec<u8> {
    let root = workspace();
    let token = target.as_token();

    let output = Command::new(binary())
        .arg("export")
        .arg("--target")
        .arg(token)
        .arg(root.join("examples/deck/slides"))
        .arg("--dist")
        .arg(scratch.0.join(format!("dist-{token}")))
        .arg("--out")
        .arg(&scratch.0)
        .current_dir(&root)
        .output()
        .expect("slidx export");

    assert!(
        output.status.success(),
        "`slidx export --target {token}` failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let file = scratch.0.join(target.file_name("making-decks-fast"));

    fs::read(&file).unwrap_or_else(|error| panic!("{} was not written: {error}", file.display()))
}

#[test]
fn the_static_site_export_is_an_archive_holding_the_deck_the_build_wrote() {
    if !buildable() {
        eprintln!("export skipped: no built plugin to drive. `vp run build:packages` first.");
        return;
    }

    let scratch = Scratch::new("site");
    let archive = export(ExportTarget::Browser, &scratch);
    let names = zip::names(&archive);

    assert_eq!(&archive[..4], b"PK\x03\x04", "the site export is not a zip");
    assert!(names.iter().any(|name| name == "slides/index.html"), "{names:?}");
    assert!(names.iter().any(|name| name == "slides/runtime.js"), "{names:?}");
    // The presenter view is part of the deck, and a hand-over that dropped it
    // would hand over less than the URL does.
    assert!(names.iter().any(|name| name.contains("presenter/index.html")), "{names:?}");
    // The frames a previous export staged are not pages of the site.
    assert!(!names.iter().any(|name| name.starts_with("export/")), "{names:?}");
}

#[test]
fn every_rendered_export_is_the_kind_of_file_it_claims_to_be() {
    if !buildable() {
        eprintln!("export skipped: no built plugin to drive. `vp run build:packages` first.");
        return;
    }

    if !browser_available() {
        eprintln!(
            "rendered exports skipped: no browser. `vp exec playwright install chromium` to run them."
        );
        return;
    }

    let scratch = Scratch::new("rendered");

    // One document. A PDF that is not a PDF is refused by every reader that
    // checks the header, which is all of them.
    let document = export(ExportTarget::Pdf, &scratch);
    assert_eq!(&document[..5], b"%PDF-", "the pdf export is not a PDF");

    // One file per slide. The example deck has four.
    let documents = export(ExportTarget::PdfZip, &scratch);
    assert_eq!(&documents[..4], b"PK\x03\x04", "the per-slide export is not a zip");
    assert_eq!(
        zip::names(&documents),
        ["slide-01.pdf", "slide-02.pdf", "slide-03.pdf", "slide-04.pdf"]
    );

    // One image per stop, which is more files than the deck has slides — the
    // whole point of the unit, and the assertion that catches a build that
    // collapsed a staged slide into its punchline.
    let images = export(ExportTarget::Png, &scratch);
    let names = zip::names(&images);

    assert_eq!(&images[..4], b"PK\x03\x04", "the image export is not a zip");
    assert!(names.len() > 4, "one image per slide rather than per stop: {names:?}");
    assert!(names.iter().all(|name| name.ends_with(".png")), "{names:?}");
    assert!(names.iter().any(|name| name.ends_with("-stop-02.png")), "{names:?}");
}
