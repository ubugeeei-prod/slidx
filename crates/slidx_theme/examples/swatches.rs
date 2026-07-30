//! Prints every built-in theme's resolved colours.
//!
//! ```sh
//! cargo run -p slidx_theme --example swatches
//! ```
//!
//! The palettes are solved rather than written down, so the only way to read
//! what a theme actually ships is to ask it. Kept as an example rather than a
//! test because its output is for a person: a reviewer changing a recipe wants
//! to see the whole family at once, which no assertion communicates.

fn main() {
    for theme in slidx_theme::builtin::all() {
        println!("\n{} — {}", theme.name, theme.description);

        for (scheme, palette) in [("light", &theme.light), ("dark", &theme.dark)] {
            println!("  {scheme}");
            for (role, color) in [
                ("canvas", palette.canvas),
                ("surface", palette.surface),
                ("text", palette.text),
                ("heading", palette.heading),
                ("muted", palette.muted),
                ("accent", palette.accent),
                ("border", palette.border),
                ("codeSurface", palette.code_surface),
                ("codeText", palette.code_text),
            ] {
                println!("    {role:<12} {}", color.to_hex());
            }

            let syntax = palette.syntax();
            for token in slidx_highlight::Token::COLOURED {
                println!("    code.{:<7} {}", token.as_token(), syntax.get(token).to_hex());
            }
        }
    }
}
