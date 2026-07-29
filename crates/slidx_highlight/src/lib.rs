//! # slidx highlight
//!
//! Readable code on a slide, decided at build time and delivered as class
//! names.
//!
//! ## Why there is no highlighter in the browser
//!
//! An audience slide ships zero JavaScript. That is not a size budget — it is
//! the offline guarantee and the failure model. A deck is opened from a USB
//! stick, over a hotel connection, on a machine that is not the author's, three
//! minutes before a talk; every script on that page is one more thing that can
//! fail to load, fail to parse, or paint late in front of two hundred people.
//! Prism and Shiki are excellent and neither is available here, because both
//! answer the question *at display time* and display time is the one moment
//! slidx refuses to depend on.
//!
//! Highlighting is a pure function of the source and the language. Running it
//! during the build turns a runtime risk into a string, and the string is
//! already in the document by the time the room sees it.
//!
//! ## Why there is no highlighting dependency
//!
//! `syntect` brings a regex engine and a binary grammar format. This crate also
//! compiles to WebAssembly, which ships to every viewer of every deck, and both
//! of those are expensive there — a grammar bundle dwarfs the pipeline that
//! reads it.
//!
//! So the scanner here is small and honest: comments, strings, numbers,
//! keywords, types, and punctuation, for the six languages a technical talk
//! actually shows. Six read by a person beats forty transcribed from grammar
//! files nobody in this repository has checked.
//!
//! ## What it gives up
//!
//! Every ambiguity resolves towards *plain text*, never towards a guess:
//!
//! - A JavaScript `/` is division. A regular expression literal stays plain,
//!   because the heuristic that would colour it also colours `a / b / c` as a
//!   string.
//! - A Rust `'` that does not close within a character literal's length is a
//!   lifetime, and goes out plain along with its name.
//! - A string that never closes ends the highlighting rather than painting the
//!   rest of the block.
//! - An interpolation — `${…}`, an f-string's `{…}` — is part of its string.
//! - An unrecognised language is not highlighted at all.
//!
//! Each of those is a test in this crate rather than a paragraph anyone has to
//! trust.
//!
//! ## Colours
//!
//! None here. A [`Token`] becomes a class name, `slidx_theme` owns what the
//! class means, and the theme audit holds every one of those colours to the
//! same contrast rules as body text — because a comment colour that is
//! illegible on a projector is exactly the failure this project exists to
//! catch.
//!
//! ```
//! use slidx_highlight::{to_html, Language};
//!
//! let html = to_html("let x = 1; // one", Some(Language::Rust));
//! assert!(html.contains(r#"<span class="slidx-code-keyword">let</span>"#));
//! assert!(html.contains(r#"<span class="slidx-code-comment">// one</span>"#));
//!
//! // A language nobody wrote a scanner for is shown as it was written.
//! assert_eq!(to_html("let x = 1", Language::parse("brainfuck")), "let x = 1");
//! ```

#![deny(missing_debug_implementations)]
#![warn(clippy::all)]

mod language;
mod render;
mod scan;
mod syntax;
mod token;

pub use language::Language;
pub use render::to_html;
pub use scan::scan;
pub use token::{Span, Token};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spans_cover_the_source_exactly_once() {
        // The property every renderer downstream relies on: concatenating the
        // spans reproduces the input. A gap silently drops code from a slide,
        // and an overlap silently duplicates it.
        let samples = [
            (Language::Rust, "fn main() { let s = \"héllo\"; /* 日本語 */ }"),
            (Language::TypeScript, "const a: Array<number> = [1, 2]; // ok"),
            (Language::JavaScript, "export default () => `a${b}c`;"),
            (Language::Python, "def f(x: int) -> str:\n    return f'{x}'\n"),
            (Language::Json, "{\"a\": [1, true, null], \"b\": \"c\"}"),
            (Language::Html, "<p class=\"x\">a &amp; b</p><!-- note -->"),
            (Language::Shell, "npm i -D @slidx/vite-plugin # install"),
        ];

        for (language, source) in samples {
            let spans = scan(source, language);
            let rebuilt: String = spans.iter().map(|span| span.text(source)).collect();

            assert_eq!(rebuilt, source, "{} lost or duplicated bytes", language.as_token());

            for pair in spans.windows(2) {
                assert_eq!(pair[0].end, pair[1].start, "gap in {}", language.as_token());
            }
        }
    }

    #[test]
    fn no_two_adjacent_spans_share_a_token() {
        // Every character of punctuation as its own element triples the size of
        // a code block for no visible difference.
        let spans = scan("f(a, b, c);", Language::Rust);

        for pair in spans.windows(2) {
            assert_ne!(pair[0].token, pair[1].token, "unmerged run in {spans:?}");
        }
    }

    #[test]
    fn an_empty_block_produces_nothing_rather_than_failing() {
        for language in Language::ALL {
            assert!(scan("", language).is_empty(), "{}", language.as_token());
            assert_eq!(to_html("", Some(language)), "");
        }
    }

    #[test]
    fn nothing_panics_on_a_block_of_punctuation_or_on_broken_syntax() {
        // A deck is edited minutes before a talk, and half-typed code on a
        // slide has to render as something rather than take the build down.
        let adversarial = [
            "\"",
            "'",
            "`",
            "/*",
            "//",
            "#",
            "<",
            "</",
            "<!--",
            "r#\"",
            "\"\"\"",
            "'''",
            "\\",
            "0x",
            "1e",
            "1.",
            "'\\",
            "${",
            "<a href=\"",
            "{\"a\":",
            "'''''",
            "\u{1F600}'",
        ];

        for source in adversarial {
            for language in Language::ALL {
                let spans = scan(source, language);
                let rebuilt: String = spans.iter().map(|span| span.text(source)).collect();

                assert_eq!(rebuilt, source, "{source:?} in {}", language.as_token());
            }
        }
    }

    #[test]
    fn non_ascii_code_survives_every_scanner() {
        // A Japanese identifier, a comment, and a string are all normal in the
        // decks this tool is built for.
        let source = "// 日本語のコメント\nlet 名前 = \"スライド\";";
        let html = to_html(source, Some(Language::Rust));

        assert!(html.contains("日本語のコメント"));
        assert!(html.contains("スライド"));
    }
}
