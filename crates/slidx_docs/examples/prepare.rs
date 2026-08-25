//! Prepares the authored pages for the Ox Content 3 site generator.
//!
//! ```sh
//! vp run docs:prepare
//! ```
//!
//! An example rather than a binary, for the reason `slidx_brand`'s emitter is
//! one: it is a maintenance command, not part of anything this workspace ships.
//!
//! Checking still happens in `test:rust`. This writes the generated tree the
//! Vite plugin builds.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = slidx_brand::assets::workspace_root();

    for path in slidx_docs::prepare_workspace()? {
        println!("  {}", path.strip_prefix(&root).unwrap_or(&path).display());
    }

    Ok(())
}
