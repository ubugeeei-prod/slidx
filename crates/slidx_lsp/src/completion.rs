//! What can be written here.
//!
//! # Closed sets from Rust, open sets from the deck
//!
//! Everything slidx decides — themes, transitions, presets, staging modes,
//! aspect ratios, frontmatter keys — is offered from [`crate::vocabulary`],
//! which reads the definitions rather than restating them.
//!
//! Everything the *author* decides is offered from the document they are
//! editing. A mark's classes and properties are theirs: `.accent` means
//! whatever their theme says it means, and slidx has no list of them to be
//! right or wrong about. Harvesting them from the deck is both the only
//! honest answer and the more useful one — the second `.highlight` completes
//! from the first.
//!
//! Between the two there is nothing left for this module to invent.

use serde::{Deserialize, Serialize};
use slidx_core::Deck;

use crate::analysis::{Analysis, FrontmatterBlock};
use crate::position::{LineIndex, Position, PositionEncoding};
use crate::vocabulary::{self, Key, Term};

/// The `CompletionItemKind` values this server uses, which are numbers on the
/// wire. Editors pick an icon from them, so the choice is about how a list
/// reads at a glance rather than about types.
mod kind {
    pub const FIELD: u8 = 5;
    pub const CLASS: u8 = 7;
    pub const PROPERTY: u8 = 10;
    pub const REFERENCE: u8 = 18;
    pub const ENUM_MEMBER: u8 = 20;
    pub const EVENT: u8 = 23;
}

/// One offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    pub kind: u8,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub documentation: String,
    /// Written instead of the label when the two differ, which is how a key
    /// completes to `title: ` rather than to `title`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
}

impl CompletionItem {
    fn from_term(term: &Term, kind: u8) -> Self {
        Self {
            label: term.label.clone(),
            kind,
            detail: term.detail.clone(),
            documentation: term.documentation.clone(),
            insert_text: None,
        }
    }
}

/// Where the cursor is, in terms of what may be written there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    /// At the start of a frontmatter line, where a key goes.
    FrontmatterKey { deck: bool },
    /// After `key:`, where its value goes.
    FrontmatterValue(&'static Key),
    /// Inside a `steps:` list item, where a selector goes.
    StepTarget,
    /// Inside `<!-- step: … -->`, where a preset goes.
    StepPreset,
    /// Inside `[text]{ … }`, where classes and properties go.
    MarkAttribute,
    /// Ordinary prose, where slidx has nothing to add.
    Body,
}

/// Everything that may be written at a position.
pub fn complete(
    analysis: &Analysis,
    text: &str,
    index: &LineIndex,
    position: Position,
    encoding: PositionEncoding,
) -> Vec<CompletionItem> {
    match context(analysis, text, index, position, encoding) {
        Context::FrontmatterKey { deck } => vocabulary::keys_for(deck)
            .into_iter()
            .map(|key| CompletionItem {
                insert_text: Some(format!("{}: ", key.name)),
                ..CompletionItem::from_term(&key.as_term(), kind::PROPERTY)
            })
            .collect(),

        Context::FrontmatterValue(key) => key
            .values
            .terms()
            .unwrap_or_default()
            .iter()
            .map(|term| CompletionItem::from_term(term, kind::ENUM_MEMBER))
            .collect(),

        Context::StepPreset => vocabulary::presets()
            .iter()
            .map(|term| CompletionItem::from_term(term, kind::EVENT))
            .collect(),

        Context::StepTarget => mark_keys(&analysis.deck),
        Context::MarkAttribute => mark_attributes(&analysis.deck),
        Context::Body => Vec::new(),
    }
}

/// Reads the cursor's surroundings.
///
/// The order matters: a step marker and a mark can both appear on a line
/// inside a frontmatter block's `steps:` value, and the innermost construct is
/// the one being typed.
pub fn context(
    analysis: &Analysis,
    text: &str,
    index: &LineIndex,
    position: Position,
    encoding: PositionEncoding,
) -> Context {
    let line = index.line_text(text, position.line);
    let before = &line[..encoding.byte_of_column(line, position.character)];

    if inside_step_marker(before) {
        return Context::StepPreset;
    }
    if inside_mark_attributes(before) {
        return Context::MarkAttribute;
    }

    let Some(block) = analysis.frontmatter_at(position.line + 1) else {
        return Context::Body;
    };

    let Some((name, _)) = split_key(before) else {
        return Context::FrontmatterKey { deck: block.is_deck() };
    };

    if enclosing_key(text, index, block, position.line) == Some("steps".to_string()) {
        return Context::StepTarget;
    }

    match vocabulary::key(&name) {
        Some(key) => Context::FrontmatterValue(key),
        None => Context::Body,
    }
}

