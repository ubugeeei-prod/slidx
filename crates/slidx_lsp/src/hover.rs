//! What the thing under the cursor means.
//!
//! Hover is the documentation an author reads without deciding to go and read
//! documentation. That makes it the right home for the two facts a deck format
//! keeps needing to explain: what a frontmatter key expects, and what a preset
//! or a transition will actually do on screen.
//!
//! Every answer here comes from [`crate::vocabulary`] or from the deck itself.
//! Nothing is written twice: the sentence describing `fly-in` on hover is the
//! sentence beside it in the completion list.

use serde::{Deserialize, Serialize};
use slidx_core::scanner;

use crate::analysis::Analysis;
use crate::position::{LineIndex, Position, PositionEncoding, Range};
use crate::vocabulary::{self, Key};
use crate::{symbols, DIAGNOSTIC_SOURCE};

/// Markdown, which every client renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkupContent {
    pub kind: String,
    pub value: String,
}

impl MarkupContent {
    fn markdown(value: impl Into<String>) -> Self {
        Self { kind: "markdown".to_string(), value: value.into() }
    }
}

/// One hover response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hover {
    pub contents: MarkupContent,
    /// The token the answer is about, so the editor highlights it.
    pub range: Range,
}

/// Explains whatever is under the cursor, or nothing.
pub fn hover(
    analysis: &Analysis,
    text: &str,
    index: &LineIndex,
    position: Position,
    encoding: PositionEncoding,
) -> Option<Hover> {
    let line = index.line_text(text, position.line);
    let cursor = encoding.byte_of_column(line, position.character);
    let (token, span) = token_at(line, cursor)?;

    let markdown = if enclosed_by(line, cursor, "<!--", "-->") {
        preset_documentation(&token)
    } else if enclosed_by(line, cursor, "]{", "}") {
        mark_documentation(&token)
    } else if let Some(block) = analysis.frontmatter_at(position.line + 1) {
        frontmatter_documentation(line, span.0, &token, block.is_deck())
    } else {
        slide_documentation(analysis, text, index, position.line + 1, line)
    }?;

    Some(Hover {
        contents: MarkupContent::markdown(markdown),
        range: Range::new(
            Position::new(position.line, encoding.column_of_byte(line, span.0)),
            Position::new(position.line, encoding.column_of_byte(line, span.1)),
        ),
    })
}

/// A frontmatter key, or one of the values it accepts.
///
/// Which of the two is decided by where the colon is: an author hovering the
/// left of it is asking what the key is for, and one hovering the right is
/// asking what they just chose.
fn frontmatter_documentation(
    line: &str,
    token_start: usize,
    token: &str,
    deck: bool,
) -> Option<String> {
    let colon = line.find(':');
    let on_key = colon.is_none_or(|at| token_start <= at);

    if on_key {
        return vocabulary::key(token).filter(|key| key.applies(deck)).map(describe_key);
    }

    let name = line[..colon?].trim().trim_start_matches("- ");
    let terms = vocabulary::key(name)?.values.terms()?;

    vocabulary::find(&terms, token)
        .map(|term| format!("**{}** — {}\n\n{}", term.label, term.detail, term.documentation))
}

fn describe_key(key: &'static Key) -> String {
    format!("**{}**\n\n{}\n\nExpects {}.", key.name, key.summary, key.values.hint())
}

fn preset_documentation(token: &str) -> Option<String> {
    let presets = vocabulary::presets();
    let term = vocabulary::find(&presets, token)?;

    Some(format!("**{}** — {} effect\n\n{}", term.label, term.detail, term.documentation))
}

