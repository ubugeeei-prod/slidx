//! Builds the documentation site.
//!
//! ```sh
//! vp run docs:build
//! ```
//!
//! An example rather than a binary, for the reason `slidx_brand`'s emitter is
//! one and `slidx_render`'s preview renderer is one: it is a maintenance
//! command, not part of anything this workspace ships.
//!
//! Nothing has to run it to know the site is correct. `test:rust` reads the real
//! pages, renders every one of them, and fails on a dead link or a page in no
//! section — so a broken site fails CI whether or not anybody built it.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = slidx_brand::assets::workspace_root();
    let out = std::env::args().nth(1).map(Into::into).unwrap_or_else(|| root.join(slidx_docs::OUTPUT_DIR));

    let site = slidx_docs::Site::read(&root.join(slidx_docs::CONTENT_DIR))?;

    for path in site.write(&out)? {
        println!("  {}", path.strip_prefix(&root).unwrap_or(&path).display());
    }

    println!("\n{} page(s) -> {}", site.pages().len(), out.display());
    Ok(())
}