/// True when the cursor sits inside an unclosed `<!-- step … -->`.
fn inside_step_marker(before: &str) -> bool {
    let Some(open) = before.rfind("<!--") else {
        return false;
    };
    let inner = &before[open + 4..];

    !inner.contains("-->") && inner.trim_start().starts_with("step")
}

/// True when the cursor sits inside an unclosed `[text]{ … }`.
fn inside_mark_attributes(before: &str) -> bool {
    let Some(open) = before.rfind("]{") else {
        return false;
    };

    !before[open + 2..].contains('}')
}

/// Splits `key: value` as written so far, ignoring a list item's dash.
fn split_key(before: &str) -> Option<(String, &str)> {
    let trimmed = before.trim_start().trim_start_matches("- ");
    let (name, rest) = trimmed.split_once(':')?;
    let name = name.trim();

    // A bare `-` with nothing after it is a list item, not a key.
    (!name.is_empty()).then(|| (name.to_string(), rest))
}

/// The top-level key whose value the cursor is inside.
///
/// `steps:` is the only frontmatter value with structure under it, and its
/// list items are keys of their own — so without this, `- reveal:` on line
/// four would be looked up as a frontmatter key and found not to be one.
fn enclosing_key(
    text: &str,
    index: &LineIndex,
    block: FrontmatterBlock,
    line: u32,
) -> Option<String> {
    let mut enclosing = None;

    for candidate in block.lines.first..=line.saturating_add(1).min(block.lines.last) {
        let text = index.line_text(text, candidate.saturating_sub(1));
        if text.starts_with([' ', '\t', '-']) || text.trim().is_empty() {
            continue;
        }

        enclosing = text.split_once(':').map(|(name, _)| name.trim().to_string());
    }

    enclosing.filter(|_| {
        let current = index.line_text(text, line);
        current.starts_with([' ', '\t', '-'])
    })
}

/// Mark keys the deck already defines, for a step to target.
fn mark_keys(deck: &Deck) -> Vec<CompletionItem> {
    let mut keys: Vec<String> = deck
        .slides
        .iter()
        .flat_map(|slide| slide.marks.iter())
        .filter_map(|mark| mark.key.clone())
        .collect();

    keys.sort_unstable();
    keys.dedup();

    keys.into_iter()
        .map(|key| CompletionItem {
            label: format!("#{key}"),
            kind: kind::REFERENCE,
            detail: "mark in this deck".to_string(),
            documentation: String::new(),
            insert_text: None,
        })
        .collect()
}

