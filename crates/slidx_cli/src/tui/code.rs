//! Code in the box, coloured by the same scanner the build uses.
//!
//! [`slidx_highlight`] decides what a keyword is; this only decides what a
//! keyword looks like in a terminal. Inventing a second answer to "is `await` a
//! keyword in this language" would give a deck two highlighters that could
//! disagree, and the one somebody checked would not be the one on the
//! projector.
//!
//! The colours come from the theme's [`SyntaxPalette`], through the same
//! [`Token`] names the CSS classes use — so a theme that recoloured strings
//! recolours them here too, without this file knowing a theme exists beyond
//! asking it.
//!
//! ## Truecolour, and what happens without it
//!
//! A palette is RGB and a terminal may only have 256 colours, or 8, or none.
//! `COLORTERM` is the only signal there is, so it is what decides: truecolour
//! when the terminal advertises it, the 256-colour cube otherwise. Where colour
//! is off entirely the code is drawn plain, which is legible — the point of the
//! box is structure, and structure survives being monochrome.

use slidx_highlight::{scan, Language, Token};
use slidx_lint::Rgba;
use slidx_theme::palette::SyntaxPalette;

use crate::style::{Ink, Style};

/// Draws one line of a fenced block.
///
/// A line at a time rather than the whole block, because the box draws rows and
/// a scanner that spanned rows would need the block re-flowed to match. Every
/// language slidx highlights is line-oriented enough for this: a string that
/// runs across two lines loses its colour on the second, which is a smaller
/// wrong than a row that does not line up with its neighbours.
pub fn render(line: &str, language: Option<&str>, style: &Style) -> String {
    if !style.is_colored() {
        return line.to_string();
    }

    let Some(language) = language.and_then(recognise) else {
        return style.paint(Ink::Faint, line);
    };

    let palette = SyntaxPalette::monochrome(Rgba::opaque(0xd4, 0xd4, 0xd4));
    paint(line, language, &palette, truecolor())
}

/// Draws one line against a theme's own palette.
pub fn render_with(
    line: &str,
    language: Option<&str>,
    palette: &SyntaxPalette,
    style: &Style,
) -> String {
    if !style.is_colored() {
        return line.to_string();
    }

    match language.and_then(recognise) {
        Some(language) => paint(line, language, palette, truecolor()),
        None => style.paint(Ink::Faint, line),
    }
}

fn paint(line: &str, language: Language, palette: &SyntaxPalette, truecolor: bool) -> String {
    let mut out = String::with_capacity(line.len() * 2);
    let mut at = 0;

    for span in scan(line, language) {
        // Anything the scanner did not classify is still text and still has to
        // be drawn: dropping it would silently delete code.
        if span.start > at {
            out.push_str(&line[at..span.start]);
        }

        let text = span.text(line);
        out.push_str(&match span.token {
            Token::Plain => text.to_string(),
            token => wrap(text, palette.get(token), truecolor),
        });

        at = span.end;
    }

    if at < line.len() {
        out.push_str(&line[at..]);
    }

    out
}

/// The language name on a fence, as the highlighter knows it.
///
/// Returns `None` for a fence with no language and for one the highlighter has
/// no scanner for. Both are drawn as plain code rather than guessed at — code
/// coloured by the wrong grammar looks like a lie, which is the same reason
/// [`Token::Plain`] exists.
pub fn recognise(name: &str) -> Option<Language> {
    let name = name.trim().to_lowercase();

    Language::ALL
        .into_iter()
        .find(|language| language.as_token() == name || alias(&name) == Some(*language))
}

/// The names people actually write on a fence.
fn alias(name: &str) -> Option<Language> {
    match name {
        "rs" => Some(Language::Rust),
        "ts" | "tsx" => Some(Language::TypeScript),
        "js" | "jsx" | "mjs" | "cjs" => Some(Language::JavaScript),
        "py" => Some(Language::Python),
        "sh" | "bash" | "zsh" | "console" => Some(Language::Shell),
        _ => None,
    }
}

/// One coloured run.
fn wrap(text: &str, colour: Rgba, truecolor: bool) -> String {
    let code = if truecolor {
        format!("38;2;{};{};{}", colour.r, colour.g, colour.b)
    } else {
        format!("38;5;{}", cube(colour))
    };

    format!("\u{1b}[{code}m{text}\u{1b}[0m")
}

/// The nearest colour in the 256-colour cube.
///
/// The cube is 6×6×6 starting at index 16, with each axis quantised to six
/// levels. Rounding rather than truncating, because truncating drags every
/// colour towards black and a dark theme's comments disappear.
pub fn cube(colour: Rgba) -> u8 {
    let level = |value: u8| -> u32 { ((value as f64 / 255.0) * 5.0).round() as u32 };

    (16 + 36 * level(colour.r) + 6 * level(colour.g) + level(colour.b)) as u8
}

