//! Reference tables, read out of the code rather than copied into prose.
//!
//! # Why nothing here is written twice
//!
//! A reference page is the part of documentation that ages fastest and most
//! quietly. Someone adds an effect preset, and a page listing nineteen of them
//! is now wrong in a way no test notices and no reviewer sees — the page still
//! builds, still reads well, and is missing the thing the reader came for.
//!
//! So every closed set slidx has is rendered from the one place that defines
//! it. The frontmatter keys and the value vocabularies come from
//! [`slidx_lsp::vocabulary`], which is already the module that refuses to
//! restate an enum; the commands come from the table the parser, the help text
//! and six completion scripts read; the rule groups come from the linter's own
//! registry. Adding a preset changes this site by rebuilding it.
//!
//! # How a page asks for one
//!
//! An HTML comment on its own line:
//!
//! ```md
//! <!-- slidx-docs: frontmatter-deck -->
//! ```
//!
//! A placeholder naming a block nobody generates is an error rather than a
//! comment left in the output, because the failure it prevents is a page that
//! publishes with a hole where its reference was.

pub mod cli;
pub mod lint;
pub mod vocabulary;

use slidx_render::{render_markdown, MarkdownOptions};

/// What a page may ask for.
///
/// Named rather than discovered so the error for a typo can list the real ones.
pub const NAMES: &[&str] = &[
    "frontmatter-deck",
    "frontmatter-slide",
    "themes",
    "transitions",
    "aspects",
    "auto-steps",
    "step-presets",
    "lint-groups",
    "commands",
    "declined",
];

/// Renders one block, or `None` when nothing generates that name.
pub fn block(name: &str) -> Option<String> {
    match name {
        "frontmatter-deck" => Some(vocabulary::deck_keys()),
        "frontmatter-slide" => Some(vocabulary::slide_keys()),
        "themes" => Some(vocabulary::themes()),
        "transitions" => Some(vocabulary::transitions()),
        "aspects" => Some(vocabulary::aspects()),
        "auto-steps" => Some(vocabulary::auto_steps()),
        "step-presets" => Some(vocabulary::step_presets()),
        "lint-groups" => Some(lint::groups()),
        "commands" => Some(cli::commands()),
        "declined" => Some(cli::declined()),
        _ => None,
    }
}

/// Fills every placeholder in a rendered page.
///
/// Returns the names a page asked for that nothing generates, so the caller can
/// fail rather than publish a page with a hole in it.
pub fn fill(html: &str) -> (String, Vec<String>) {
    const OPEN: &str = "<!-- slidx-docs: ";

    let mut out = String::with_capacity(html.len());
    let mut missing = Vec::new();
    let mut rest = html;

    while let Some(start) = rest.find(OPEN) {
        let (before, after) = rest.split_at(start);
        out.push_str(before);

        let Some((request, remainder)) = after[OPEN.len()..].split_once("-->") else {
            break;
        };

        let name = request.trim();
        match block(name) {
            Some(rendered) => out.push_str(&rendered),
            None => missing.push(name.to_string()),
        }

        rest = remainder;
    }

    out.push_str(rest);
    (out, missing)
}

/// An HTML table, from headers and rows of already-rendered cells.
pub(crate) fn table(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut html = String::from("<table>\n<thead>\n<tr>");

    for header in headers {
        html.push_str(&format!("<th>{}</th>", escape(header)));
    }

    html.push_str("</tr>\n</thead>\n<tbody>\n");

    for row in rows {
        html.push_str("<tr>");
        for cell in row {
            html.push_str(&format!("<td>{cell}</td>"));
        }
        html.push_str("</tr>\n");
    }

    html.push_str("</tbody>\n</table>");
    html
}

/// A cell holding prose that was written as Markdown.
///
/// The descriptions in the registries are Markdown — they say things like
/// "cancelled under `prefers-reduced-motion`" — so they go through the same
/// engine the pages do rather than being printed with their backticks showing.
/// A single paragraph is unwrapped, because a `<p>` inside a table cell buys
/// margins nobody wants.
pub(crate) fn prose(text: &str) -> String {
    let html = render_markdown(text, &MarkdownOptions::default());
    let trimmed = html.trim();

    match trimmed.strip_prefix("<p>").and_then(|rest| rest.strip_suffix("</p>")) {
        Some(inner) if !inner.contains("<p>") => inner.to_string(),
        _ => trimmed.to_string(),
    }
}

/// A cell holding one literal token.
pub(crate) fn code(text: &str) -> String {
    format!("<code>{}</code>", escape(text))
}

pub(crate) fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_generates_something() {
        for name in NAMES {
            let rendered = block(name).unwrap_or_else(|| panic!("{name} generates nothing"));
            assert!(!rendered.trim().is_empty(), "{name} generated an empty block");
        }
    }

    #[test]
    fn a_placeholder_naming_a_block_nobody_generates_is_reported() {
        // Otherwise the comment survives into the published page and the
        // reference the reader came for is simply absent.
        let (_, missing) = fill("<p>before</p>\n<!-- slidx-docs: nothing -->\n<p>after</p>");

        assert_eq!(missing, vec!["nothing".to_string()]);
    }

    #[test]
    fn a_filled_placeholder_leaves_the_rest_of_the_page_alone() {
        let (html, missing) = fill("<p>before</p>\n<!-- slidx-docs: themes -->\n<p>after</p>");

        assert!(missing.is_empty());
        assert!(html.starts_with("<p>before</p>"));
        assert!(html.trim_end().ends_with("<p>after</p>"));
        assert!(html.contains("<table>"));
    }

    #[test]
    fn an_ordinary_comment_is_left_where_it_was() {
        let html = "<p>a</p>\n<!-- an aside -->\n<p>b</p>";
        assert_eq!(fill(html).0, html);
    }

    #[test]
    fn prose_with_one_paragraph_arrives_without_its_wrapper() {
        assert_eq!(prose("Fades in. The default entrance."), "Fades in. The default entrance.");
    }

    #[test]
    fn prose_keeps_the_markup_its_backticks_asked_for() {
        assert!(prose("cancelled under `prefers-reduced-motion`").contains("<code>"));
    }
}