/// Classes and property names the deck already uses.
///
/// Neither is a closed set: a class means whatever the theme says, and a
/// property becomes a `data-` attribute the theme reads. slidx has no list of
/// either to offer, and the author's own vocabulary is the better one anyway.
fn mark_attributes(deck: &Deck) -> Vec<CompletionItem> {
    let marks = || deck.slides.iter().flat_map(|slide| slide.marks.iter());

    let mut classes: Vec<String> = marks().flat_map(|mark| mark.classes.clone()).collect();
    classes.sort_unstable();
    classes.dedup();

    let mut properties: Vec<String> =
        marks().flat_map(|mark| mark.properties.keys().cloned().collect::<Vec<String>>()).collect();
    properties.sort_unstable();
    properties.dedup();

    classes
        .into_iter()
        .map(|class| CompletionItem {
            label: format!(".{class}"),
            kind: kind::CLASS,
            detail: "class used in this deck".to_string(),
            documentation: String::new(),
            insert_text: None,
        })
        .chain(properties.into_iter().map(|property| CompletionItem {
            label: format!("{property}="),
            kind: kind::FIELD,
            detail: "property used in this deck".to_string(),
            documentation: String::new(),
            insert_text: None,
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;

    /// Completes where `|` is, which keeps a fixture readable as the line an
    /// author would actually be looking at.
    fn at_cursor(source: &str) -> Vec<CompletionItem> {
        let (text, position) = split_cursor(source);
        let analysis = analyze(&text);

        complete(&analysis, &text, &LineIndex::new(&text), position, PositionEncoding::Utf16)
    }

    fn context_at(source: &str) -> Context {
        let (text, position) = split_cursor(source);
        let analysis = analyze(&text);

        context(&analysis, &text, &LineIndex::new(&text), position, PositionEncoding::Utf16)
    }

    fn split_cursor(source: &str) -> (String, Position) {
        let at = source.find('|').expect("fixture needs a cursor");
        let before = &source[..at];
        let line = before.matches('\n').count() as u32;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count() as u32;

        (source.replace('|', ""), Position::new(line, column))
    }

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|item| item.label.as_str()).collect()
    }

    #[test]
    fn a_frontmatter_line_offers_the_keys_that_belong_in_it() {
        let items = at_cursor("---\n|\n---\n\n# One\n");

        assert!(labels(&items).contains(&"title"));
        assert_eq!(items[0].insert_text.as_deref(), Some("title: "), "and leaves room for a value");
    }

    #[test]
    fn a_slide_block_does_not_offer_deck_keys() {
        let items = at_cursor("# One\n\n---\nbudget: 30s\n|\n---\n\n# Two\n");

        assert!(!labels(&items).contains(&"title"));
        assert!(labels(&items).contains(&"layout"));
    }

    #[test]
    fn theme_names_come_from_the_themes_that_exist() {
        let items = at_cursor("---\ntheme: |\n---\n\n# One\n");

        assert_eq!(labels(&items), vec!["minimal", "editorial", "terminal", "contrast"]);
        assert_eq!(items[0].documentation, slidx_theme::builtin::minimal().description);
    }

    #[test]
    fn transition_names_come_from_the_transitions_that_exist() {
        let items = at_cursor("---\ntransition: |\n---\n\n# One\n");
        assert_eq!(
            labels(&items),
            slidx_theme::Transition::ALL.iter().map(|kind| kind.as_token()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_free_text_key_offers_nothing_to_pick_from() {
        // Guessing at someone's talk title is worse than staying quiet.
        assert!(at_cursor("---\ntitle: |\n---\n\n# One\n").is_empty());
    }

    #[test]
    fn frontmatter_being_typed_completes_before_it_is_closed() {
        // The first thing anyone does in a new deck.
        assert!(labels(&at_cursor("---\n|")).contains(&"title"));
        assert!(labels(&at_cursor("---\ntheme: |")).contains(&"terminal"));
    }

    #[test]
    fn a_step_marker_offers_every_preset() {
        let items = at_cursor("# One\n\n- a <!-- step: |\n");

        assert_eq!(items.len(), slidx_core::EffectPreset::ALL.len());
        assert!(labels(&items).contains(&"fly-in"));
    }

    #[test]
    fn a_closed_step_marker_is_no_longer_being_typed() {
        assert_eq!(context_at("- a <!-- step: fade -->|\n"), Context::Body);
    }

    #[test]
    fn a_comment_that_is_not_a_step_marker_offers_nothing() {
        assert_eq!(context_at("<!-- notes: say something |"), Context::Body);
    }

    #[test]
    fn a_mark_offers_the_classes_the_deck_already_uses() {
        // slidx has no list of classes to be right about; the author does.
        let items = at_cursor("# One\n\n[a]{.accent} and [b]{|\n");

        assert!(labels(&items).contains(&".accent"));
    }

    #[test]
    fn a_mark_offers_the_property_names_the_deck_already_uses() {
        let items = at_cursor("# One\n\n[a]{color=danger} and [b]{|\n");

        assert!(labels(&items).contains(&"color="));
    }

    #[test]
    fn a_closed_mark_is_no_longer_being_typed() {
        assert_eq!(context_at("[a]{.accent}|\n"), Context::Body);
    }

    #[test]
    fn a_link_is_not_a_mark() {
        assert_eq!(context_at("[slidx](https://example.com|)\n"), Context::Body);
    }

    #[test]
    fn a_step_target_offers_the_marks_it_could_name() {
        // The one completion that saves an author scrolling: which keys exist.
        let items = at_cursor("---\nsteps:\n  - reveal: |\n---\n\n[42]{#count}\n");

        assert_eq!(labels(&items), vec!["#count"]);
    }

    #[test]
    fn a_steps_list_item_is_not_read_as_a_frontmatter_key() {
        assert_eq!(context_at("---\nsteps:\n  - reveal: |\n---\n\n# One\n"), Context::StepTarget);
    }

    #[test]
    fn a_key_slidx_does_not_know_offers_nothing_rather_than_guessing() {
        // Frontmatter is open on purpose: themes and plugins read keys this
        // crate has never heard of, and it must not claim they are wrong.
        assert_eq!(context_at("---\nthemeOption: |\n---\n\n# One\n"), Context::Body);
    }

    #[test]
    fn prose_offers_nothing() {
        assert_eq!(context_at("# One\n\nJust wri|ting.\n"), Context::Body);
    }

    #[test]
    fn a_cursor_after_japanese_text_still_reads_its_own_line() {
        // The column arrives in UTF-16 units; slicing it as bytes would cut a
        // kanji in half and panic.
        let items = at_cursor("---\ntitle: 高速なデッキ\ntheme: |\n---\n\n# 導入\n");

        assert!(labels(&items).contains(&"terminal"));
    }

    #[test]
    fn a_completion_item_serialises_with_the_field_names_the_protocol_uses() {
        let json = serde_json::to_value(&at_cursor("---\n|\n---\n\n# One\n")[0]).unwrap();

        assert!(json.get("insertText").is_some(), "{json}");
        assert_eq!(json["kind"], 10);
    }
}
