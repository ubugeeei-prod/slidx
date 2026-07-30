//! Writes every generated brand file.
//!
//! ```sh
//! vp run generate:brand
//! ```
//!
//! An example rather than a binary because it is a maintenance command, not part
//! of the shipped crate — the same reason `slidx_render`'s preview renderer is
//! one. The files it writes are committed, and
//! `slidx_brand::assets::every_committed_asset_is_what_the_crate_generates`
//! fails when they stop matching, so this is how you fix that failure rather
//! than something a build has to run.

fn main() -> std::io::Result<()> {
    let root = slidx_brand::assets::workspace_root();

    for path in slidx_brand::assets::write_all()? {
        let shown = path.strip_prefix(&root).unwrap_or(&path);
        println!("  {}", shown.display());
    }

    Ok(())
}