/// The three forms a mark attribute can take.
///
/// Which classes and properties exist is a theme's business rather than
/// slidx's, so what is explained here is the grammar — the part slidx does
/// decide.
fn mark_documentation(token: &str) -> Option<String> {
    if let Some(key) = token.strip_prefix('#') {
        return Some(format!(
            "**`#{key}`** — a stable identifier for this span.\n\nWhat `steps:` and the visual \
             editor use to address it. Two adjacent marks sharing a key are one element that \
             changes, not two elements."
        ));
    }
    if let Some(class) = token.strip_prefix('.') {
        return Some(format!(
            "**`.{class}`** — a style class, resolved by the theme.\n\nCompiles to \
             `class=\"slidx-{class}\"`, so what it looks like is the theme's decision rather \
             than the deck's."
        ));
    }

    let (name, _) = token.split_once('=')?;
    Some(format!(
        "**`{name}=`** — a typed property, resolved by the theme.\n\nCompiles to \
         `data-slidx-{name}`. Emitting resolved CSS instead would bake one theme's answer into \
         the markup."
    ))
}

/// What a slide costs, shown on its heading.
fn slide_documentation(
    analysis: &Analysis,
    text: &str,
    index: &LineIndex,
    line_number: u32,
    line: &str,
) -> Option<String> {
    scanner::heading_text(line)?;

    let at = analysis.slide_at(line_number)?;
    if analysis.content_line(at, text, index) != line_number {
        return None;
    }

    let slide = analysis.deck.slides.get(at)?;
    let findings = analysis
        .findings
        .iter()
        .filter(|finding| finding.span.slide_index == Some(at as u32))
        .count();

    let mut markdown = format!("**{}**\n\n{}", slide.display_title(), symbols::describe(slide));
    if findings > 0 {
        markdown.push_str(&format!(" · {findings} {DIAGNOSTIC_SOURCE} finding(s)"));
    }

    Some(markdown)
}

/// True when the cursor sits between an opening and closing delimiter.
fn enclosed_by(line: &str, cursor: usize, open: &str, close: &str) -> bool {
    let Some(start) = line[..cursor.min(line.len())].rfind(open) else {
        return false;
    };
    let after = &line[start + open.len()..];

    match after.find(close) {
        Some(at) => cursor <= start + open.len() + at,
        None => true,
    }
}

