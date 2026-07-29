//! Colouring the code blocks in a rendered slide.
//!
//! # Why this works on the output rather than on the source
//!
//! Ox Content owns what a fenced block is, and slidx does not re-implement
//! Markdown. Its renderer has no hook for the inside of a code block, so the
//! seam is the one shape it always emits:
//!
//! ```text
//! <pre><code class="language-rust">…escaped source…</code></pre>
//! ```
//!
//! Reading that back is narrow enough to be safe and is asserted against the
//! real renderer rather than against a fixture — if Ox Content ever emits a
//! different shape, the test that renders a deck and looks for colour fails.
//!
//! # What is left alone
//!
//! A block whose info string names no language slidx scans is copied through
//! untouched, byte for byte. Not "highlighted with a default grammar", and not
//! re-escaped: a deck full of pseudo-code, a language nobody wrote a scanner
//! for, and a plain fence all render exactly as they did before this module
//! existed.

use slidx_highlight::{to_html, Language};

/// The one shape Ox Content emits for a fenced block with an info string.
const OPEN: &str = "<pre><code class=\"language-";
const CLOSE: &str = "</code></pre>";

/// Rewrites every code block whose language slidx can scan.
pub fn highlight_code_blocks(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + html.len() / 2);
    let mut rest = html;

    while let Some(at) = rest.find(OPEN) {
        let name_at = at + OPEN.len();
        let Some(name_end) = rest[name_at..].find("\">").map(|offset| name_at + offset) else {
            break;
        };

        let body_at = name_end + 2;
        let Some(body_end) = rest[body_at..].find(CLOSE).map(|offset| body_at + offset) else {
            break;
        };

        out.push_str(&rest[..body_at]);

        match Language::parse(&rest[name_at..name_end]) {
            Some(language) => {
                out.push_str(&to_html(&unescape(&rest[body_at..body_end]), Some(language)));
            }
            None => out.push_str(&rest[body_at..body_end]),
        }

        rest = &rest[body_end..];
    }

    out.push_str(rest);
    out
}

/// Reverses the escaping the Markdown renderer applied.
///
/// One pass rather than five replacements: replacing `&amp;` first turns
/// `&amp;lt;` — an author writing about escaping, which is a slide that exists
/// — into a literal `<`, and the code block then shows something nobody wrote.
pub(crate) fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];

        let entity = ENTITIES.iter().find(|(entity, _)| rest.starts_with(entity));

        match entity {
            Some((entity, character)) => {
                out.push(*character);
                rest = &rest[entity.len()..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }

    out.push_str(rest);
    out
}

/// Exactly the entities the Markdown renderer produces, and no others.
///
/// A longer table would decode text the renderer never encoded: `&nbsp;` in a
/// code block is something the author typed, and turning it into a space
/// changes their code.
const ENTITIES: [(&str, char); 5] =
    [("&amp;", '&'), ("&lt;", '<'), ("&gt;", '>'), ("&quot;", '"'), ("&#39;", '\'')];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::{render, MarkdownOptions};

    fn rendered(source: &str) -> String {
        render(source, &MarkdownOptions::default())
    }

    #[test]
    fn a_fenced_block_in_a_language_slidx_scans_comes_back_coloured() {
        // End to end through the real Markdown renderer, so the shape this
        // module reads back cannot drift from the shape that is produced.
        let html = rendered("```rust\nlet x = 1; // one\n```\n");

        assert!(html.contains("<span class=\"slidx-code-keyword\">let</span>"), "{html}");
        assert!(html.contains("<span class=\"slidx-code-comment\">// one</span>"), "{html}");
    }

    #[test]
    fn a_block_in_a_language_nobody_scans_is_left_exactly_as_it_was() {
        let source = "```elixir\ndefmodule A do\nend\n```\n";
        let plain = render(source, &MarkdownOptions { highlight: false, ..Default::default() });

        assert_eq!(rendered(source), plain);
        assert!(!rendered(source).contains("slidx-code-"));
    }

    #[test]
    fn a_fence_with_no_language_is_left_exactly_as_it_was() {
        let source = "```\njust text\n```\n";
        let plain = render(source, &MarkdownOptions { highlight: false, ..Default::default() });

        assert_eq!(rendered(source), plain);
    }

    #[test]
    fn inline_code_is_not_a_code_block_and_is_not_touched() {
        let html = rendered("Call `let x = 1` in Rust.\n");

        assert!(html.contains("<code>let x = 1</code>"));
        assert!(!html.contains("slidx-code-"));
    }

    #[test]
    fn every_block_on_a_slide_is_highlighted_not_only_the_first() {
        let html = rendered("```rust\nlet a = 1;\n```\n\n```python\ndef f(): pass\n```\n");

        assert!(html.contains("<span class=\"slidx-code-keyword\">let</span>"));
        assert!(html.contains("<span class=\"slidx-code-keyword\">def</span>"));
    }

    #[test]
    fn the_source_survives_the_round_trip_through_escaping() {
        // The escape/unescape pair is the risky part: a block showing HTML, or
        // showing HTML that is already escaped, has to come out of the pipeline
        // character for character as the author wrote it.
        for source in ["<a href=\"x\">&amp;</a>", "a && b", "x < 1 && y > 2", "\"quoted\""] {
            let html = rendered(&format!("```html\n{source}\n```\n"));

            assert_eq!(code_text(&html), format!("{source}\n"), "from {source:?}");
        }
    }

    #[test]
    fn whitespace_inside_a_block_is_content_and_is_never_reflowed() {
        // Every code slide in the deck gains a phantom indent if this stops
        // being true, and nobody notices until it is on a wall.
        let source = "fn main() {\n    let x = 1;\n}\n";
        let html = rendered(&format!("```rust\n{source}```\n"));

        assert_eq!(code_text(&html), source);
    }

    #[test]
    fn an_author_writing_about_escaping_gets_what_they_wrote() {
        // `&amp;lt;` decoded in two passes becomes `<`, and the slide then
        // shows something nobody typed.
        assert_eq!(unescape("&amp;lt;"), "&lt;");
        assert_eq!(unescape("&amp;"), "&");
        assert_eq!(unescape("&nbsp;"), "&nbsp;", "an entity the renderer never wrote is content");
    }

    #[test]
    fn a_truncated_block_does_not_lose_the_rest_of_the_slide() {
        // Not reachable through the real renderer, which always closes its
        // elements — but this walks a string, and a string can be anything.
        let broken = "<p>before</p><pre><code class=\"language-rust\">let x = 1;";
        assert_eq!(highlight_code_blocks(broken), broken);
    }

    /// The text of the first code block, with the highlighting taken back off.
    fn code_text(html: &str) -> String {
        let body = html.split_once("\">").expect("a block with a language").1;
        let body = body.split_once(CLOSE).expect("a closed block").0;

        let mut out = String::new();
        let mut rest = body;

        while let Some(open) = rest.find('<') {
            out.push_str(&rest[..open]);
            rest = &rest[open + rest[open..].find('>').expect("a closed element") + 1..];
        }

        out.push_str(rest);
        unescape(&out)
    }
}