/// Whether this terminal advertises 24-bit colour.
///
/// `COLORTERM` is the only signal that exists. Absent, the 256-colour cube is
/// assumed, which every terminal in use has had for twenty years.
fn truecolor() -> bool {
    std::env::var("COLORTERM")
        .map(|value| value == "truecolor" || value == "24bit")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> SyntaxPalette {
        SyntaxPalette {
            comment: Rgba::opaque(0x6a, 0x99, 0x55),
            string: Rgba::opaque(0xce, 0x91, 0x78),
            number: Rgba::opaque(0xb5, 0xce, 0xa8),
            keyword: Rgba::opaque(0x56, 0x9c, 0xd6),
            type_name: Rgba::opaque(0x4e, 0xc9, 0xb0),
            punctuation: Rgba::opaque(0xd4, 0xd4, 0xd4),
        }
    }

    #[test]
    fn the_scanner_decides_what_a_keyword_is_and_this_only_decides_the_colour() {
        // A second answer to "is `fn` a keyword" would give a deck two
        // highlighters, and the one somebody checked would not be the one on
        // the projector.
        let painted = paint("fn main() {}", Language::Rust, &palette(), true);

        assert!(painted.contains("fn"), "{painted}");
        assert!(painted.contains("\u{1b}["), "{painted}");
    }

    #[test]
    fn every_character_of_the_source_survives_being_painted() {
        // The bug worth guarding: a gap between two spans silently deletes
        // code, and nobody notices until the slide is on a projector.
        for source in
            ["fn main() { let x = 1; }", "// a comment", "let s = \"hello\";", "   indented();", ""]
        {
            let painted = paint(source, Language::Rust, &palette(), true);
            let stripped: String = strip(&painted);

            assert_eq!(stripped, source, "for {source:?}");
        }
    }

    #[test]
    fn a_language_the_highlighter_has_no_scanner_for_is_left_plain() {
        // Code coloured by the wrong grammar looks like a lie, which is the
        // same reason the token set has a Plain at all.
        assert!(recognise("brainfuck").is_none());
        assert!(recognise("").is_none());
    }

    #[test]
    fn the_names_people_actually_write_on_a_fence_are_recognised() {
        assert_eq!(recognise("rs"), Some(Language::Rust));
        assert_eq!(recognise("ts"), Some(Language::TypeScript));
        assert_eq!(recognise("bash"), Some(Language::Shell));
        assert_eq!(recognise("PY"), Some(Language::Python));
    }

    #[test]
    fn every_language_the_highlighter_knows_is_recognised_by_its_own_name() {
        // The drift that would otherwise happen quietly: a language added to
        // the highlighter and not here is code that stops being coloured.
        for language in Language::ALL {
            assert_eq!(recognise(language.as_token()), Some(language), "{}", language.as_token());
        }
    }

    #[test]
    fn a_theme_that_recolours_strings_recolours_them_here() {
        // The palette is the theme's, not this file's. A theme is the only
        // place a colour is chosen.
        let mut warm = palette();
        warm.string = Rgba::opaque(0xff, 0x00, 0x00);

        let painted = paint("let s = \"hi\";", Language::Rust, &warm, true);

        assert!(painted.contains("38;2;255;0;0"), "{painted}");
    }

    #[test]
    fn a_truecolour_terminal_gets_the_exact_colour_the_theme_chose() {
        let painted = paint("// c", Language::Rust, &palette(), true);

        assert!(painted.contains("38;2;106;153;85"), "{painted}");
    }

    #[test]
    fn a_terminal_without_truecolour_gets_the_nearest_colour_it_has() {
        let painted = paint("// c", Language::Rust, &palette(), false);

        assert!(painted.contains("38;5;"), "{painted}");
        assert!(!painted.contains("38;2;"), "{painted}");
    }

    #[test]
    fn the_colour_cube_rounds_rather_than_truncating() {
        // Truncating drags every colour towards black, and a dark theme's
        // comments disappear into the background.
        assert_eq!(cube(Rgba::opaque(0, 0, 0)), 16);
        assert_eq!(cube(Rgba::opaque(255, 255, 255)), 231);
        // 0xd4 is 83% of the way up, which rounds to level 4 of 5, not 3.
        assert_eq!(cube(Rgba::opaque(0xd4, 0, 0)), 16 + 36 * 4);
    }

    #[test]
    fn code_is_drawn_plainly_when_colour_is_off() {
        // The box is about structure, and structure survives being
        // monochrome. Escape codes in a piped frame would not.
        let plain = render("fn main() {}", Some("rust"), &Style::plain());

        assert_eq!(plain, "fn main() {}");
        assert!(!plain.contains('\u{1b}'));
    }

    #[test]
    fn a_theme_palette_reaches_the_line_through_the_public_entry_point() {
        let painted = render_with("// c", Some("rust"), &palette(), &Style::colored());

        assert!(painted.contains('\u{1b}'), "{painted}");
        assert_eq!(strip(&painted), "// c");
    }

    /// The text with every escape sequence removed.
    fn strip(text: &str) -> String {
        let mut out = String::new();
        let mut in_escape = false;

        for character in text.chars() {
            match character {
                '\u{1b}' => in_escape = true,
                'm' if in_escape => in_escape = false,
                _ if !in_escape => out.push(character),
                _ => {}
            }
        }

        out
    }
}