/// The token under the cursor, and the bytes it occupies.
///
/// Tokens keep `:`, `-`, `.`, `#`, and `=` because the vocabulary is full of
/// them — `16:9`, `fly-in`, `auto-steps`, `.accent`, `#count`, `color=`. A
/// trailing colon is dropped so `theme:` looks up as `theme`.
fn token_at(line: &str, cursor: usize) -> Option<(String, (usize, usize))> {
    let is_token = |c: char| c.is_alphanumeric() || "-_:.#=/".contains(c);

    let cursor = cursor.min(line.len());
    let start = line[..cursor].rfind(|c| !is_token(c)).map_or(0, |at| at + 1);
    let end = line[cursor..].find(|c| !is_token(c)).map_or(line.len(), |at| cursor + at);

    let token = line.get(start..end)?.trim_end_matches(':');

    (!token.is_empty()).then(|| (token.to_string(), (start, start + token.len())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;

    fn at_cursor(source: &str) -> Option<Hover> {
        let at = source.find('|').expect("fixture needs a cursor");
        let before = &source[..at];
        let position = Position::new(
            before.matches('\n').count() as u32,
            before.rsplit('\n').next().unwrap_or("").chars().count() as u32,
        );

        let text = source.replace('|', "");
        let analysis = analyze(&text);

        hover(&analysis, &text, &LineIndex::new(&text), position, PositionEncoding::Utf16)
    }

    fn markdown(source: &str) -> String {
        at_cursor(source).expect("expected a hover").contents.value
    }

    #[test]
    fn a_frontmatter_key_says_what_it_is_for_and_what_it_expects() {
        let value = markdown("---\ndur|ation: 25m\n---\n\n# One\n");

        assert!(value.contains("speaking slot"));
        assert!(value.contains("Expects seconds, or `25m`"));
    }

    #[test]
    fn a_key_written_in_kebab_case_is_still_explained() {
        assert!(markdown("---\nauto-|steps: list\n---\n\n- one\n").contains("**autoSteps**"));
    }

    #[test]
    fn a_theme_name_is_explained_in_the_theme_s_own_words() {
        let value = markdown("---\ntheme: term|inal\n---\n\n# One\n");

        assert!(value.contains(&slidx_theme::builtin::terminal().description));
    }

    #[test]
    fn a_transition_says_whether_reduced_motion_will_cancel_it() {
        // The thing an author cannot see from the name, and the reason the
        // vocabulary is only four long.
        assert!(markdown("---\ntransition: pu|sh\n---\n\n# One\n").contains("cancelled"));
    }

    #[test]
    fn an_aspect_ratio_is_explained_where_it_is_written() {
        assert!(markdown("---\naspect: 4|:3\n---\n\n# One\n").contains("1440×1080"));
    }

    #[test]
    fn a_step_preset_says_what_it_does_and_what_it_costs() {
        let value = markdown("# One\n\n- a <!-- step: type|writer -->\n");

        assert!(value.contains("character by character"));
        assert!(value.contains("repaints"), "the linter will say so later; hover says so now");
    }

    #[test]
    fn a_mark_key_explains_what_addressing_a_span_is_for() {
        assert!(markdown("[42]{#co|unt}\n").contains("steps:"));
    }

    #[test]
    fn a_mark_class_explains_that_the_theme_decides_what_it_looks_like() {
        assert!(markdown("[a]{.acc|ent}\n").contains("slidx-accent"));
    }

    #[test]
    fn a_mark_property_explains_the_attribute_it_becomes() {
        assert!(markdown("[a]{col|or=danger}\n").contains("data-slidx-color"));
    }

    #[test]
    fn a_slide_heading_says_what_the_slide_costs_a_presenter() {
        let value = markdown("---\nbudget: 90s\n---\n\n# Deep| Dive\n\n- a <!-- step -->\n");

        assert!(value.contains("2 stops"));
        assert!(value.contains("90s"));
    }

    #[test]
    fn a_slide_heading_counts_the_findings_against_it() {
        let value = markdown("# On|e\n\n![](./a.png)\n");
        assert!(value.contains("finding"), "{value}");
    }

    #[test]
    fn ordinary_prose_has_nothing_to_say() {
        assert!(at_cursor("# One\n\nJust wri|ting.\n").is_none());
    }

    #[test]
    fn a_key_slidx_does_not_know_is_left_alone() {
        // Frontmatter is open: a theme option is not an error and not this
        // server's to explain.
        assert!(at_cursor("---\ntheme|Option: true\n---\n\n# One\n").is_none());
    }

    #[test]
    fn a_deck_only_key_is_not_explained_inside_a_slide_block() {
        assert!(at_cursor("# One\n\n---\ntit|le: T\n---\n\n# Two\n").is_none());
    }

    #[test]
    fn the_highlighted_range_covers_the_token_and_not_the_line() {
        let hovered = at_cursor("---\nthe|me: terminal\n---\n\n# One\n").unwrap();

        assert_eq!(hovered.range.start.character, 0);
        assert_eq!(hovered.range.end.character, 5, "`theme`, without its colon");
    }

    #[test]
    fn a_token_after_japanese_text_is_highlighted_in_code_units() {
        let hovered = at_cursor("---\ntitle: 高速なデッキ\ntransition: fa|de\n---\n\n# 導入\n");
        let range = hovered.unwrap().range;

        assert_eq!(range.start.character, 12, "past `transition: `");
        assert_eq!(range.end.character, 16);
    }

    #[test]
    fn hover_serialises_with_the_shape_the_protocol_expects() {
        let json =
            serde_json::to_value(at_cursor("---\nthe|me: terminal\n---\n").unwrap()).unwrap();

        assert_eq!(json["contents"]["kind"], "markdown");
        assert!(json["range"]["start"].is_object());
    }
}
