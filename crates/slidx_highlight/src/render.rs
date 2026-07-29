//! Spans to HTML.
//!
//! The output is `<span class="slidx-code-…">` and nothing else: no inline
//! styles, no colour values, no wrapper element. The theme owns every colour in
//! this project, and a highlighter that wrote `#6a737d` into a page would be a
//! second place colours come from — one the contrast audit cannot see and a
//! theme cannot override.

use std::fmt::Write as _;

use crate::language::Language;
use crate::scan::scan;

/// Highlights `source`, or escapes it unchanged when the language is unknown.
///
/// Taking `Option<Language>` rather than making the caller branch is the point:
/// "no language" is the normal case for a fence with no info string, and it has
/// to produce the same escaping as a highlighted block or a deck would render
/// differently depending on whether anyone recognised it.
pub fn to_html(source: &str, language: Option<Language>) -> String {
    let Some(language) = language else {
        return escape(source);
    };

    let mut out = String::with_capacity(source.len() * 2);

    for span in scan(source, language) {
        match span.token.class() {
            Some(class) => {
                let _ = write!(out, "<span class=\"{class}\">{}</span>", escape(span.text(source)));
            }
            None => out.push_str(&escape(span.text(source))),
        }
    }

    out
}

/// The three characters that change how a text node parses.
///
/// Quotes are left alone deliberately. Code is full of them, escaping them
/// triples the size of a block in the emitted document, and inside element
/// content they mean nothing to a parser.
fn escape(text: &str) -> String {
    if !text.contains(['&', '<', '>']) {
        return text.to_string();
    }

    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_language_is_escaped_and_left_alone() {
        let html = to_html("fn main() {}", None);

        assert_eq!(html, "fn main() {}");
        assert!(!html.contains("<span"));
    }

    #[test]
    fn markup_in_the_source_cannot_escape_the_code_block() {
        // A slide showing a JSX snippet or a shell heredoc is ordinary content,
        // and it must not be able to close the element it is drawn in.
        let html = to_html("const a = \"</code><script>\";", Some(Language::JavaScript));

        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;/code&gt;"));
    }

    #[test]
    fn an_ampersand_survives_as_an_ampersand() {
        // Escaping `&` twice is the classic corruption, and shell slides are
        // full of `&&`.
        let html = to_html("a && b", Some(Language::Shell));

        assert!(html.contains("&amp;&amp;"));
        assert!(!html.contains("&amp;amp;"));
    }

    #[test]
    fn quotes_are_left_as_written() {
        // Inside element content a quote means nothing to a parser, and code is
        // full of them.
        let html = to_html("print(\"hi\")", Some(Language::Python));

        assert!(html.contains("\"hi\""));
        assert!(!html.contains("&quot;"));
    }

    #[test]
    fn no_colour_value_ever_reaches_the_output() {
        // The theme owns every colour in this project. A highlighter that wrote
        // one would be a second source the contrast audit cannot see.
        let html = to_html("// note\nlet x = 1;", Some(Language::Rust));

        assert!(!html.contains("style="));
        assert!(!html.contains('#'));
        assert!(html.contains("class=\"slidx-code-comment\""));
    }

    #[test]
    fn the_text_of_the_source_survives_verbatim() {
        // Whitespace inside a code block is content. Anything that reflowed it
        // would put a phantom indent on every slide that shows code.
        let source = "def f():\n    return {\n        'a': 1,\n    }\n";
        let stripped = strip_tags(&to_html(source, Some(Language::Python)));

        assert_eq!(stripped, source);
    }

    #[test]
    fn highlighting_is_deterministic() {
        let source = "fn main() { println!(\"hi\"); }";

        assert_eq!(to_html(source, Some(Language::Rust)), to_html(source, Some(Language::Rust)));
    }

    /// The emitted HTML with every element removed and entities resolved.
    fn strip_tags(html: &str) -> String {
        let mut out = String::new();
        let mut rest = html;

        while let Some(open) = rest.find('<') {
            out.push_str(&rest[..open]);
            match rest[open..].find('>') {
                Some(close) => rest = &rest[open + close + 1..],
                None => break,
            }
        }

        out.push_str(rest);
        out.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
    }
}
