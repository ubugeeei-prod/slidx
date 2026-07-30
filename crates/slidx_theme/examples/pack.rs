//! Writes the theme document `@slidx/theme-workshop` ships.
//!
//! ```sh
//! vp run generate:theme
//! ```
//!
//! An example rather than a binary for the same reason `slidx_brand`'s emitter
//! is one: it is a maintenance command and not part of the shipped crate. The
//! file it writes is committed, and
//! `slidx_theme::published::the_committed_document_is_what_this_module_produces`
//! fails when the two stop agreeing — so this is how that failure is fixed,
//! not something a build has to run.

fn main() -> std::io::Result<()> {
    let path = slidx_theme::published::write()?;
    println!("  {}", path.display());

    Ok(())
}
