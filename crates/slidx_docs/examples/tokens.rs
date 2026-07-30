//! Prints the CSS the documentation site draws with.
//!
//! The brand's tokens and the deck theme's code colours, in the order the site
//! puts them. `scripts/record.mjs` draws captured terminal output with the same
//! values, and it cannot call Rust — so it asks for them here rather than
//! keeping a second copy that would be wrong the first time a colour moved.
//!
//! ```sh
//! cargo run -q -p slidx_docs --example tokens
//! ```

fn main() {
    println!("{}", slidx_brand::css::render());
    println!("{}", slidx_theme::css::render(&slidx_theme::default_theme()));
}
